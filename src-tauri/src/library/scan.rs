//! Folder scanning: walk, read tags in parallel, cache covers, write to the DB.
//!
//! Scans are incremental: files whose modification time and size are
//! unchanged since the last scan are skipped.

use super::covers::{self, CoverStore};
use super::db::{self, NewCover, NewTrack};
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use lofty::probe::Probe;
use parking_lot::Mutex;
use rayon::prelude::*;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Extensions we can both tag-read and play.
pub const PLAYABLE_EXTS: &[&str] = &["mp3", "m4a", "m4b", "mp4", "aac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc"];
/// Audio we recognise but can't decode yet; counted and reported, not added.
const UNSUPPORTED_EXTS: &[&str] = &["opus", "ape", "mpc", "wv", "spx", "wma", "dsf", "dff"];

const CHUNK: usize = 64;
/// Cap on individual failure messages kept for the summary.
const MAX_REPORTED_FAILURES: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub folder: String,
    pub phase: Phase,
    /// Audio files found so far in this folder.
    pub found: usize,
    /// Files that needed reading (new or changed).
    pub total: usize,
    pub processed: usize,
    /// Of the files read so far: with and without artwork.
    pub with_cover: usize,
    pub no_cover: usize,
    /// The last file read, for the scanning screen.
    pub file: String,
}

/// Where cover art comes from when a file has both embedded art and a
/// folder image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverSource {
    #[default]
    Embedded,
    Folder,
    EmbeddedOnly,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Discovering,
    Reading,
    Cleaning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Files in formats we can't play (Opus, APE, WMA, ...), not added.
    pub unsupported: usize,
    pub failed_count: usize,
    pub failures: Vec<Failure>,
    /// Watched folders that weren't reachable (e.g. unplugged drive). Their
    /// tracks are kept, not removed.
    pub missing_folders: Vec<String>,
    pub cancelled: bool,
}

impl Summary {
    fn fail(&mut self, path: &Path, message: String) {
        log::warn!("scan: {}: {message}", path.display());
        self.failed_count += 1;
        if self.failures.len() < MAX_REPORTED_FAILURES {
            self.failures.push(Failure { path: path.to_string_lossy().into_owned(), message });
        }
    }
}

struct Found {
    path: PathBuf,
    mtime: i64,
    size: i64,
}

/// Per-directory cache of folder cover art: (hash, bytes), or None if the
/// directory has no cover image.
type FolderCovers = Mutex<HashMap<PathBuf, Option<(String, Arc<Vec<u8>>)>>>;
type ReadResult = (NewTrack, Option<CoverBytes>);

struct CoverBytes {
    hash: String,
    bytes: Arc<Vec<u8>>,
    source: &'static str,
}

/// Scan one watched folder into the database.
/// How a scan runs: cancellation flag and cover preference.
pub struct ScanOptions<'a> {
    pub cancel: &'a AtomicBool,
    pub cover_source: CoverSource,
}

pub fn scan_folder(
    conn: &Mutex<Connection>,
    store: &CoverStore,
    folder_id: i64,
    root: &Path,
    opts: &ScanOptions,
    summary: &mut Summary,
    progress: &dyn Fn(Progress),
) -> Result<(), String> {
    let (cancel, cover_source) = (opts.cancel, opts.cover_source);
    let folder = root.to_string_lossy().into_owned();
    #[allow(clippy::too_many_arguments)]
    let p = |phase, found, total, processed, with_cover, no_cover, file: String| Progress {
        folder: folder.clone(),
        phase,
        found,
        total,
        processed,
        with_cover,
        no_cover,
        file,
    };
    if !root.is_dir() {
        summary.missing_folders.push(folder);
        return Ok(());
    }

    // 1. Discover.
    let mut found = Vec::new();
    for entry in WalkDir::new(root).follow_links(true) {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            return Ok(());
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let path = e.path().map(Path::to_path_buf).unwrap_or_else(|| root.to_path_buf());
                summary.fail(&path, format!("Can't read: {e}"));
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if UNSUPPORTED_EXTS.contains(&ext.as_str()) {
            summary.unsupported += 1;
            continue;
        }
        if !PLAYABLE_EXTS.contains(&ext.as_str()) {
            continue;
        }
        match entry.metadata() {
            Ok(m) => found.push(Found { path: entry.into_path(), mtime: mtime(&m), size: m.len() as i64 }),
            Err(e) => summary.fail(entry.path(), format!("Can't read file info: {e}")),
        }
        if found.len() % 500 == 0 {
            progress(p(Phase::Discovering, found.len(), 0, 0, 0, 0, String::new()));
        }
    }

    // 2. Diff against what we know.
    let known = db::known_files(&conn.lock(), folder_id).map_err(|e| e.to_string())?;
    let seen: HashSet<String> = found.iter().map(|f| f.path.to_string_lossy().into_owned()).collect();
    let mut todo = Vec::new();
    for f in found.iter() {
        match known.get(f.path.to_string_lossy().as_ref()) {
            Some(&(m, s)) if m == f.mtime && s == f.size => summary.unchanged += 1,
            _ => todo.push(f),
        }
    }
    let total = todo.len();
    progress(p(Phase::Reading, found.len(), total, 0, 0, 0, String::new()));
    let (mut with_cover, mut no_cover) = (0usize, 0usize);

    // 3. Read tags + covers in parallel, write each chunk in one transaction.
    let folder_covers: FolderCovers = Mutex::new(HashMap::new());
    let mut processed = 0;
    for chunk in todo.chunks(CHUNK) {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            return Ok(());
        }
        let results: Vec<(&Found, Result<ReadResult, String>)> =
            chunk.par_iter().map(|f| (*f, read_file(f, folder_id, &folder_covers, cover_source))).collect();
        for (_, r) in &results {
            match r {
                Ok((_, Some(_))) => with_cover += 1,
                Ok((_, None)) => no_cover += 1,
                Err(_) => {}
            }
        }
        let last_file = chunk.last().map(|f| f.path.to_string_lossy().into_owned()).unwrap_or_default();

        // Cache covers we haven't seen before, in parallel.
        let mut new_hashes = HashSet::new();
        let mut to_cache = Vec::new();
        {
            let c = conn.lock();
            for (_, r) in &results {
                if let Ok((_, Some(cb))) = r {
                    if new_hashes.insert(cb.hash.clone()) && !db::has_cover(&c, &cb.hash).map_err(|e| e.to_string())? {
                        to_cache.push((cb.hash.clone(), cb.bytes.clone()));
                    }
                }
            }
        }
        let cached: Vec<(String, Result<NewCover, String>)> =
            to_cache.par_iter().map(|(h, b)| (h.clone(), store.process(h, b))).collect();
        let mut bad_covers = HashSet::new();

        let now = unix_now();
        let mut c = conn.lock();
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for (hash, r) in &cached {
            match r {
                Ok(nc) => db::insert_cover(&tx, nc).map_err(|e| e.to_string())?,
                Err(_) => {
                    bad_covers.insert(hash.clone());
                }
            }
        }
        for (f, r) in results {
            match r {
                Ok((mut track, cover)) => {
                    if let Some(cb) = cover {
                        if bad_covers.contains(&cb.hash) {
                            let msg = cached.iter().find(|(h, _)| *h == cb.hash).and_then(|(_, r)| r.as_ref().err()).cloned();
                            summary.fail(&f.path, format!("Cover art is unreadable: {}", msg.unwrap_or_default()));
                        } else {
                            track.cover_hash = Some(cb.hash);
                            track.cover_source = Some(cb.source.to_string());
                        }
                    }
                    let is_new = !known.contains_key(&track.path);
                    db::upsert_track(&tx, &track, now).map_err(|e| e.to_string())?;
                    if is_new {
                        summary.added += 1;
                    } else {
                        summary.updated += 1;
                    }
                }
                Err(msg) if msg == UNSUPPORTED => summary.unsupported += 1,
                Err(msg) => summary.fail(&f.path, msg),
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        drop(c);

        processed += chunk.len();
        progress(p(Phase::Reading, found.len(), total, processed, with_cover, no_cover, last_file));
    }

    // 4. Forget files that disappeared.
    progress(p(Phase::Cleaning, found.len(), total, processed, with_cover, no_cover, String::new()));
    let gone: Vec<&String> = known.keys().filter(|p| !seen.contains(*p)).collect();
    if !gone.is_empty() {
        let mut c = conn.lock();
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for p in &gone {
            db::remove_track_path(&tx, p).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        summary.removed += gone.len();
    }
    Ok(())
}

const UNSUPPORTED: &str = "unsupported";

fn read_file(
    f: &Found,
    folder_id: i64,
    folder_covers: &FolderCovers,
    cover_source: CoverSource,
) -> Result<ReadResult, String> {
    let (mut track, embedded) = read(&f.path, true)?;
    track.folder_id = folder_id;
    track.mtime = f.mtime;
    track.size = f.size;

    let from_file = || embedded.clone().map(|bytes| CoverBytes { hash: covers::hash(&bytes), bytes: Arc::new(bytes), source: "embedded" });
    let from_folder = || {
        let dir = f.path.parent()?;
        let cached = folder_covers.lock().get(dir).cloned();
        let entry = match cached {
            Some(e) => e,
            None => {
                let e = covers::find_in_folder(dir).and_then(|p| std::fs::read(p).ok()).map(|b| (covers::hash(&b), Arc::new(b)));
                folder_covers.lock().insert(dir.to_path_buf(), e.clone());
                e
            }
        };
        entry.map(|(hash, bytes)| CoverBytes { hash, bytes, source: "folder" })
    };
    let cover = match cover_source {
        CoverSource::Embedded => from_file().or_else(from_folder),
        CoverSource::Folder => from_folder().or_else(from_file),
        CoverSource::EmbeddedOnly => from_file(),
    };
    Ok((track, cover))
}

/// Cover bytes for one file, honoring the cover source preference.
pub fn cover_bytes(path: &Path, embedded: Option<Vec<u8>>, source: CoverSource) -> Option<(Vec<u8>, &'static str)> {
    let from_folder = || {
        let p = covers::find_in_folder(path.parent()?)?;
        std::fs::read(p).ok().map(|b| (b, "folder"))
    };
    let from_file = || embedded.clone().map(|b| (b, "embedded"));
    match source {
        CoverSource::Embedded => from_file().or_else(from_folder),
        CoverSource::Folder => from_folder().or_else(from_file),
        CoverSource::EmbeddedOnly => from_file(),
    }
}

/// Read tags and audio properties into a track record, plus the embedded
/// cover bytes when `with_cover` is set.
pub fn read(path: &Path, with_cover: bool) -> Result<(NewTrack, Option<Vec<u8>>), String> {
    let tagged = Probe::open(path)
        .map_err(|e| format!("Can't open: {e}"))?
        .options(ParseOptions::new().read_cover_art(with_cover))
        .guess_file_type()
        .map_err(|e| format!("Can't open: {e}"))?
        .read()
        .map_err(|e| format!("Can't read tags: {e}"))?;
    if matches!(tagged.file_type(), FileType::Opus | FileType::Speex | FileType::Ape | FileType::Mpc | FileType::WavPack) {
        return Err(UNSUPPORTED.into());
    }

    let props = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let text = |v: Option<std::borrow::Cow<'_, str>>| v.map(|s| s.trim().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    let title = tag.map(|t| text(t.title())).filter(|s| !s.is_empty()).unwrap_or(stem);
    let cover = if with_cover { tag.and_then(covers::embedded) } else { None };
    let track = NewTrack {
        path: path.to_string_lossy().into_owned(),
        folder_id: 0,
        mtime: 0,
        size: 0,
        title,
        artist: tag.map(|t| text(t.artist())).unwrap_or_default(),
        album: tag.map(|t| text(t.album())).unwrap_or_default(),
        album_artist: tag.and_then(|t| t.get_string(ItemKey::AlbumArtist)).map(|s| s.trim().to_string()).unwrap_or_default(),
        genre: tag.map(|t| text(t.genre())).unwrap_or_default(),
        year: tag.and_then(|t| t.date()).map(|d| d.year as i64).filter(|y| *y > 0),
        track_no: tag.and_then(|t| t.track()).map(i64::from),
        disc_no: tag.and_then(|t| t.disk()).map(i64::from),
        duration: props.duration().as_secs_f64(),
        bitrate: props.audio_bitrate().or(props.overall_bitrate()).map(i64::from),
        sample_rate: props.sample_rate().map(i64::from),
        channels: props.channels().map(i64::from),
        bit_depth: props.bit_depth().map(i64::from),
        format: format_name(path, tagged.file_type()),
        cover_hash: None,
        cover_source: None,
    };
    Ok((track, cover))
}

fn format_name(path: &Path, ft: FileType) -> String {
    match ft {
        FileType::Mpeg => "MP3".into(),
        FileType::Flac => "FLAC".into(),
        FileType::Vorbis => "Ogg Vorbis".into(),
        FileType::Wav => "WAV".into(),
        FileType::Aiff => "AIFF".into(),
        FileType::Aac => "AAC".into(),
        _ => path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_else(|| "Audio".into()),
    }
}

fn mtime(m: &std::fs::Metadata) -> i64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
