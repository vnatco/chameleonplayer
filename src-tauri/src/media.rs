//! Windows media overlay (System Media Transport Controls) and media keys.
//!
//! Runs on its own thread: it receives player status and position updates,
//! keeps the overlay's title/artist/cover, play state and timeline current,
//! and turns media key presses into player commands.

use crate::audio::{self, AudioHandle, Command, PlayState, Status};
use crate::library::{db, scan, Library};
use crossbeam_channel::{Receiver, Sender};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How far the "seek" media buttons jump when the OS doesn't say.
const SEEK_STEP: f64 = 10.0;
/// Timeline refresh while playing (spec E: "every ~5 s").
const TIMELINE_EVERY: Duration = Duration::from_secs(5);

enum Msg {
    Status(Status),
    Position(f64),
    /// No-cover image for a track (rendered by the UI), so the overlay is
    /// never blank.
    Fallback { track: PathBuf, image: PathBuf },
}

#[derive(Clone)]
pub struct MediaHandle {
    tx: Sender<Msg>,
}

impl MediaHandle {
    pub fn update(&self, status: Status) {
        let _ = self.tx.send(Msg::Status(status));
    }

    pub fn fallback(&self, track: PathBuf, image: PathBuf) {
        let _ = self.tx.send(Msg::Fallback { track, image });
    }
}

pub fn spawn(hwnd: *mut std::ffi::c_void, audio: AudioHandle, library: Arc<Library>) -> Result<MediaHandle, String> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let hwnd_addr = hwnd as usize;
    let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);
    std::thread::Builder::new()
        .name("media-controls".into())
        .spawn(move || {
            let config = PlatformConfig {
                display_name: "Chameleon Player",
                dbus_name: "chameleon_player",
                hwnd: Some(hwnd_addr as *mut std::ffi::c_void),
            };
            let mut controls = match MediaControls::new(config) {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Media overlay unavailable: {e:?}")));
                    return;
                }
            };
            let keys = audio.clone();
            if let Err(e) = controls.attach(move |event| on_event(&keys, event)) {
                let _ = ready_tx.send(Err(format!("Media keys unavailable: {e:?}")));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            run(controls, rx, &library);
        })
        .map_err(|e| format!("Can't start media controls: {e}"))?;
    ready_rx.recv().map_err(|_| "Media controls thread exited".to_string())??;
    Ok(MediaHandle { tx })
}

fn on_event(audio: &AudioHandle, event: MediaControlEvent) {
    let pos = || audio.status().position;
    match event {
        MediaControlEvent::Play => audio.send(Command::Play),
        MediaControlEvent::Pause => audio.send(Command::Pause),
        MediaControlEvent::Toggle => audio.send(Command::Toggle),
        MediaControlEvent::Next => audio.send(Command::Next),
        MediaControlEvent::Previous => audio.send(Command::Prev),
        MediaControlEvent::Stop => audio.send(Command::Stop),
        MediaControlEvent::Seek(dir) => audio.send(Command::Seek(step(pos(), dir, SEEK_STEP))),
        MediaControlEvent::SeekBy(dir, by) => audio.send(Command::Seek(step(pos(), dir, by.as_secs_f64()))),
        MediaControlEvent::SetPosition(MediaPosition(p)) => audio.send(Command::Seek(p.as_secs_f64())),
        MediaControlEvent::SetVolume(v) => audio.send(Command::SetVolume(v as f32)),
        _ => {}
    }
}

fn step(pos: f64, dir: SeekDirection, by: f64) -> f64 {
    match dir {
        SeekDirection::Forward => pos + by,
        SeekDirection::Backward => (pos - by).max(0.0),
    }
}

struct Shown {
    path: Option<PathBuf>,
    duration: Option<f64>,
    state: Option<PlayState>,
    timeline_at: Instant,
}

fn set_playback(controls: &mut MediaControls, state: PlayState, position: f64) {
    let progress = Some(MediaPosition(Duration::from_secs_f64(position.max(0.0))));
    let playback = match state {
        PlayState::Playing => MediaPlayback::Playing { progress },
        PlayState::Paused => MediaPlayback::Paused { progress },
        PlayState::Stopped => MediaPlayback::Stopped,
    };
    if let Err(e) = controls.set_playback(playback) {
        log::warn!("media overlay: can't set playback state: {e:?}");
    }
}

fn run(mut controls: MediaControls, rx: Receiver<Msg>, library: &Library) {
    let mut shown = Shown { path: None, duration: None, state: None, timeline_at: Instant::now() };
    let mut fallbacks: HashMap<PathBuf, PathBuf> = HashMap::new();
    let key = |p: &Path| crate::library::normalize_path(p);
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Status(status) => {
                let path = status.track.as_ref().map(|t| t.path.clone());
                if path != shown.path || status.duration != shown.duration {
                    match &path {
                        Some(p) => set_metadata(&mut controls, library, p, status.duration, fallbacks.get(&key(p))),
                        None => {
                            if let Err(e) = controls.set_metadata(MediaMetadata::default()) {
                                log::warn!("media overlay: can't clear metadata: {e:?}");
                            }
                        }
                    }
                    shown.path = path;
                    shown.duration = status.duration;
                }
                // Keep Paused vs Stopped exact, and the timeline in step.
                set_playback(&mut controls, status.state, status.position);
                shown.state = Some(status.state);
                shown.timeline_at = Instant::now();
            }
            Msg::Position(pos) => {
                if shown.state == Some(PlayState::Playing) && shown.timeline_at.elapsed() >= TIMELINE_EVERY {
                    set_playback(&mut controls, PlayState::Playing, pos);
                    shown.timeline_at = Instant::now();
                }
            }
            Msg::Fallback { track, image } => {
                let track = key(&track);
                if fallbacks.len() > 64 {
                    fallbacks.clear();
                }
                fallbacks.insert(track.clone(), image);
                if let Some(p) = shown.path.clone() {
                    if key(&p) == track {
                        set_metadata(&mut controls, library, &p, shown.duration, fallbacks.get(&track));
                    }
                }
            }
        }
    }
}

fn set_metadata(controls: &mut MediaControls, library: &Library, path: &Path, duration: Option<f64>, fallback: Option<&PathBuf>) {
    let normalized = crate::library::normalize_path(path);
    let row = library.with(|c| db::track_by_path(c, &normalized.to_string_lossy())).ok().flatten();
    let (title, artist, album) = match &row {
        Some(t) => (t.title.clone(), t.artist.clone(), t.album.clone()),
        None => match scan::read(path, false) {
            Ok((t, _)) => (t.title, t.artist, t.album),
            Err(_) => (path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), String::new(), String::new()),
        },
    };
    let cover_url = match library.cover_for_path(path) {
        Ok(Some((cover, _))) => Some(format!("file://{}", cover.full)),
        Ok(None) => fallback.map(|f| format!("file://{}", f.display())),
        Err(e) => {
            log::warn!("media overlay: no cover for {}: {e}", path.display());
            fallback.map(|f| format!("file://{}", f.display()))
        }
    };
    let meta = MediaMetadata {
        title: Some(&title),
        artist: (!artist.is_empty()).then_some(artist.as_str()),
        album: (!album.is_empty()).then_some(album.as_str()),
        cover_url: cover_url.as_deref(),
        duration: duration.map(Duration::from_secs_f64),
    };
    if let Err(e) = controls.set_metadata(meta) {
        log::warn!("media overlay: can't set metadata: {e:?}");
    }
}

/// Forward audio events to the overlay as well as the UI.
pub fn forward(media: &Option<MediaHandle>, event: &audio::Event) {
    let Some(m) = media else { return };
    match event {
        audio::Event::Status(s) => m.update(s.clone()),
        audio::Event::Position { position, .. } => {
            let _ = m.tx.send(Msg::Position(*position));
        }
        _ => {}
    }
}
