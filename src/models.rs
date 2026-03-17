// src/models.rs
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

#[allow(dead_code)]
fn _log_models_module() {
    trace!("models module loaded");
    debug!("Photo and DeleteRequest types available");
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Photo {
    pub name: String,
    pub path: String, // Ensure this is String, not PathBuf
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub paths: Vec<String>,
}
