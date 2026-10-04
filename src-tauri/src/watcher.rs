//! Watches library folders and triggers an incremental rescan of a folder a
//! few seconds after its files stop changing (so copying an album in causes
//! one scan, not hundreds).

use crate::library::db::Folder;
use crossbeam_channel::RecvTimeoutError;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const QUIET_PERIOD: Duration = Duration::from_secs(3);

pub struct FolderWatcher {
    watcher: RecommendedWatcher,
    watched: Arc<Mutex<HashMap<PathBuf, i64>>>,
}

impl FolderWatcher {
    /// `rescan` is called with folder ids that changed; it returns false if
    /// it couldn't start (e.g. a scan is already running), and is retried.
    pub fn spawn(rescan: impl Fn(Vec<i64>) -> bool + Send + 'static) -> Result<Self, String> {
        let (tx, rx) = crossbeam_channel::unbounded::<PathBuf>();
        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                    for p in event.paths {
                        let _ = tx.send(p);
                    }
                }
            }
            Err(e) => log::warn!("folder watcher: {e}"),
        })
        .map_err(|e| format!("Can't watch folders for changes: {e}"))?;

        let watched: Arc<Mutex<HashMap<PathBuf, i64>>> = Arc::default();
        let map = watched.clone();
        std::thread::Builder::new()
            .name("folder-watcher".into())
            .spawn(move || {
                let mut dirty: HashMap<i64, Instant> = HashMap::new();
                loop {
                    match rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(path) => {
                            let owner = map.lock().iter().filter(|(root, _)| path.starts_with(root)).max_by_key(|(root, _)| root.as_os_str().len()).map(|(_, id)| *id);
                            if let Some(id) = owner {
                                dirty.insert(id, Instant::now());
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    let ready: Vec<i64> = dirty.iter().filter(|(_, t)| t.elapsed() >= QUIET_PERIOD).map(|(id, _)| *id).collect();
                    if !ready.is_empty() && rescan(ready.clone()) {
                        for id in ready {
                            dirty.remove(&id);
                        }
                    }
                }
            })
            .map_err(|e| format!("Can't start folder watcher: {e}"))?;

        Ok(Self { watcher, watched })
    }

    /// Make the watch list match the library's folders.
    pub fn sync(&mut self, folders: &[Folder]) {
        let wanted: HashMap<PathBuf, i64> = folders.iter().map(|f| (PathBuf::from(&f.path), f.id)).collect();
        let mut watched = self.watched.lock();
        for root in watched.keys().filter(|r| !wanted.contains_key(*r)).cloned().collect::<Vec<_>>() {
            if let Err(e) = self.watcher.unwatch(&root) {
                log::warn!("folder watcher: can't unwatch {}: {e}", root.display());
            }
            watched.remove(&root);
        }
        for (root, id) in wanted {
            if watched.contains_key(&root) {
                continue;
            }
            // A missing folder (unplugged drive) is retried on the next sync.
            match self.watcher.watch(&root, RecursiveMode::Recursive) {
                Ok(()) => {
                    watched.insert(root, id);
                }
                Err(e) => log::warn!("folder watcher: can't watch {}: {e}", root.display()),
            }
        }
    }
}
