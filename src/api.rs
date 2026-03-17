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

pub async fn list_photos(State(state): State<AppState>) -> AppResult<Json<Vec<Photo>>> {
    let mut entries = fs::read_dir(state.storage_path()).await?;
    let mut photos = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            let filename = entry.file_name().to_string_lossy().to_string();

            // This construction ensures the string is formatted exactly as the frontend expects
            let web_path = format!("{}/{}", PHOTOS_BASE_URL, filename);

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
        }
    }

    photos.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(Json(photos))
}

pub async fn upload_photo(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    while let Some(field) = multipart.next_field().await? {
        let name = field.file_name().unwrap_or("unnamed").to_string();
        let data = field.bytes().await?;

        let safe_name = StdPath::new(&name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let path = state.storage_path().join(safe_name);
        fs::write(path, data).await?;
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
    let path = state.storage_path().join(&filename);
    if !path.starts_with(state.storage_path()) {
        return Ok(StatusCode::FORBIDDEN);
    }
    if path.exists() {
        fs::remove_file(path).await?;
        state.notify();
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

pub async fn delete_photos(
    State(state): State<AppState>,
    Json(req): Json<DeleteRequest>,
) -> AppResult<StatusCode> {
    for name in req.paths {
        let path = state.storage_path().join(&name);
        if path.starts_with(state.storage_path()) && path.exists() {
            fs::remove_file(path).await?;
        }
    }
    state.notify();
    Ok(StatusCode::OK)
}

pub async fn ws_handler(
    State(state): State<AppState>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        let mut rx = state.subscribe();
        while rx.recv().await.is_ok() {
            if socket
                .send(axum::extract::ws::Message::Text("update".into()))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}
