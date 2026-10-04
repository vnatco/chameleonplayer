//! A pass-through `Source` that measures loudness, mostly in the bass, to
//! drive the optional "music pulse" on the cover glow.

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Shared meter reading, 0..1, stored as `f32` bits.
#[derive(Clone, Default)]
pub struct Level(Arc<AtomicU32>);

impl Level {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn reset(&self) {
        self.0.store(0f32.to_bits(), Ordering::Relaxed);
    }

    fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
}

pub struct Meter<S> {
    inner: S,
    level: Level,
    /// One-pole low-pass state; keeps roughly < 150 Hz.
    lp: f32,
    lp_alpha: f32,
    acc: f32,
    n: u32,
    window: u32,
    smoothed: f32,
}

impl<S: Source> Meter<S> {
    pub fn new(inner: S, level: Level) -> Self {
        let rate = inner.sample_rate().get() as f32;
        let channels = inner.channels().get() as u32;
        let dt = 1.0 / rate;
        let rc = 1.0 / (2.0 * std::f32::consts::PI * 150.0);
        Self {
            inner,
            level,
            lp: 0.0,
            lp_alpha: dt / (rc + dt),
            acc: 0.0,
            n: 0,
            // ~16 ms of audio per reading.
            window: ((rate / 60.0) as u32 * channels).max(64),
            smoothed: 0.0,
        }
    }
}

impl<S: Source> Iterator for Meter<S> {
    type Item = S::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let s = self.inner.next()?;
        let x = s;
        self.lp += self.lp_alpha * (x - self.lp);
        self.acc += self.lp * self.lp;
        self.n += 1;
        if self.n >= self.window {
            let rms = (self.acc / self.n as f32).sqrt();
            // Bass RMS of loud masters sits around 0.2-0.35; scale into 0..1.
            let v = (rms * 3.0).min(1.0);
            // Fast attack, slow release, so the pulse feels like a beat.
            let k = if v > self.smoothed { 0.6 } else { 0.12 };
            self.smoothed += k * (v - self.smoothed);
            self.level.set(self.smoothed);
            self.acc = 0.0;
            self.n = 0;
        }
        Some(s)
    }
}

impl<S: Source> Source for Meter<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)
    }
}
