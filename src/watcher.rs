// src/watcher.rs
use crate::state::AppState;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;

pub fn init_watcher(state: AppState, path: PathBuf) {
    // FIX: Using a standard thread prevents the blocking receiver loop
    // from hanging integration tests.
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher.watch(&path, RecursiveMode::NonRecursive).is_err() {
            return;
        }

        for res in rx {
            match res {
                Ok(_) => state.notify(),
                Err(_) => break, // Exit loop if directory is dropped or inaccessible
            }
        }
    });
}
