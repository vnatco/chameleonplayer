//! Built-in fallback covers for music without art.
//!
//! The images are embedded in the app. Each album (or loose song) gets one,
//! picked from a hash of its key, so it's random across the library but
//! stable: the same album always shows the same fallback. They are marked as
//! placeholders, so the library still counts the music as missing a cover.

use super::covers::{self, CoverStore};
use super::db::Cover;
use rayon::prelude::*;

const IMAGES: [&[u8]; 7] = [
    include_bytes!("../../assets/fallback-covers/01.jpg"),
    include_bytes!("../../assets/fallback-covers/02.jpg"),
    include_bytes!("../../assets/fallback-covers/03.jpg"),
    include_bytes!("../../assets/fallback-covers/04.jpg"),
    include_bytes!("../../assets/fallback-covers/05.jpg"),
    include_bytes!("../../assets/fallback-covers/06.jpg"),
    include_bytes!("../../assets/fallback-covers/07.jpg"),
];

/// Hashes of the embedded images (cache file names), for the cache sweep.
pub fn static_hashes() -> impl Iterator<Item = String> {
    IMAGES.iter().map(|b| covers::hash(b))
}

/// Prepared fallbacks: cached files plus palette JSON.
pub struct Fallbacks {
    items: Vec<(Cover, String)>,
}

impl Fallbacks {
    /// Cache and analyse the embedded images (in parallel; a few tens of ms).
    pub fn prepare(store: &CoverStore) -> Self {
        let items = IMAGES
            .par_iter()
            .filter_map(|bytes| {
                let hash = covers::hash(bytes);
                match store.process(&hash, bytes) {
                    Ok(nc) => Some((
                        Cover {
                            hash: nc.hash,
                            full: nc.full,
                            thumb: nc.thumb,
                            width: nc.width,
                            height: nc.height,
                            edge: nc.edge,
                            glow: nc.glow,
                            placeholder: true,
                        },
                        nc.palette_json,
                    )),
                    Err(e) => {
                        log::warn!("fallback cover {hash} unusable: {e}");
                        None
                    }
                }
            })
            .collect();
        Self { items }
    }

    pub fn hashes(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|(c, _)| c.hash.as_str())
    }

    /// The fallback for `key` (an album key, or a path for loose files).
    pub fn pick(&self, key: &str) -> Option<&(Cover, String)> {
        if self.items.is_empty() {
            return None;
        }
        let h = blake3::hash(key.as_bytes());
        let n = u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap());
        self.items.get((n % self.items.len() as u64) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn stable_and_spread() {
        let dir = temp_dir("fallbacks");
        let store = CoverStore::new(&dir).unwrap();
        let f = Fallbacks::prepare(&store);
        assert_eq!(f.items.len(), IMAGES.len());
        assert!(f.items.iter().all(|(c, p)| c.placeholder && p.contains("glowStops")));
        let a = f.pick("artist\u{1f}album").unwrap().0.hash.clone();
        assert_eq!(f.pick("artist\u{1f}album").unwrap().0.hash, a, "same key, same cover");
        let distinct: std::collections::HashSet<_> = (0..200).map(|i| f.pick(&format!("album {i}")).unwrap().0.hash.clone()).collect();
        assert!(distinct.len() >= 5, "picks spread across the set: {}", distinct.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
