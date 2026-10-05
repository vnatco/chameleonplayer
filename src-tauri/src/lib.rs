pub mod audio;
pub mod commands;
pub mod defaults;
pub mod library;
pub mod media;
pub mod online;
pub mod palette;
pub mod settings;
pub mod shell;
pub mod tags;
pub mod watcher;
pub mod window;
#[cfg(test)]
mod testutil;

use commands::AppState;
use library::{db, Library};
use parking_lot::Mutex;
use settings::SettingsStore;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch (e.g. "Open With") hands its files
        // to the running instance instead of starting another player.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            open_args(app, &argv[1..]);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            setup(app.handle()).map_err(|e| {
                log::error!("startup failed: {e}");
                e
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_stats,
            commands::library_folders,
            commands::library_add_folder,
            commands::library_remove_folder,
            commands::library_rescan,
            commands::library_cancel_scan,
            commands::library_is_scanning,
            commands::library_albums,
            commands::library_artists,
            commands::library_genres,
            commands::library_tracks,
            commands::library_track_by_path,
            commands::cover_for_path,
            commands::cover_palette,
            commands::palette_fallback,
            commands::image_preview,
            commands::image_palette,
            commands::covers_find,
            commands::covers_download,
            commands::album_set_cover,
            commands::tags_read,
            commands::tags_write,
            commands::player_load,
            commands::player,
            commands::player_status,
            commands::open_paths,
            commands::settings_get,
            commands::settings_set_ui,
            commands::bootstrap,
            commands::player_insert,
            commands::player_queue,
            commands::library_set_cover_source,
            commands::library_update_track,
            window::window_hit,
            window::window_frame,
            window::window_set_frame,
            window::window_chrome,
            window::window_set_on_top,
            window::window_toggle_maximize,
            window::window_show,
            commands::shell_update,
            commands::open_default_apps,
            commands::default_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Chameleon Player");
}

fn setup(app: &AppHandle) -> Result<(), String> {
    let data_dir = app.path().app_local_data_dir().map_err(|e| format!("No app data folder: {e}"))?;
    let library = Arc::new(Library::open(&data_dir)?);
    if let Err(e) = library.upgrade_palettes() {
        log::error!("palette upgrade failed: {e}");
    }
    library.sweep_cover_cache();
    {
        let lib = library.clone();
        std::thread::Builder::new()
            .name("fallback-covers".into())
            .spawn(move || lib.prepare_fallbacks())
            .map_err(|e| format!("Can't start fallback covers: {e}"))?;
    }
    // Let the webview load cached covers (and nothing else) from disk.
    app.asset_protocol_scope()
        .allow_directory(data_dir.join("covers"), true)
        .map_err(|e| format!("Can't allow cover cache: {e}"))?;

    let settings_store = SettingsStore::new(&data_dir);
    let settings = settings_store.load();
    library.set_cover_source(settings.cover_source);

    // Audio events go to the UI and to the Windows media overlay. The overlay
    // needs the audio handle for media keys, so it's attached afterwards.
    let media: Arc<OnceLock<media::MediaHandle>> = Arc::default();
    let (emit_app, emit_media) = (app.clone(), media.clone());
    let audio = audio::spawn(Arc::new(move |event| {
        media::forward(&emit_media.get().cloned(), &event);
        let _ = emit_app.emit("player", &event);
    }));
    audio.send(audio::Command::SetVolume(settings.volume));
    audio.send(audio::Command::SetRepeat(settings.repeat));

    if let Some(window) = app.get_webview_window("main") {
        app.manage(window::spawn_hit_test(app, window.clone()));
        match window.hwnd() {
            Ok(hwnd) => match media::spawn(hwnd.0, audio.clone(), library.clone()) {
                Ok(handle) => {
                    let _ = media.set(handle);
                }
                Err(e) => log::warn!("{e}"),
            },
            Err(e) => log::warn!("Media overlay unavailable: no window handle ({e})"),
        }
    }

    let watcher_app = app.clone();
    let watcher = match watcher::FolderWatcher::spawn(move |ids| commands::start_scan(&watcher_app, Some(ids))) {
        Ok(w) => Some(w),
        Err(e) => {
            log::warn!("{e}; library will only update on manual rescan");
            None
        }
    };

    // Downloaded cover candidates are temporary.
    let downloads = data_dir.join("covers").join("downloads");
    if downloads.exists() {
        if let Err(e) = std::fs::remove_dir_all(&downloads) {
            log::warn!("can't clear old cover downloads: {e}");
        }
    }

    shell::Shell::clear_fallbacks(&data_dir);
    match shell::Shell::new(app, audio.clone(), &data_dir) {
        Ok(s) => {
            app.manage(s);
        }
        Err(e) => return Err(format!("Can't set up the tray and taskbar: {e}")),
    }

    app.manage(AppState {
        data_dir: data_dir.clone(),
        media: media.clone(),
        audio,
        library: library.clone(),
        watcher: Mutex::new(watcher),
        settings: Mutex::new(settings),
        settings_store,
        scan_job: Arc::new(AtomicBool::new(false)),
    });
    commands::sync_watcher(app);

    // Catch up on anything that changed while the app was closed.
    if library.with(db::folders).map(|f| !f.is_empty()).unwrap_or(false) {
        commands::start_scan(app, None);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    open_args(app, &args);
    Ok(())
}

/// Play files/folders passed on the command line.
fn open_args(app: &AppHandle, args: &[String]) {
    let paths: Vec<PathBuf> = args.iter().filter(|a| !a.starts_with('-')).map(PathBuf::from).filter(|p| p.exists()).collect();
    if paths.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let queue = commands::tracks_for_paths(&state.library, &paths);
    if queue.is_empty() {
        log::warn!("none of the opened files can be played: {paths:?}");
        let _ = app.emit("app-error", "None of these files can be played");
        return;
    }
    state.audio.send(audio::Command::Load { queue, index: 0, autoplay: true });
}
