// src/api.rs
use crate::{
    error::AppResult,
    models::{DeleteRequest, Photo},
    processor::ImageProcessor,
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Query, Path, State, WebSocketUpgrade},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::path::Path as StdPath;
use tokio::fs;
use tokio_util::io::ReaderStream;
use sqlx::Row;
use std::collections::HashMap;


pub const PHOTOS_BASE_URL: &str = "/photos";

use tracing::{debug, info, trace, warn, error};

// Device detection helper - simple version based on User-Agent
#[derive(Debug, PartialEq)]
pub enum DeviceType {
    Mobile,
    Tablet,
    Desktop1080p,
    Desktop4K,
    Desktop8K,
    Unknown,
}

pub fn detect_device(user_agent: Option<&str>) -> DeviceType {
    let ua = user_agent.unwrap_or("").to_lowercase();

    if ua.contains("mobile") || ua.contains("android") || ua.contains("iphone") {
        DeviceType::Mobile
    } else if ua.contains("tablet") || ua.contains("ipad") {
        DeviceType::Tablet
    } else {
        // For desktop, we'd ideally detect screen size via JS,
        // but this is a server-side fallback. Default to web/desktop.
        DeviceType::Desktop1080p
    }
}

// Helper to get image dimensions from bytes - FIXED VERSION
async fn get_image_dimensions(data: &[u8]) -> AppResult<(u32, u32)> {
    let data_vec = data.to_vec();
    let result = tokio::task::spawn_blocking(move || {
        // Use image::load_from_memory to get dimensions
        match image::load_from_memory(&data_vec) {
            Ok(img) => Ok((img.width(), img.height())),
            Err(e) => {
                tracing::warn!("Could not read image dimensions: {}", e);
                Ok((0, 0))
            }
        }
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Blocking task failed: {}", e)))?;

    result
}

pub async fn get_photo(
    State(state): State<AppState>,
    Path(image_id): Path<String>,
    query: Query<HashMap<String, String>>,  // Remove Option, use Query directly
) -> AppResult<Response> {
    trace!(%image_id, "get_photo called with query: {:?}", query);
    
    // Check if a specific resolution was requested
    let requested_res = query
        .get("resolution")
        .cloned()
        .unwrap_or_else(|| {
            // Auto-detect based on user agent
            let user_agent = std::env::var("MOCK_USER_AGENT").ok();
            match detect_device(user_agent.as_deref()) {
                DeviceType::Mobile => "mobile".to_string(),
                DeviceType::Tablet => "web".to_string(),
                DeviceType::Desktop1080p => "desktop".to_string(),
                DeviceType::Desktop4K => "4k".to_string(),
                DeviceType::Desktop8K => "8k".to_string(),
                DeviceType::Unknown => "web".to_string(),
            }
        });
    
    debug!(%image_id, %requested_res, "requested resolution");
    
    // First, check if the image exists in the database
    let image_info: Option<(String, String, i32, i32)> = sqlx::query_as(
        "SELECT id, original_path, width, height FROM images WHERE id = ?1"
    )
    .bind(&image_id)
    .fetch_optional(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    let (_, original_path, width, height) = match image_info {
        Some(info) => info,
        None => {
            warn!(%image_id, "image not found in database");
            return Ok(StatusCode::NOT_FOUND.into_response());
        }
    };
    
    debug!(%image_id, %original_path, width, height, "found image in database");
    
    // Convert relative path to absolute if needed
    let storage_path = state.storage_path();
    let absolute_original_path = resolve_path(&original_path, storage_path);
    
    debug!(%image_id, abs_path = %absolute_original_path.display(), "resolved absolute path");
    
    // Check if original exists
    if !absolute_original_path.exists() {
        error!(%image_id, path = %absolute_original_path.display(), "original file not found");
        return Ok(StatusCode::NOT_FOUND.into_response());
    }
    
    // Query database for specific variant
    let variant_path: Option<String> = sqlx::query_scalar(
        "SELECT file_path FROM variants 
         WHERE image_id = ?1 AND resolution = ?2"
    )
    .bind(&image_id)
    .bind(&requested_res)
    .fetch_optional(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    // Resolve variant path if it exists
    let final_path = if let Some(variant_rel_path) = variant_path {
        let absolute_variant_path = resolve_path(&variant_rel_path, storage_path);
        
        if absolute_variant_path.exists() {
            debug!(%image_id, %requested_res, path = %absolute_variant_path.display(), "serving variant");
            absolute_variant_path
        } else {
            warn!(%image_id, variant_path = %absolute_variant_path.display(), "variant file not found, falling back to original");
            absolute_original_path
        }
    } else {
        debug!(%image_id, "no variant found for resolution {}, using original", requested_res);
        absolute_original_path
    };
    
    // Serve the file
    let file = fs::File::open(&final_path).await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    
    // Determine content type
    let content_type = if final_path.extension().and_then(|ext| ext.to_str()) == Some("jpg") ||
                          final_path.extension().and_then(|ext| ext.to_str()) == Some("jpeg") {
        "image/jpeg"
    } else if final_path.extension().and_then(|ext| ext.to_str()) == Some("png") {
        "image/png"
    } else if final_path.extension().and_then(|ext| ext.to_str()) == Some("webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    };
    
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, content_type)], body).into_response())
}

// Helper function to resolve relative paths
fn resolve_path(path_str: &str, storage_path: &std::path::Path) -> std::path::PathBuf {
    if path_str.starts_with("./") {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let clean_path = path_str.trim_start_matches("./");
        current_dir.join(clean_path)
    } else if path_str.starts_with('/') {
        std::path::PathBuf::from(path_str)
    } else if path_str.starts_with("photos/") {
        let current_dir = std::env::current_dir().unwrap_or_default();
        current_dir.join(path_str)
    } else {
        storage_path.join(path_str)
    }
}

pub async fn get_photo_by_filename(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> AppResult<Response> {
    trace!(%filename, "get_photo_by_filename called");
    
    // Find the image by filename
    let image_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM images WHERE original_filename = ?1 LIMIT 1"
    )
    .bind(&filename)
    .fetch_optional(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    match image_id {
        Some(id) => {
            // Redirect to the main photo endpoint
            let redirect_url = format!("/api/v1/photo/{}", id);
            Ok((StatusCode::FOUND, [(header::LOCATION, redirect_url)]).into_response())
        }
        None => {
            // Try to serve from flat directory as fallback
            let flat_path = state.storage_path().join(&filename);
            if flat_path.exists() && flat_path.is_file() {
                let file = fs::File::open(&flat_path).await?;
                let stream = ReaderStream::new(file);
                let body = Body::from_stream(stream);
                Ok((StatusCode::OK, body).into_response())
            } else {
                Ok(StatusCode::NOT_FOUND.into_response())
            }
        }
    }
}

pub async fn list_photos(State(state): State<AppState>) -> AppResult<Json<Vec<Photo>>> {
    trace!("list_photos called");
    
    // Query all images from database
    let rows = sqlx::query(
        "SELECT id, original_filename, uploaded_at, original_path, width, height FROM images ORDER BY uploaded_at DESC"
    )
    .fetch_all(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    let mut photos = Vec::new();
    
    for row in rows {
        let id: String = row.get(0);
        let original_filename: String = row.get(1);
        let uploaded_at: i64 = row.get(2);
        let original_path: String = row.get(3);
        
        // Try to find the best variant to display
        let variant_path: Option<String> = sqlx::query_scalar(
            "SELECT file_path FROM variants WHERE image_id = ?1 AND resolution IN ('web', 'mobile', 'desktop') ORDER BY resolution LIMIT 1"
        )
        .bind(&id)
        .fetch_optional(state.db())
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Variant query failed: {}", e)))?;
        
        // Use variant if available, otherwise use original
        let display_path = match variant_path {
            Some(path) => path,
            None => original_path,
        };
        
        // Get file size
        let size = match tokio::fs::metadata(&display_path).await {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                warn!(path = %display_path, error = %e, "Failed to get file size");
                0
            }
        };
        
        photos.push(Photo {
            name: original_filename,
            path: format!("/api/v1/photo/{}", id),
            size,
            modified: uploaded_at as u64,
        });
    }
    
    photos.sort_by(|a, b| b.modified.cmp(&a.modified));
    info!(count = photos.len(), "returning photo list from database");
    Ok(Json(photos))
}

pub async fn upload_photo(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    trace!("upload_photo called");
    let mut uploaded_ids = Vec::new();

    while let Some(field) = multipart.next_field().await? {
        let name = field.file_name().unwrap_or("unnamed").to_string();
        let data = field.bytes().await?;

        // Generate unique ID
        let image_id = uuid::Uuid::new_v4().to_string();

        // Sanitize filename and create original path
        let safe_name = StdPath::new(&name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let original_path = state
            .storage_path()
            .join("originals")
            .join(&image_id)
            .join(&safe_name);

        // Create directory
        if let Some(parent) = original_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Save original
        tokio::fs::write(&original_path, &data).await?;

        // Get image dimensions
        let (width, height) = get_image_dimensions(&data).await?;
        info!(%image_id, width, height, "original image dimensions");

        // Store in database - using sqlx::query (not query!)
        let original_path_str = original_path.to_str().unwrap_or("");
        let timestamp = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO images (id, original_filename, original_path, uploaded_at, mime_type, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )
        .bind(&image_id)
        .bind(&safe_name)
        .bind(original_path_str)
        .bind(timestamp)
        .bind("image/jpeg")
        .bind(width)
        .bind(height)
        .execute(state.db())
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Database insert failed: {}", e)))?;

        // Spawn async processing (don't block response)
        let state_clone = state.clone();
        let image_id_clone = image_id.clone();
        let original_path_clone = original_path.clone();

        tokio::spawn(async move {
            let processor = ImageProcessor::new(state_clone.storage_path().clone());

            match processor
                .process_upload(&image_id_clone, &original_path_clone)
                .await
            {
                Ok(variants) => {
                    info!(image_id = %image_id_clone, count = variants.len(), "processing variants");

                    // Store variants in database
                    for (resolution, path, size) in variants {
                        let path_str = path.to_str().unwrap_or("");
                        let variant_timestamp = chrono::Utc::now().timestamp();

                        if let Err(e) = sqlx::query(
                            "INSERT INTO variants (image_id, resolution, file_path, file_size, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)"
                        )
                        .bind(&image_id_clone)
                        .bind(&resolution)
                        .bind(path_str)
                        .bind(size as i64)
                        .bind(variant_timestamp)
                        .execute(state_clone.db())
                        .await
                        {
                            warn!(%image_id_clone, %resolution, error = %e, "failed to store variant");
                        } else {
                            debug!(%image_id_clone, %resolution, "variant stored");
                        }
                    }
                    info!(image_id = %image_id_clone, "All variants processed successfully");
                }
                Err(e) => {
                    warn!(image_id = %image_id_clone, error = %e, "Variant generation failed");
                }
            }
            state_clone.notify();
        });

        uploaded_ids.push(image_id);
    }

    if uploaded_ids.is_empty() {
        warn!("upload_photo: no fields found in multipart");
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No files uploaded"
            })),
        ));
    }

    info!(
        count = uploaded_ids.len(),
        "upload_photo: processing started"
    );
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "processing",
            "image_ids": uploaded_ids
        })),
    ))
}

pub async fn delete_photo(
    State(state): State<AppState>,
    Path(identifier): Path<String>,
) -> AppResult<StatusCode> {
    trace!(%identifier, "delete_photo called");
    
    // Check if this looks like a UUID (image ID) or a filename
    let is_uuid = identifier.contains('-') && identifier.len() >= 32;
    
    if is_uuid {
        // Delete by UUID - look up in database
        return delete_photo_by_id(State(state), Path(identifier)).await;
    }
    
    // Otherwise, treat as filename (legacy behavior)
    let path = state.storage_path().join(&identifier);
    debug!(path = %path.display(), "computed path for deletion (legacy)");
    
    if !path.starts_with(state.storage_path()) {
        warn!(%identifier, "attempt to delete outside storage_dir");
        return Ok(StatusCode::FORBIDDEN);
    }
    
    if path.exists() {
        tokio::fs::remove_file(path).await?;
        info!(%identifier, "file deleted");
        state.notify();
        Ok(StatusCode::OK)
    } else {
        warn!(%identifier, "delete requested but file not found");
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

pub async fn delete_photo_by_id(
    State(state): State<AppState>,
    Path(image_id): Path<String>,
) -> AppResult<StatusCode> {
    trace!(%image_id, "delete_photo_by_id called");
    
    // First, get the image info from database
    let image_info: Option<(String, String)> = sqlx::query_as(
        "SELECT original_filename, original_path FROM images WHERE id = ?1"
    )
    .bind(&image_id)
    .fetch_optional(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    let (original_filename, original_path) = match image_info {
        Some(info) => info,
        None => {
            warn!(%image_id, "image not found in database");
            return Ok(StatusCode::NOT_FOUND);
        }
    };
    
    debug!(%image_id, %original_filename, %original_path, "found image in database");
    
    // Resolve the original path
    let storage_path = state.storage_path();
    let absolute_original_path = resolve_path(&original_path, storage_path);
    
    // Delete original file if it exists
    if absolute_original_path.exists() {
        tokio::fs::remove_file(&absolute_original_path).await?;
        info!(%image_id, path = %absolute_original_path.display(), "deleted original file");
    } else {
        warn!(%image_id, path = %absolute_original_path.display(), "original file not found");
    }
    
    // Delete all variant files
    let variants: Vec<String> = sqlx::query_scalar(
        "SELECT file_path FROM variants WHERE image_id = ?1"
    )
    .bind(&image_id)
    .fetch_all(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Variant query failed: {}", e)))?;
    
    for variant_path in variants {
        let absolute_variant_path = resolve_path(&variant_path, storage_path);
        if absolute_variant_path.exists() {
            tokio::fs::remove_file(&absolute_variant_path).await?;
            debug!(%image_id, path = %absolute_variant_path.display(), "deleted variant file");
        }
    }
    
    // Delete the image directory (originals/{image_id}/) if it's empty
    let originals_dir = storage_path.join("originals").join(&image_id);
    if originals_dir.exists() && originals_dir.is_dir() {
        // Try to remove the directory (will only succeed if empty)
        if let Err(e) = tokio::fs::remove_dir(&originals_dir).await {
            debug!(%image_id, path = %originals_dir.display(), error = %e, "could not remove originals directory (may not be empty)");
        } else {
            info!(%image_id, path = %originals_dir.display(), "deleted originals directory");
        }
    }
    
    // Delete the variants directory (variants/{image_id}/) if it's empty
    let variants_dir = storage_path.join("variants").join(&image_id);
    if variants_dir.exists() && variants_dir.is_dir() {
        if let Err(e) = tokio::fs::remove_dir(&variants_dir).await {
            debug!(%image_id, path = %variants_dir.display(), error = %e, "could not remove variants directory");
        } else {
            info!(%image_id, path = %variants_dir.display(), "deleted variants directory");
        }
    }
    
    // Delete from database
    // Delete variants first (foreign key constraint)
    sqlx::query("DELETE FROM variants WHERE image_id = ?1")
        .bind(&image_id)
        .execute(state.db())
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Failed to delete variants: {}", e)))?;
    
    // Then delete the image
    sqlx::query("DELETE FROM images WHERE id = ?1")
        .bind(&image_id)
        .execute(state.db())
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Failed to delete image: {}", e)))?;
    
    info!(%image_id, %original_filename, "image and variants deleted successfully");
    state.notify();
    
    Ok(StatusCode::OK)
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
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

// src/api.rs - update get_variants to return file sizes
pub async fn get_variants(
    State(state): State<AppState>,
    Path(image_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    trace!(%image_id, "get_variants called");
    
    let rows = sqlx::query(
        "SELECT resolution, file_size FROM variants WHERE image_id = ?1"
    )
    .bind(&image_id)
    .fetch_all(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Database query failed: {}", e)))?;
    
    let mut variants = serde_json::Map::new();
    
    for row in rows {
        let resolution: String = row.get(0);
        let file_size: i64 = row.get(1);
        variants.insert(resolution, serde_json::json!({
            "size": file_size,
            "size_formatted": format_file_size(file_size as u64)
        }));
    }
    
    Ok(Json(serde_json::json!({
        "image_id": image_id,
        "variants": variants
    })))
}

// Helper for formatting file sizes
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub async fn debug_db(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let images: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, original_filename, original_path FROM images"
    )
    .fetch_all(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Query failed: {}", e)))?;
    
    let variants: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT image_id, resolution, file_path FROM variants"
    )
    .fetch_all(state.db())
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("Query failed: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "images": images,
        "variants": variants,
        "storage_path": state.storage_path().display().to_string()
    })))
}
