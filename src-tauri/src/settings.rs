//! Persistent settings in `settings.json` under the app's data directory.
//!
//! Playback settings are typed; UI preferences (glow intensity, pulse, ...)
//! are kept as an opaque JSON object owned by the frontend.

use crate::audio::Repeat;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub volume: f32,
    pub repeat: Repeat,
    pub cover_source: crate::library::scan::CoverSource,
    pub ui: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { volume: 0.8, repeat: Repeat::Off, cover_source: Default::default(), ui: serde_json::Map::new() }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("settings.json") }
    }

    /// Load settings; a missing file gives defaults, a corrupt one is kept
    /// aside as `settings.json.bad` and defaults are used.
    pub fn load(&self) -> Settings {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(e) => {
                log::warn!("Can't read settings ({e}); using defaults");
                return Settings::default();
            }
        };
        match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Settings file is corrupt ({e}); keeping it as settings.json.bad and using defaults");
                let _ = std::fs::rename(&self.path, self.path.with_extension("json.bad"));
                Settings::default()
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        crate::library::covers::write_atomic(&self.path, text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = temp_dir("settings");
        let store = SettingsStore::new(&dir);
        assert_eq!(store.load().volume, 0.8);

        let mut s = Settings { volume: 0.3, repeat: Repeat::All, ..Default::default() };
        s.ui.insert("glow".into(), serde_json::json!(0.7));
        store.save(&s).unwrap();
        let back = store.load();
        assert_eq!((back.volume, back.repeat), (0.3, Repeat::All));
        assert_eq!(back.ui["glow"], serde_json::json!(0.7));

        std::fs::write(dir.join("settings.json"), "{ nope").unwrap();
        assert_eq!(store.load().volume, 0.8);
        assert!(dir.join("settings.json.bad").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
