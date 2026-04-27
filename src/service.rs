// src/service.rs - UPDATED to handle async state creation
use crate::{api, config::Config, state::AppState, watcher};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
};
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

pub struct GalleryService {
    pub state: AppState,
    pub config: Config,
}

impl GalleryService {
    pub async fn new(config: Config) -> miette::Result<Self> {
        use tracing::{debug, info, trace};
        trace!("GalleryService::new called");
        if !config.storage_dir.exists() {
            debug!(path = %config.storage_dir.display(), "storage_dir does not exist, creating");
            tokio::fs::create_dir_all(&config.storage_dir)
                .await
                .map_err(|e| miette::miette!("Failed to create storage directory: {}", e))?;
            info!("storage_dir created");
        }

        // AppState::new is now async and returns Result
        let state = AppState::new(config.storage_dir.clone())
            .await
            .map_err(|e| miette::miette!("Failed to create app state: {}", e))?;

        // Initialize watcher with the state
        watcher::init_watcher(state.clone(), config.storage_dir.clone());

        Ok(Self { state, config })
    }

    pub async fn run(self) -> Result<(), std::io::Error> {
        let addr: SocketAddr = self
            .config
            .server_addr()
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let app = self.into_router();

        tracing::info!("GALLERY_RS is live at http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await
    }

    pub fn into_router(self) -> Router {
        Router::new()
            .route("/api/v1/photos", get(api::list_photos))
            .route("/api/v1/images", get(api::list_photos))
            .route("/api/v1/upload", post(api::upload_photo))
            .route("/api/v1/delete-multiple", post(api::delete_photos))
            .route("/api/v1/photos/{filename}", delete(api::delete_photo))
            .route("/api/v1/photo/{image_id}", get(api::get_photo))
            .route("/api/v1/photo/{image_id}/variants", get(api::get_variants))
            .route("/api/v1/debug", get(api::debug_db))
            .route("/photos/{filename}", get(api::get_photo_by_filename))
            .route("/ws", get(api::ws_handler))
            //.nest_service("/photos", ServeDir::new(self.state.storage_path()))
            .fallback_service(ServeDir::new("templates"))
            // INCREASE BODY LIMIT: Axum defaults to 2MB, which causes "NetworkError" on large photos
            .layer(DefaultBodyLimit::max(250 * 1024 * 1024))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone())
    }
}
