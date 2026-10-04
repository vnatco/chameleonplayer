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
