//! Windows media overlay (System Media Transport Controls) and media keys.
//!
//! Runs on its own thread: it receives player status updates, keeps the
//! overlay's title/artist/cover and play state current, and turns media key
//! presses into player commands.

use crate::audio::{self, AudioHandle, Command, PlayState, Status};
use crate::library::{db, scan, Library};
use crossbeam_channel::{Receiver, Sender};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// How far the "seek" media buttons jump when the OS doesn't say.
const SEEK_STEP: f64 = 10.0;

#[derive(Clone)]
pub struct MediaHandle {
    tx: Sender<Status>,
}

impl MediaHandle {
    pub fn update(&self, status: Status) {
        let _ = self.tx.send(status);
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
    path: Option<std::path::PathBuf>,
    state: Option<PlayState>,
}

fn run(mut controls: MediaControls, rx: Receiver<Status>, library: &Library) {
    let mut shown = Shown { path: None, state: None };
    while let Ok(mut status) = rx.recv() {
        // Only the latest status matters.
        while let Ok(newer) = rx.try_recv() {
            status = newer;
        }
        let path = status.track.as_ref().map(|t| t.path.clone());
        if path != shown.path {
            match &path {
                Some(p) => set_metadata(&mut controls, library, p, status.duration),
                None => {
                    if let Err(e) = controls.set_metadata(MediaMetadata::default()) {
                        log::warn!("media overlay: can't clear metadata: {e:?}");
                    }
                }
            }
            shown.path = path;
        }
        if shown.state != Some(status.state) || status.state != PlayState::Stopped {
            let progress = Some(MediaPosition(Duration::from_secs_f64(status.position.max(0.0))));
            let playback = match status.state {
                PlayState::Playing => MediaPlayback::Playing { progress },
                PlayState::Paused => MediaPlayback::Paused { progress },
                PlayState::Stopped => MediaPlayback::Stopped,
            };
            if let Err(e) = controls.set_playback(playback) {
                log::warn!("media overlay: can't set playback state: {e:?}");
            }
            shown.state = Some(status.state);
        }
    }
}

fn set_metadata(controls: &mut MediaControls, library: &Library, path: &Path, duration: Option<f64>) {
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
        Ok(None) => None,
        Err(e) => {
            log::warn!("media overlay: no cover for {}: {e}", path.display());
            None
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
    if let (Some(m), audio::Event::Status(s)) = (media, event) {
        m.update(s.clone());
    }
}
