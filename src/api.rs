// src/api.rs
use crate::{
    error::AppResult,
    models::{DeleteRequest, Photo},
    state::AppState,
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::path::Path as StdPath;
use tokio::fs;

pub const PHOTOS_BASE_URL: &str = "/photos";

use tracing::{debug, info, trace, warn};

pub async fn list_photos(State(state): State<AppState>) -> AppResult<Json<Vec<Photo>>> {
    trace!("list_photos called");
    let mut entries = fs::read_dir(state.storage_path()).await?;
    let mut photos = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            let filename = entry.file_name().to_string_lossy().to_string();
            debug!(%filename, "found file");

            // This construction ensures the string is formatted exactly as the frontend expects
            let web_path = format!("{}/{}", PHOTOS_BASE_URL, filename);
            trace!(%web_path, size = metadata.len(), "constructed web path");

            photos.push(Photo {
                name: filename,
                path: web_path,
                size: metadata.len(),
                modified: metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
        } else {
            trace!("skipped non-file entry");
        }
    }

    photos.sort_by(|a, b| b.modified.cmp(&a.modified));
    info!(count = photos.len(), "returning photo list");
    Ok(Json(photos))
}

pub async fn upload_photo(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    trace!("upload_photo called");
    let mut found = 0usize;
    while let Some(field) = multipart.next_field().await? {
        let name = field.file_name().unwrap_or("unnamed").to_string();
        trace!(%name, "processing multipart field");
        let data = field.bytes().await?;

        let safe_name = StdPath::new(&name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let path = state.storage_path().join(safe_name.clone());
        debug!(path = %path.display(), "writing uploaded file");
        fs::write(path, data).await?;
        found += 1;
    }

    if found == 0 {
        warn!("upload_photo: no fields found in multipart");
    } else {
        info!(uploaded = found, "upload_photo: files written");
    }

    state.notify();
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "success"})),
    ))
}

pub async fn delete_photo(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> AppResult<StatusCode> {
    trace!(%filename, "delete_photo called");
    let path = state.storage_path().join(&filename);
    debug!(path = %path.display(), "computed path for deletion");
    if !path.starts_with(state.storage_path()) {
        warn!(%filename, "attempt to delete outside storage_dir");
        return Ok(StatusCode::FORBIDDEN);
    }
    if path.exists() {
        fs::remove_file(path).await?;
        info!(%filename, "file deleted");
        state.notify();
        Ok(StatusCode::OK)
    } else {
        warn!(%filename, "delete requested but file not found");
        Ok(StatusCode::NOT_FOUND)
    }
}

pub async fn delete_photos(
    State(state): State<AppState>,
    Json(req): Json<DeleteRequest>,
) -> AppResult<StatusCode> {
    trace!("delete_photos called");
    for name in req.paths {
        let path = state.storage_path().join(&name);
        debug!(%name, path = %path.display(), "attempting to delete");
        if path.starts_with(state.storage_path()) && path.exists() {
            fs::remove_file(path).await?;
            info!(%name, "deleted");
        } else {
            warn!(%name, "could not delete (missing or outside storage)");
        }
    }
    state.notify();
    Ok(StatusCode::OK)
}

pub async fn ws_handler(
    State(state): State<AppState>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl IntoResponse {
    trace!("ws_handler upgrade requested");
    ws.on_upgrade(|mut socket| async move {
        let mut rx = state.subscribe();
        while rx.recv().await.is_ok() {
            trace!("broadcast received, sending websocket update");
            if socket
                .send(axum::extract::ws::Message::Text("update".into()))
                .await
                .is_err()
            {
                warn!("failed to send websocket message, closing");
                break;
            }
        }
        debug!("websocket handler exiting");
    })
}
