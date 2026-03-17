// src/watcher.rs
use crate::state::AppState;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;

pub fn init_watcher(state: AppState, path: PathBuf) {
    use tracing::{debug, error, trace, warn};
    trace!("init_watcher called: path = {}", path.display().to_string());
    // FIX: Using a standard thread prevents the blocking receiver loop
    // from hanging integration tests.
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => {
                debug!("filesystem watcher created");
                w
            }
            Err(e) => {
                error!("failed to create filesystem watcher: {:?}", e);
                return;
            }
        };

        if watcher.watch(&path, RecursiveMode::NonRecursive).is_err() {
            warn!("failed to watch path: {}", path.display());
            return;
        }

        for res in rx {
            match res {
                Ok(ev) => {
                    debug!("watcher event received: {:?}", ev);
                    state.notify()
                }
                Err(e) => {
                    error!("watcher error: {:?}", e);
                    break; // Exit loop if directory is dropped or inaccessible
                }
            }
        }
        debug!("watcher thread exiting");
    });
}
