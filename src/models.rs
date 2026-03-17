// src/models.rs
use serde::{Deserialize, Serialize};

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
