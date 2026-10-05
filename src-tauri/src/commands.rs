//! Commands the frontend calls via `invoke`, plus the background scan job.
//!
//! Events emitted to the frontend:
//! - `player`: [`audio::Event`] (status, position, level, error)
//! - `scan-progress`: [`scan::Progress`]
//! - `scan-done`: [`ScanDone`]
//! - `library-changed`: [`LibraryChanged`]

use crate::audio::{self, AudioHandle, Command, Repeat, Track};
use crate::library::{self, db, scan, Library};
use crate::settings::{Settings, SettingsStore};
use crate::tags::{self, TagEdit, TagInfo};
use crate::watcher::FolderWatcher;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub data_dir: PathBuf,
    pub media: Arc<std::sync::OnceLock<crate::media::MediaHandle>>,
    pub audio: AudioHandle,
    pub library: Arc<Library>,
    pub watcher: Mutex<Option<FolderWatcher>>,
    pub settings: Mutex<Settings>,
    pub settings_store: SettingsStore,
    pub scan_job: Arc<AtomicBool>,
}

type Res<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDone {
    pub summary: Option<scan::Summary>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChanged {
    /// Files whose tags/cover changed, when known; empty means "anything".
    pub paths: Vec<String>,
}

/// Start a background scan unless one is already running. Returns whether it
/// started.
pub fn start_scan(app: &AppHandle, only: Option<Vec<i64>>) -> bool {
    let state = app.state::<AppState>();
    if state.scan_job.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (library, job, app) = (state.library.clone(), state.scan_job.clone(), app.clone());
    let spawned = std::thread::Builder::new().name("library-scan".into()).spawn(move || {
        let result = library.scan(only.as_deref(), &|p| {
            let _ = app.emit("scan-progress", p);
        });
        job.store(false, Ordering::SeqCst);
        let done = match result {
            Ok(summary) => ScanDone { summary: Some(summary), error: None },
            Err(e) => {
                log::error!("scan failed: {e}");
                ScanDone { summary: None, error: Some(e) }
            }
        };
        let _ = app.emit("scan-done", &done);
        let _ = app.emit("library-changed", LibraryChanged { paths: vec![] });
        // Folders that were unreachable may be back; re-arm their watches.
        sync_watcher(&app);
    });
    if let Err(e) = spawned {
        state.scan_job.store(false, Ordering::SeqCst);
        log::error!("can't start scan thread: {e}");
        return false;
    }
    true
}

pub fn sync_watcher(app: &AppHandle) {
    let state = app.state::<AppState>();
    match state.library.with(db::folders) {
        Ok(folders) => {
            if let Some(w) = state.watcher.lock().as_mut() {
                w.sync(&folders);
            }
        }
        Err(e) => log::warn!("can't sync folder watcher: {e}"),
    }
}

// ---- Library ---------------------------------------------------------------

#[tauri::command]
pub fn library_stats(state: State<'_, AppState>) -> Res<db::Stats> {
    state.library.with(db::stats)
}

#[tauri::command]
pub fn library_folders(state: State<'_, AppState>) -> Res<Vec<db::Folder>> {
    state.library.with(db::folders)
}

#[tauri::command]
pub fn library_add_folder(app: AppHandle, state: State<'_, AppState>, path: PathBuf) -> Res<db::Folder> {
    let folder = state.library.add_folder(&path)?;
    sync_watcher(&app);
    if !start_scan(&app, Some(vec![folder.id])) {
        // A scan is running; the watcher or the next rescan picks it up.
        log::info!("scan busy; {} will be scanned on the next pass", folder.path);
    }
    Ok(folder)
}

#[tauri::command]
pub fn library_remove_folder(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    state.library.remove_folder(id)?;
    sync_watcher(&app);
    let _ = app.emit("library-changed", LibraryChanged { paths: vec![] });
    Ok(())
}

/// Returns false if a scan is already running.
#[tauri::command]
pub fn library_rescan(app: AppHandle) -> bool {
    start_scan(&app, None)
}

#[tauri::command]
pub fn library_cancel_scan(state: State<'_, AppState>) {
    state.library.cancel_scan();
}

#[tauri::command]
pub fn library_is_scanning(state: State<'_, AppState>) -> bool {
    state.scan_job.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn library_albums(state: State<'_, AppState>, artist: Option<String>, search: Option<String>) -> Res<Vec<db::AlbumRow>> {
    state.library.with(|c| db::albums(c, artist.as_deref(), search.as_deref()))
}

#[tauri::command]
pub fn library_artists(state: State<'_, AppState>, search: Option<String>) -> Res<Vec<db::NamedCount>> {
    state.library.with(|c| db::artists(c, search.as_deref()))
}

#[tauri::command]
pub fn library_genres(state: State<'_, AppState>, search: Option<String>) -> Res<Vec<db::NamedCount>> {
    state.library.with(|c| db::genres(c, search.as_deref()))
}

#[tauri::command]
pub fn library_tracks(state: State<'_, AppState>, filter: db::TrackFilter) -> Res<Vec<db::TrackRow>> {
    state.library.with(|c| db::tracks(c, &filter))
}

#[tauri::command]
pub fn library_track_by_path(state: State<'_, AppState>, path: PathBuf) -> Res<Option<db::TrackRow>> {
    let p = library::normalize_path(&path);
    state.library.with(|c| db::track_by_path(c, &p.to_string_lossy()))
}

// ---- Covers & palettes -----------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverInfo {
    pub cover: Option<db::Cover>,
    pub palette: serde_json::Value,
}

/// Cover + palette for any audio file. Files without art get the neutral
/// fallback palette and `cover: null`.
#[tauri::command]
pub async fn cover_for_path(state: State<'_, AppState>, path: PathBuf) -> Res<CoverInfo> {
    let library = state.library.clone();
    tauri::async_runtime::spawn_blocking(move || match library.cover_for_path(&path)? {
        Some((cover, palette)) => Ok(CoverInfo { cover: Some(cover), palette: serde_json::from_str(&palette).map_err(|e| e.to_string())? }),
        None => Ok(CoverInfo { cover: None, palette: serde_json::to_value(crate::palette::fallback()).map_err(|e| e.to_string())? }),
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Neutral palette for tracks without cover art (and the empty player).
#[tauri::command]
pub fn palette_fallback() -> crate::palette::Palette {
    crate::palette::fallback()
}

#[tauri::command]
pub fn cover_palette(state: State<'_, AppState>, hash: String) -> Res<serde_json::Value> {
    let json = state.library.with(|c| db::palette_json(c, &hash))?.ok_or("Unknown cover")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

/// Small JPEG preview of any image file as a data URL (for picking a new
/// cover; the webview can't read arbitrary files directly).
#[tauri::command]
pub async fn image_preview(path: PathBuf) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine;
        let img = image::open(&path).map_err(|e| format!("Can't open image: {e}"))?;
        let thumb = img.resize(360, 360, image::imageops::FilterType::Triangle).to_rgb8();
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&thumb).map_err(|e| e.to_string())?;
        Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Palette of any image file, so the tag editor can preview a new cover's
/// colors before saving.
#[tauri::command]
pub async fn image_palette(path: PathBuf) -> Res<crate::palette::Palette> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = image::open(&path).map_err(|e| format!("Can't open image: {e}"))?;
        Ok(crate::palette::extract(&img))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- Shell (tray, taskbar, media overlay fallback) ---------------------------

#[tauri::command]
pub fn shell_update(state: State<'_, AppState>, shell: State<'_, crate::shell::Shell>, update: crate::shell::ShellUpdate) -> Res<()> {
    let track = update.path.clone();
    let fallback = shell.update(update)?;
    if let (Some(track), Some(image), Some(media)) = (track, fallback, state.media.get()) {
        media.fallback(PathBuf::from(track), image);
    }
    Ok(())
}

/// Open Windows' Default Apps page on Chameleon's entry, where the user can
/// make it the default player (apps can't do that themselves on Windows 10/11).
#[tauri::command]
pub fn open_default_apps() -> Res<()> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let program_files = [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)")];
    let machine = program_files.iter().flatten().any(|p| exe.starts_with(p));
    let url = format!(
        "ms-settings:defaultapps?registeredApp{}={}",
        if machine { "Machine" } else { "User" },
        urlencoding::encode("Chameleon Player")
    );
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| format!("Can't open Windows Settings: {e}"))
}

// ---- Online covers ---------------------------------------------------------

#[tauri::command]
pub async fn covers_find(artist: String, album: String, title: String) -> Res<Vec<crate::online::Candidate>> {
    tauri::async_runtime::spawn_blocking(move || crate::online::find(&artist, &album, &title, 3)).await.map_err(|e| e.to_string())?
}

/// Download a candidate cover into the cache; returns its local path.
#[tauri::command]
pub async fn covers_download(state: State<'_, AppState>, url: String) -> Res<String> {
    let dir = state.data_dir.join("covers").join("downloads");
    tauri::async_runtime::spawn_blocking(move || crate::online::download(&url, &dir).map(|p| p.to_string_lossy().into_owned()))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCoverResult {
    /// Tracks whose files now embed the cover.
    pub written: usize,
    /// Tracks updated in the library only (writing was off, or the file is
    /// read-only).
    pub library_only: usize,
    pub failures: Vec<String>,
    pub folder_image: Option<String>,
}

/// Give every track of an album a new cover. With `write_files`, the art is
/// embedded in each file and saved as folder.jpg (unless one exists);
/// otherwise, and for files that can't be written, only the library changes.
#[tauri::command]
pub async fn album_set_cover(app: AppHandle, state: State<'_, AppState>, album_key: String, image: PathBuf, write_files: bool) -> Res<AlbumCoverResult> {
    let library = state.library.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Res<AlbumCoverResult> {
        let tracks = library.with(|c| db::tracks(c, &db::TrackFilter { album_key: Some(album_key), ..Default::default() }))?;
        if tracks.is_empty() {
            return Err("That album has no songs in the library.".into());
        }
        let mut out = AlbumCoverResult::default();
        for t in &tracks {
            let path = PathBuf::from(&t.path);
            let edit = TagEdit {
                title: t.title.clone(),
                artist: t.artist.clone(),
                album: t.album.clone(),
                album_artist: t.album_artist.clone(),
                genre: t.genre.clone(),
                year: t.year.map(|y| y as u32),
                track_no: t.track_no.map(|n| n as u32),
                disc_no: t.disc_no.map(|n| n as u32),
                cover: crate::tags::CoverEdit::Replace { path: image.clone() },
            };
            if write_files {
                // Keep every existing tag as it is in the file; only the art changes.
                let from_file = tags::read(&path).map(|i| TagEdit {
                    title: i.title,
                    artist: i.artist,
                    album: i.album,
                    album_artist: i.album_artist,
                    genre: i.genre,
                    year: i.year,
                    track_no: i.track_no,
                    disc_no: i.disc_no,
                    cover: edit.cover.clone(),
                });
                match from_file.and_then(|e| tags::write(&path, &e)) {
                    Ok(()) => {
                        if let Err(e) = library.refresh_file(&path) {
                            out.failures.push(format!("{}: saved, but the library couldn't update: {e}", t.title));
                        }
                        out.written += 1;
                        continue;
                    }
                    Err(e) => log::warn!("album cover: can't write {}: {e}; saving to library only", t.path),
                }
            }
            match library.update_track_only(&path, &edit) {
                Ok(()) => out.library_only += 1,
                Err(e) => out.failures.push(format!("{}: {e}", t.title)),
            }
        }
        if write_files {
            if let Some(dir) = PathBuf::from(&tracks[0].path).parent() {
                if library::covers::find_in_folder(dir).is_none() {
                    let target = dir.join("folder.jpg");
                    let saved = image::open(&image)
                        .map_err(|e| e.to_string())
                        .and_then(|img| img.to_rgb8().save_with_format(&target, image::ImageFormat::Jpeg).map_err(|e| e.to_string()));
                    match saved {
                        Ok(()) => out.folder_image = Some(target.to_string_lossy().into_owned()),
                        Err(e) => out.failures.push(format!("folder.jpg: {e}")),
                    }
                }
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("library-changed", LibraryChanged { paths: vec![] });
    Ok(result)
}

// ---- Tags ------------------------------------------------------------------

#[tauri::command]
pub async fn tags_read(path: PathBuf) -> Res<TagInfo> {
    tauri::async_runtime::spawn_blocking(move || tags::read(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn tags_write(app: AppHandle, state: State<'_, AppState>, path: PathBuf, edit: TagEdit) -> Res<()> {
    let library = state.library.clone();
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tags::write(&p, &edit)?;
        // The file is saved; a failed library refresh shouldn't read as a
        // failed save, but the user should know the library may be stale.
        if let Err(e) = library.refresh_file(&p) {
            log::warn!("saved tags but library refresh failed for {}: {e}", p.display());
            return Err(format!("Saved, but the library couldn't be updated: {e}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("library-changed", LibraryChanged { paths: vec![library::normalize_path(&path).to_string_lossy().into_owned()] });
    Ok(())
}

// ---- Player ----------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    PlayIndex { index: usize },
    Seek { position: f64 },
    SetVolume { volume: f32 },
    SetRepeat { repeat: Repeat },
    SetMeter { enabled: bool },
}

#[tauri::command]
pub fn player_load(state: State<'_, AppState>, queue: Vec<Track>, index: usize, autoplay: bool) {
    state.audio.send(Command::Load { queue, index, autoplay });
}

#[tauri::command]
pub fn player(state: State<'_, AppState>, command: PlayerCommand) -> Res<()> {
    let cmd = match command {
        PlayerCommand::Play => Command::Play,
        PlayerCommand::Pause => Command::Pause,
        PlayerCommand::Toggle => Command::Toggle,
        PlayerCommand::Next => Command::Next,
        PlayerCommand::Prev => Command::Prev,
        PlayerCommand::Stop => Command::Stop,
        PlayerCommand::PlayIndex { index } => Command::PlayIndex(index),
        PlayerCommand::Seek { position } => Command::Seek(position),
        PlayerCommand::SetVolume { volume } => {
            let volume = volume.clamp(0.0, 1.0);
            update_settings(&state, |s| s.volume = volume)?;
            Command::SetVolume(volume)
        }
        PlayerCommand::SetRepeat { repeat } => {
            update_settings(&state, |s| s.repeat = repeat)?;
            Command::SetRepeat(repeat)
        }
        PlayerCommand::SetMeter { enabled } => Command::SetMeter(enabled),
    };
    state.audio.send(cmd);
    Ok(())
}

#[tauri::command]
pub fn player_status(state: State<'_, AppState>) -> audio::Status {
    state.audio.status()
}

/// Insert tracks into the queue at `at` (e.g. right after the current track)
/// and optionally play the first of them.
#[tauri::command]
pub fn player_insert(state: State<'_, AppState>, tracks: Vec<Track>, at: usize, play: bool) {
    state.audio.send(Command::Insert { tracks, at, play });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub path: String,
    pub id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub cover: Option<db::Cover>,
}

/// The play queue with display details. Library tracks come from the
/// database; files from outside it are read for their tags.
#[tauri::command]
pub async fn player_queue(state: State<'_, AppState>) -> Res<Vec<QueueItem>> {
    let (library, queue) = (state.library.clone(), state.audio.queue());
    tauri::async_runtime::spawn_blocking(move || {
        queue
            .into_iter()
            .map(|t| {
                let path = t.path.to_string_lossy().into_owned();
                if let Some(row) = library.with(|c| db::track_by_path(c, &path))? {
                    return Ok(QueueItem {
                        path,
                        id: Some(row.id),
                        title: row.title,
                        artist: row.artist,
                        album: row.album,
                        duration: row.duration,
                        cover: row.cover,
                    });
                }
                let (title, artist, album, duration) = match scan::read(&t.path, false) {
                    Ok((n, _)) => (n.title, n.artist, n.album, n.duration),
                    Err(_) => (t.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), String::new(), String::new(), 0.0),
                };
                Ok(QueueItem { path, id: None, title, artist, album, duration, cover: None })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Everything the UI needs on startup, in one round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: Settings,
    pub status: audio::Status,
    pub stats: db::Stats,
    pub scanning: bool,
    pub fallback: crate::palette::Palette,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Res<Bootstrap> {
    Ok(Bootstrap {
        settings: state.settings.lock().clone(),
        status: state.audio.status(),
        stats: state.library.with(db::stats)?,
        scanning: state.scan_job.load(Ordering::SeqCst),
        fallback: crate::palette::fallback(),
    })
}

/// Change where covers come from and re-read the library with it.
#[tauri::command]
pub fn library_set_cover_source(app: AppHandle, state: State<'_, AppState>, source: scan::CoverSource) -> Res<()> {
    update_settings(&state, |s| s.cover_source = source)?;
    if state.library.set_cover_source(source) {
        state.library.invalidate_all()?;
        if !start_scan(&app, None) {
            log::info!("scan busy; cover source change applies on the next scan");
        }
    }
    Ok(())
}

/// "Save to library only": update the library's copy of a track's tags (and
/// optionally its cover) without touching the file, for read-only files.
#[tauri::command]
pub async fn library_update_track(app: AppHandle, state: State<'_, AppState>, path: PathBuf, edit: TagEdit) -> Res<()> {
    let library = state.library.clone();
    let p = library::normalize_path(&path);
    tauri::async_runtime::spawn_blocking(move || library.update_track_only(&p, &edit)).await.map_err(|e| e.to_string())??;
    let _ = app.emit("library-changed", LibraryChanged { paths: vec![library::normalize_path(&path).to_string_lossy().into_owned()] });
    Ok(())
}

/// Play files and/or folders (drag & drop, "Open With", command line).
/// Folders are expanded to their playable files, sorted by path.
#[tauri::command]
pub fn open_paths(state: State<'_, AppState>, paths: Vec<PathBuf>) -> Res<usize> {
    let queue = tracks_for_paths(&state.library, &paths);
    if queue.is_empty() {
        return Err("None of these files can be played".into());
    }
    let n = queue.len();
    state.audio.send(Command::Load { queue, index: 0, autoplay: true });
    Ok(n)
}

pub fn tracks_for_paths(library: &Library, paths: &[PathBuf]) -> Vec<Track> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut found: Vec<PathBuf> = walkdir::WalkDir::new(p)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && is_playable(e.path()))
                .map(|e| e.into_path())
                .collect();
            found.sort_by_key(|f| f.to_string_lossy().to_lowercase());
            files.extend(found);
        } else if p.is_file() && is_playable(p) {
            files.push(p.clone());
        }
    }
    files
        .into_iter()
        .map(|f| {
            let path = library::normalize_path(&f);
            let row = library.with(|c| db::track_by_path(c, &path.to_string_lossy())).ok().flatten();
            Track { id: row.as_ref().map(|r| r.id), duration: row.map(|r| r.duration).filter(|d| *d > 0.0), path }
        })
        .collect()
}

fn is_playable(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| scan::PLAYABLE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

// ---- Settings --------------------------------------------------------------

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings.lock().clone()
}

/// Replace the frontend-owned UI preferences.
#[tauri::command]
pub fn settings_set_ui(state: State<'_, AppState>, ui: serde_json::Map<String, serde_json::Value>) -> Res<()> {
    update_settings(&state, |s| s.ui = ui)
}

fn update_settings(state: &AppState, f: impl FnOnce(&mut Settings)) -> Res<()> {
    let mut s = state.settings.lock();
    f(&mut s);
    state.settings_store.save(&s).map_err(|e| format!("Can't save settings: {e}"))
}
