// src/lib.rs
pub mod api;
pub mod config;
pub mod error;
pub mod models;
pub mod processor;
pub mod service;
pub mod state;
pub mod watcher;

// Re-export everything needed by main.rs and integration tests at the root level.
// This allows tests to use `use gallery_rs::{Config, AppState, GalleryService};`
pub use crate::config::Config;
pub use crate::error::{AppError, AppResult};
pub use crate::service::GalleryService;
pub use crate::state::AppState;

pub async fn run(config: crate::config::Config) -> miette::Result<()> {
    tracing::trace!("run() called: config = {:?}", &config);
    let service = GalleryService::new(config).await?;
    tracing::debug!("service created, entering run");
    service
        .run()
        .await
        .map_err(|e| miette::miette!(e.to_string()))?;
    tracing::info!("run() completed successfully");
    Ok(())
}
