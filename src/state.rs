// src/state.rs
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    storage_dir: PathBuf,
    tx: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(storage_dir: PathBuf) -> Self {
        let (tx, _) = broadcast::channel(16);
        Self {
            inner: Arc::new(AppStateInner { storage_dir, tx }),
        }
    }

    pub fn storage_path(&self) -> &PathBuf {
        &self.inner.storage_dir
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.inner.tx.subscribe()
    }

    pub fn notify(&self) {
        let _ = self.inner.tx.send(());
    }
}
