//! The music library: watched folders scanned into SQLite, with cached covers
//! and their palettes.

pub mod covers;
pub mod db;
pub mod scan;

use covers::CoverStore;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Library {
    conn: Mutex<Connection>,
    store: CoverStore,
    scanning: AtomicBool,
    cancel: AtomicBool,
}

/// Clears the "scanning" flag however the scan ends.
struct ScanGuard<'a>(&'a AtomicBool);

impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Library {
    /// Open (or create) the library under the app's local data directory.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| format!("Can't create {}: {e}", data_dir.display()))?;
        let conn = db::open(&data_dir.join("library.db")).map_err(|e| format!("Can't open library database: {e}"))?;
        let store = CoverStore::new(&data_dir.join("covers")).map_err(|e| format!("Can't create cover cache: {e}"))?;
        Ok(Self { conn: Mutex::new(conn), store, scanning: AtomicBool::new(false), cancel: AtomicBool::new(false) })
    }

    /// Recompute cached palettes if the extraction algorithm changed since
    /// they were made. Covers whose cached image is gone are dropped from the
    /// cache so the next scan re-extracts them from the audio files.
    pub fn upgrade_palettes(&self) -> Result<(), String> {
        if self.with(|c| db::meta_get(c, "palette_version"))? == Some(crate::palette::VERSION) {
            return Ok(());
        }
        let files = self.with(db::cover_files)?;
        log::info!("palette algorithm changed; recomputing {} palettes", files.len());
        use rayon::prelude::*;
        let results: Vec<(String, Result<String, String>)> = files
            .par_iter()
            .map(|(hash, full)| {
                let r = image::open(full)
                    .map_err(|e| e.to_string())
                    .and_then(|img| serde_json::to_string(&crate::palette::extract(&img)).map_err(|e| e.to_string()));
                (hash.clone(), r)
            })
            .collect();
        for (hash, r) in results {
            match r {
                Ok(json) => self.with(|c| db::set_palette(c, &hash, &json))?,
                Err(e) => {
                    log::warn!("cached cover {hash} is unreadable ({e}); it will be re-read on the next scan");
                    self.with(|c| {
                        c.execute("UPDATE tracks SET cover_hash = NULL, mtime = 0 WHERE cover_hash = ?1", [&hash])?;
                        c.execute("DELETE FROM covers WHERE hash = ?1", [&hash])
                    })?;
                }
            }
        }
        self.with(|c| db::meta_set(c, "palette_version", crate::palette::VERSION))
    }

    /// Run a read or small write against the database.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> db::Result<T>) -> Result<T, String> {
        f(&self.conn.lock()).map_err(|e| format!("Library database error: {e}"))
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    pub fn cancel_scan(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Scan all watched folders, or only `only` (folder ids). Blocks; call it
    /// from a background thread.
    pub fn scan(&self, only: Option<&[i64]>, progress: &dyn Fn(scan::Progress)) -> Result<scan::Summary, String> {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return Err("A scan is already running".into());
        }
        let _guard = ScanGuard(&self.scanning);
        self.cancel.store(false, Ordering::SeqCst);

        let folders = self.with(db::folders)?;
        let mut summary = scan::Summary::default();
        for folder in folders.iter().filter(|f| only.is_none_or(|ids| ids.contains(&f.id))) {
            scan::scan_folder(&self.conn, &self.store, folder.id, Path::new(&folder.path), &self.cancel, &mut summary, progress)?;
            if summary.cancelled {
                break;
            }
        }
        self.prune_covers();
        log::info!(
            "scan done: +{} ~{} -{} ={} unsupported={} failed={}",
            summary.added,
            summary.updated,
            summary.removed,
            summary.unchanged,
            summary.unsupported,
            summary.failed_count
        );
        Ok(summary)
    }

    /// Add a folder to the watch list. Rejects folders that are inside, or
    /// contain, one already watched, so files are never indexed twice.
    pub fn add_folder(&self, path: &Path) -> Result<db::Folder, String> {
        let path = std::fs::canonicalize(path).map_err(|e| format!("Can't open folder {}: {e}", path.display()))?;
        if !path.is_dir() {
            return Err(format!("{} is not a folder", path.display()));
        }
        let path = strip_verbatim(path);
        for f in self.with(db::folders)? {
            let existing = PathBuf::from(&f.path);
            if path.starts_with(&existing) {
                return Err(format!("{} is already in your library (inside {})", path.display(), f.path));
            }
            if existing.starts_with(&path) {
                return Err(format!("{} contains {}, which is already in your library. Remove it first.", path.display(), f.path));
            }
        }
        let path_str = path.to_string_lossy().into_owned();
        let id = self.with(|c| db::add_folder(c, &path_str))?;
        Ok(db::Folder { id, path: path_str, track_count: 0 })
    }

    pub fn remove_folder(&self, id: i64) -> Result<(), String> {
        if self.is_scanning() {
            return Err("Wait for the scan to finish before removing a folder".into());
        }
        self.with(|c| db::remove_folder(c, id))?;
        self.prune_covers();
        Ok(())
    }

    /// Re-read one file (after a tag edit) and update its row.
    pub fn refresh_file(&self, path: &Path) -> Result<(), String> {
        let path = &normalize_path(path);
        let path_str = path.to_string_lossy().into_owned();
        let Some(existing) = self.with(|c| db::track_by_path(c, &path_str))? else {
            return Ok(()); // Not in the library (played from outside it).
        };
        let folder_id = self.with(|c| {
            c.query_row("SELECT folder_id FROM tracks WHERE id = ?1", [existing.id], |r| r.get::<_, i64>(0))
        })?;
        let meta = std::fs::metadata(path).map_err(|e| format!("Can't read {}: {e}", path.display()))?;
        let (mut track, embedded) = scan::read(path, true)?;
        track.folder_id = folder_id;
        track.mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
        track.size = meta.len() as i64;

        let cover_bytes = match embedded {
            Some(b) => Some((b, "embedded")),
            None => path.parent().and_then(covers::find_in_folder).and_then(|p| std::fs::read(p).ok()).map(|b| (b, "folder")),
        };
        if let Some((bytes, source)) = cover_bytes {
            let hash = covers::hash(&bytes);
            if !self.with(|c| db::has_cover(c, &hash))? {
                let nc = self.store.process(&hash, &bytes)?;
                self.with(|c| db::insert_cover(c, &nc))?;
            }
            track.cover_hash = Some(hash);
            track.cover_source = Some(source.into());
        }
        self.with(|c| db::upsert_track(c, &track, scan::unix_now()))?;
        self.prune_covers();
        Ok(())
    }

    /// Cover and palette JSON for any audio file, in the library or not.
    /// Covers of files outside the library are cached but not recorded, and
    /// are swept on the next start.
    pub fn cover_for_path(&self, path: &Path) -> Result<Option<(db::Cover, String)>, String> {
        let path = &normalize_path(path);
        let path_str = path.to_string_lossy().into_owned();
        if let Some(t) = self.with(|c| db::track_by_path(c, &path_str))? {
            let Some(cover) = t.cover else { return Ok(None) };
            let palette = self.with(|c| db::palette_json(c, &cover.hash))?.ok_or("Cover palette is missing from the library")?;
            return Ok(Some((cover, palette)));
        }
        let (_, embedded) = scan::read(path, true)?;
        let bytes = match embedded {
            Some(b) => b,
            None => match path.parent().and_then(covers::find_in_folder) {
                Some(p) => std::fs::read(&p).map_err(|e| format!("Can't read {}: {e}", p.display()))?,
                None => return Ok(None),
            },
        };
        let hash = covers::hash(&bytes);
        let nc = self.store.process(&hash, &bytes)?;
        let cover = db::Cover { hash: nc.hash, full: nc.full, thumb: nc.thumb, width: nc.width, height: nc.height };
        Ok(Some((cover, nc.palette_json)))
    }

    /// Delete cached cover files the database doesn't know about (left over
    /// from files played outside the library, or an interrupted scan).
    pub fn sweep_cover_cache(&self) {
        match self.with(|c| {
            let mut stmt = c.prepare("SELECT hash FROM covers")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<db::Result<std::collections::HashSet<String>>>()
        }) {
            Ok(keep) => self.store.sweep(&keep),
            Err(e) => log::warn!("Cover cache sweep skipped: {e}"),
        }
    }

    fn prune_covers(&self) {
        match self.with(db::prune_covers) {
            Ok(files) => {
                for (full, thumb) in files {
                    for f in [full, thumb] {
                        if let Err(e) = std::fs::remove_file(&f) {
                            if e.kind() != std::io::ErrorKind::NotFound {
                                log::warn!("Can't delete cached cover {f}: {e}");
                            }
                        }
                    }
                }
            }
            Err(e) => log::warn!("Cover cleanup failed: {e}"),
        }
    }
}

/// The form paths are stored in: absolute, long names (no `PROGRA~1`), no
/// `\\?\` prefix. Falls back to the input if the file does not exist.
pub fn normalize_path(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(strip_verbatim).unwrap_or_else(|_| p.to_path_buf())
}

/// `canonicalize` on Windows returns `\\?\C:\...`; store the familiar form.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{self, CoverEdit, TagEdit};
    use crate::testutil::{temp_dir, wav_bytes};
    use std::fs;

    fn tag(path: &Path, title: &str, album: &str) {
        let edit = TagEdit {
            title: title.into(),
            artist: "Artist".into(),
            album: album.into(),
            album_artist: String::new(),
            genre: "House".into(),
            year: Some(2020),
            track_no: None,
            disc_no: None,
            cover: CoverEdit::Keep,
        };
        tags::write(path, &edit).unwrap();
    }

    #[test]
    fn scan_end_to_end() {
        let root = temp_dir("scan");
        let music = root.join("Music");
        let album = music.join("Album");
        fs::create_dir_all(&album).unwrap();
        for (i, name) in ["one.wav", "two.wav"].iter().enumerate() {
            let p = album.join(name);
            fs::write(&p, wav_bytes()).unwrap();
            tag(&p, &format!("Song {i}"), "Ping");
        }
        image::RgbImage::from_pixel(64, 64, image::Rgb([30, 60, 200])).save(album.join("cover.jpg")).unwrap();
        fs::write(music.join("loose.wav"), wav_bytes()).unwrap();
        fs::write(music.join("voice.opus"), b"x").unwrap();
        fs::write(music.join("broken.mp3"), b"not audio at all").unwrap();
        fs::write(music.join("notes.txt"), b"x").unwrap();

        let lib = Library::open(&root.join("data")).unwrap();
        lib.add_folder(&music).unwrap();
        assert!(lib.add_folder(&album).is_err(), "nested folder rejected");

        let s = lib.scan(None, &|_| {}).unwrap();
        assert_eq!((s.added, s.unsupported, s.failed_count), (3, 1, 1), "{s:?}");
        let stats = lib.with(db::stats).unwrap();
        assert_eq!((stats.tracks, stats.albums, stats.missing_covers), (3, 1, 1));

        // Folder cover was cached once and shared by both album tracks.
        let albums = lib.with(|c| db::albums(c, None, None)).unwrap();
        let cover = albums[0].cover.clone().expect("album has folder cover");
        assert!(Path::new(&cover.full).exists() && Path::new(&cover.thumb).exists());
        let (_, palette) = lib.cover_for_path(&album.join("one.wav")).unwrap().unwrap();
        assert!(palette.contains("\"glow\""));

        // A palette algorithm change recomputes cached palettes in place.
        lib.with(|c| {
            db::meta_set(c, "palette_version", 1)?;
            c.execute("UPDATE covers SET palette = '{}'", [])
        })
        .unwrap();
        lib.upgrade_palettes().unwrap();
        let json = lib.with(|c| db::palette_json(c, &cover.hash)).unwrap().unwrap();
        assert!(json.contains("\"glowStops\""), "palette recomputed: {json}");

        // Unchanged rescan does nothing.
        let s = lib.scan(None, &|_| {}).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (0, 0, 0, 3));

        // Edit, delete, and refresh after a tag edit.
        tag(&album.join("one.wav"), "Renamed", "Ping");
        fs::remove_file(music.join("loose.wav")).unwrap();
        let s = lib.scan(None, &|_| {}).unwrap();
        assert_eq!((s.updated, s.removed), (1, 1));
        let titles: Vec<String> = lib.with(|c| db::tracks(c, &db::TrackFilter::default())).unwrap().into_iter().map(|t| t.title).collect();
        assert!(titles.contains(&"Renamed".to_string()));

        tag(&album.join("two.wav"), "Edited In App", "Ping");
        lib.refresh_file(&album.join("two.wav")).unwrap();
        let two = normalize_path(&album.join("two.wav"));
        let t = lib.with(|c| db::track_by_path(c, &two.to_string_lossy())).unwrap().unwrap();
        assert_eq!(t.title, "Edited In App");

        // An unreachable folder keeps its tracks.
        let moved = root.join("Moved");
        fs::rename(&music, &moved).unwrap();
        let s = lib.scan(None, &|_| {}).unwrap();
        assert_eq!(s.missing_folders.len(), 1);
        assert_eq!(lib.with(db::stats).unwrap().tracks, 2);

        // Removing the folder drops its tracks and cached covers.
        let id = lib.with(db::folders).unwrap()[0].id;
        lib.remove_folder(id).unwrap();
        assert_eq!(lib.with(db::stats).unwrap().tracks, 0);
        assert!(!Path::new(&cover.full).exists());
        drop(lib);
        let _ = fs::remove_dir_all(&root);
    }
}
