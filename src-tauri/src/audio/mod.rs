//! Playback engine. Runs on its own thread (the audio device handle is not
//! `Send`) and is driven by commands over a channel.
//!
//! Gapless playback: while a track plays, the next one is already appended to
//! the rodio `Player`, so the decoder hands over sample-exactly. The engine
//! notices the hand-over when the player's queue length drops.

mod meter;

use crossbeam_channel::{Receiver, Sender};
use meter::{Level, Meter};
use parking_lot::Mutex;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A decoded, metered track ready to append to the player.
type TrackSource = Meter<Decoder<BufReader<File>>>;

const TICK: Duration = Duration::from_millis(33);
const POSITION_EVERY: Duration = Duration::from_millis(250);
/// "Previous" restarts the current track instead when past this point.
const PREV_RESTART_AFTER: f64 = 3.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Library id, when the track came from the library.
    pub id: Option<i64>,
    pub path: PathBuf,
    /// Known duration in seconds (from tags), used when the decoder can't tell.
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: PlayState,
    pub index: Option<usize>,
    pub track: Option<Track>,
    pub position: f64,
    pub duration: Option<f64>,
    pub volume: f32,
    pub repeat: Repeat,
    pub queue_len: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// Full status; sent on any state change.
    Status(Status),
    /// Lightweight position update while playing.
    Position { position: f64, duration: Option<f64> },
    /// Bass level 0..1 for the glow pulse (only while the meter is enabled).
    Level { value: f32 },
    Error { path: Option<PathBuf>, message: String },
}

#[derive(Debug)]
pub enum Command {
    Load { queue: Vec<Track>, index: usize, autoplay: bool },
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    PlayIndex(usize),
    Seek(f64),
    SetVolume(f32),
    SetRepeat(Repeat),
    SetMeter(bool),
    Stop,
}

pub type Emitter = Arc<dyn Fn(Event) + Send + Sync>;

/// Cheap, cloneable handle used by the rest of the app.
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Command>,
    status: Arc<Mutex<Status>>,
}

impl AudioHandle {
    pub fn send(&self, cmd: Command) {
        // The engine thread lives as long as the app; a send can only fail
        // during shutdown, when there's nothing useful to do about it.
        let _ = self.tx.send(cmd);
    }

    pub fn status(&self) -> Status {
        self.status.lock().clone()
    }
}

pub fn spawn(emit: Emitter) -> AudioHandle {
    let (tx, rx) = crossbeam_channel::unbounded();
    let status = Arc::new(Mutex::new(Status {
        state: PlayState::Stopped,
        index: None,
        track: None,
        position: 0.0,
        duration: None,
        volume: 0.8,
        repeat: Repeat::Off,
        queue_len: 0,
    }));
    let shared = status.clone();
    std::thread::Builder::new()
        .name("audio".into())
        .spawn(move || Engine::new(emit, shared).run(rx))
        .expect("spawn audio thread");
    AudioHandle { tx, status }
}

struct Engine {
    sink: Option<MixerDeviceSink>,
    player: Option<Player>,
    queue: Vec<Track>,
    index: Option<usize>,
    /// Queue index already appended behind the current track.
    preloaded: Option<usize>,
    /// Index whose preload failed, so we don't retry it every tick.
    preload_failed: Option<usize>,
    duration: Option<f64>,
    volume: f32,
    repeat: Repeat,
    level: Level,
    meter_on: bool,
    emit: Emitter,
    shared: Arc<Mutex<Status>>,
    last_position: Instant,
}

impl Engine {
    fn new(emit: Emitter, shared: Arc<Mutex<Status>>) -> Self {
        let volume = shared.lock().volume;
        Self {
            sink: None,
            player: None,
            queue: Vec::new(),
            index: None,
            preloaded: None,
            preload_failed: None,
            duration: None,
            volume,
            repeat: Repeat::Off,
            level: Level::default(),
            meter_on: false,
            emit,
            shared,
            last_position: Instant::now(),
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            match rx.recv_timeout(TICK) {
                Ok(cmd) => {
                    self.handle(cmd);
                    self.publish();
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => self.tick(),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Load { queue, index, autoplay } => {
                self.queue = queue;
                if self.queue.is_empty() {
                    self.stop();
                } else {
                    self.start(index.min(self.queue.len() - 1), autoplay);
                }
            }
            Command::Play => self.play(),
            Command::Pause => {
                if let Some(p) = &self.player {
                    p.pause();
                }
            }
            Command::Toggle => match self.state() {
                PlayState::Playing => {
                    if let Some(p) = &self.player {
                        p.pause();
                    }
                }
                _ => self.play(),
            },
            Command::Next => {
                if let Some(next) = self.next_index(true) {
                    if self.preloaded == Some(next) {
                        // Let the gapless hand-over machinery do the work.
                        if let Some(p) = &self.player {
                            p.skip_one();
                        }
                    } else {
                        self.start(next, self.state() != PlayState::Paused);
                    }
                }
            }
            Command::Prev => {
                let pos = self.position();
                match self.index {
                    Some(i) if pos < PREV_RESTART_AFTER && i > 0 => self.start(i - 1, self.state() != PlayState::Paused),
                    Some(_) => self.seek(0.0),
                    None => {}
                }
            }
            Command::PlayIndex(i) if i < self.queue.len() => self.start(i, true),
            Command::PlayIndex(_) => {}
            Command::Seek(secs) => self.seek(secs),
            Command::SetVolume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                if let Some(p) = &self.player {
                    p.set_volume(perceptual(self.volume));
                }
            }
            Command::SetRepeat(r) => {
                self.repeat = r;
                // The preloaded track may no longer be the right one.
                if self.preloaded.is_some() && self.preloaded != self.next_index(false) {
                    let pos = self.position();
                    let paused = self.state() == PlayState::Paused;
                    if let Some(i) = self.index {
                        self.start(i, !paused);
                        self.seek(pos);
                    }
                } else {
                    self.preload();
                }
            }
            Command::SetMeter(on) => {
                self.meter_on = on;
                if !on {
                    self.level.reset();
                }
            }
            Command::Stop => self.stop(),
        }
    }

    fn tick(&mut self) {
        let Some(player) = &self.player else { return };
        let len = player.len();

        if self.preloaded.is_some() && len == 1 {
            // Gapless hand-over happened.
            self.index = self.preloaded.take();
            self.duration = self.index.and_then(|i| self.queue[i].duration);
            self.preload();
            self.publish();
        } else if len == 0 && self.index.is_some() {
            // Reached the end of the queue.
            self.player = None;
            self.level.reset();
            self.publish();
            return;
        }

        if self.meter_on && self.state() == PlayState::Playing {
            (self.emit)(Event::Level { value: self.level.get() });
        }
        if self.last_position.elapsed() >= POSITION_EVERY && self.state() == PlayState::Playing {
            self.last_position = Instant::now();
            let position = self.position();
            self.shared.lock().position = position;
            (self.emit)(Event::Position { position, duration: self.duration });
        }
    }

    fn sink(&mut self) -> Option<&MixerDeviceSink> {
        if self.sink.is_none() {
            match DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    self.sink = Some(s);
                }
                Err(e) => self.error(None, format!("No audio output device: {e}")),
            }
        }
        self.sink.as_ref()
    }

    /// Start playing queue index `i` on a fresh player, skipping over files
    /// that fail to open.
    fn start(&mut self, mut i: usize, autoplay: bool) {
        self.player = None;
        self.preloaded = None;
        self.preload_failed = None;
        self.level.reset();
        let Some(sink) = self.sink() else { return };
        let player = Player::connect_new(sink.mixer());
        player.set_volume(perceptual(self.volume));
        if !autoplay {
            player.pause();
        }

        for _ in 0..self.queue.len() {
            match self.open(i) {
                Ok((src, dur)) => {
                    player.append(src);
                    self.index = Some(i);
                    self.duration = dur;
                    self.player = Some(player);
                    self.preload();
                    return;
                }
                Err(msg) => {
                    let path = self.queue[i].path.clone();
                    self.error(Some(path), msg);
                    i = (i + 1) % self.queue.len();
                }
            }
        }
        self.index = None;
    }

    fn preload(&mut self) {
        if self.preloaded.is_some() {
            return;
        }
        let Some(next) = self.next_index(false) else { return };
        if self.preload_failed == Some(next) {
            return;
        }
        match self.open(next) {
            Ok((src, _)) => {
                if let Some(p) = &self.player {
                    p.append(src);
                    self.preloaded = Some(next);
                }
            }
            Err(msg) => {
                self.preload_failed = Some(next);
                let path = self.queue[next].path.clone();
                self.error(Some(path), msg);
            }
        }
    }

    fn open(&self, i: usize) -> Result<(TrackSource, Option<f64>), String> {
        let track = &self.queue[i];
        let file = open_shared(&track.path).map_err(|e| format!("Can't open file: {e}"))?;
        let len = file.metadata().map(|m| m.len()).ok();
        let mut builder = Decoder::builder().with_data(BufReader::new(file)).with_seekable(true).with_gapless(true);
        if let Some(len) = len {
            builder = builder.with_byte_len(len);
        }
        if let Some(ext) = track.path.extension().and_then(|e| e.to_str()) {
            builder = builder.with_hint(ext);
        }
        let decoder = builder.build().map_err(|e| format!("Can't decode: {e}"))?;
        let duration = rodio::Source::total_duration(&decoder).map(|d| d.as_secs_f64()).or(track.duration);
        Ok((Meter::new(decoder, self.level.clone()), duration))
    }

    /// Next index after the current one. `manual` = user pressed Next, which
    /// moves on even in repeat-one mode.
    fn next_index(&self, manual: bool) -> Option<usize> {
        next_index(self.index?, self.queue.len(), self.repeat, manual)
    }

    fn play(&mut self) {
        match &self.player {
            Some(p) => p.play(),
            // Stopped at the end of the queue: start over from the current/first track.
            None if !self.queue.is_empty() => self.start(self.index.unwrap_or(0), true),
            None => {}
        }
    }

    fn seek(&mut self, secs: f64) {
        let Some(p) = &self.player else { return };
        let secs = match self.duration {
            Some(d) => secs.clamp(0.0, (d - 0.25).max(0.0)),
            None => secs.max(0.0),
        };
        if let Err(e) = p.try_seek(Duration::from_secs_f64(secs)) {
            self.error(self.index.map(|i| self.queue[i].path.clone()), format!("Seek failed: {e}"));
        }
        self.shared.lock().position = secs;
        (self.emit)(Event::Position { position: secs, duration: self.duration });
    }

    fn stop(&mut self) {
        self.player = None;
        self.preloaded = None;
        self.level.reset();
    }

    fn state(&self) -> PlayState {
        match &self.player {
            Some(p) if p.len() > 0 && p.is_paused() => PlayState::Paused,
            Some(p) if p.len() > 0 => PlayState::Playing,
            _ => PlayState::Stopped,
        }
    }

    fn position(&self) -> f64 {
        self.player.as_ref().map(|p| p.get_pos().as_secs_f64()).unwrap_or(0.0)
    }

    fn error(&self, path: Option<PathBuf>, message: String) {
        log::warn!("audio: {message} ({path:?})");
        (self.emit)(Event::Error { path, message });
    }

    fn publish(&self) {
        let status = Status {
            state: self.state(),
            index: self.index,
            track: self.index.map(|i| self.queue[i].clone()),
            position: self.position(),
            duration: self.duration,
            volume: self.volume,
            repeat: self.repeat,
            queue_len: self.queue.len(),
        };
        *self.shared.lock() = status.clone();
        (self.emit)(Event::Status(status));
    }
}

fn next_index(i: usize, len: usize, repeat: Repeat, manual: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match repeat {
        Repeat::One if !manual => Some(i),
        Repeat::All | Repeat::One => Some((i + 1) % len),
        Repeat::Off => (i + 1 < len).then_some(i + 1),
    }
}

/// Open for reading while letting other code rename or replace the file, so
/// the tag editor can save the track that is currently playing. The decoder
/// keeps reading the original data until the next track.
pub fn open_shared(path: &std::path::Path) -> std::io::Result<File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_SHARE_DELETE: u32 = 0x4;
        opts.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    opts.open(path)
}

/// Map a linear 0..1 slider to gain so the slider feels even across its range.
fn perceptual(v: f32) -> f32 {
    v * v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_index_rules() {
        assert_eq!(next_index(0, 3, Repeat::Off, false), Some(1));
        assert_eq!(next_index(2, 3, Repeat::Off, false), None);
        assert_eq!(next_index(2, 3, Repeat::All, false), Some(0));
        assert_eq!(next_index(1, 3, Repeat::One, false), Some(1), "repeat-one replays automatically");
        assert_eq!(next_index(2, 3, Repeat::One, true), Some(0), "Next button moves on and wraps");
        assert_eq!(next_index(0, 1, Repeat::All, false), Some(0));
        assert_eq!(next_index(0, 0, Repeat::All, false), None);
    }

    #[test]
    fn volume_curve() {
        assert_eq!(perceptual(0.0), 0.0);
        assert_eq!(perceptual(1.0), 1.0);
        assert!(perceptual(0.5) < 0.5);
    }
}
