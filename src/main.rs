use axum::{
    Router,
    extract::{
        DefaultBodyLimit, Multipart as AxumMultipart, Path as AxumPath, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
};
use clap::Parser;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use walkdir::{DirEntry, WalkDir};

/// Defines the structure for real-time filesystem change notifications.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
enum ChangeType {
    Added,
    Updated,
    Removed,
}

/// The payload sent to connected WebSocket clients when the gallery contents change.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct GalleryEvent {
    change_type: ChangeType,
    rel_path: String,
}

/// Command-line configuration for the server instance.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "A secure, real-time photo gallery server in Rust"
)]
struct ServerConfig {
    /// Network port to bind the server to.
    #[arg(short, long, default_value_t = 3020)]
    port: u16,

    /// Local directory path where photos are stored and served from.
    #[arg(short, long, default_value = "./photos")]
    storage_dir: String,

    /// Enable verbose mode (prints to stdout directly, ignoring tracing filters)
    #[arg(short, long)]
    verbose: bool,
}

/// Global shared state for the application.
struct GlobalState {
    broadcast_tx: broadcast::Sender<GalleryEvent>,
    base_dir: PathBuf,
    verbose: bool,
}

impl GlobalState {
    /// Prints to stdout only if the verbose flag is set.
    /// This bypasses the tracing module as requested.
    fn v_print(&self, msg: &str) {
        if self.verbose {
            println!("[VERBOSE] {}", msg);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Tracing with RUST_LOG sensitivity
    // If RUST_LOG is not set, we default to "none" (empty string filter)
    let filter: EnvFilter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(""));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config: ServerConfig = ServerConfig::parse();

    info!("Initializing Gallery Server...");
    trace!("Config parsed: {:?}", config);

    // Step 1: Initialize Storage
    let mut base_path: PathBuf = PathBuf::from(&config.storage_dir);
    if !base_path.exists() {
        warn!(
            "Storage directory {:?} does not exist. Creating...",
            base_path
        );
        std::fs::create_dir_all(&base_path)?;
    }
    base_path = base_path.canonicalize()?;
    info!("Storage root canonicalized to: {:?}", base_path);

    // Step 2: Setup Communication Channels
    let (tx, _rx): (
        broadcast::Sender<GalleryEvent>,
        broadcast::Receiver<GalleryEvent>,
    ) = broadcast::channel::<GalleryEvent>(100);
    let shared_state: Arc<GlobalState> = Arc::new(GlobalState {
        broadcast_tx: tx.clone(),
        base_dir: base_path.clone(),
        verbose: config.verbose,
    });

    shared_state.v_print("Verbose mode is enabled. High-level events will be printed to stdout.");

    // Step 3: Initialize Filesystem Watcher
    let watcher_tx: broadcast::Sender<GalleryEvent> = tx.clone();
    let root_for_watcher: PathBuf = base_path.clone();

    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| match res {
            Ok(event) => {
                trace!("FS Watcher received event: {:?}", event);
                broadcast_fs_changes(event, &root_for_watcher, &watcher_tx);
            }
            Err(e) => error!("FS Watcher error: {:?}", e),
        },
        Config::default(),
    )?;

    watcher.watch(&base_path, RecursiveMode::Recursive)?;
    debug!("FS Watcher attached to recursive path: {:?}", base_path);

    // Step 4: Configure Router
    let app: Router = Router::new()
        .route("/", get(serve_ui))
        .route("/api/v1/images", get(list_all_images))
        .route(
            "/api/v1/images/{*path}",
            get(fetch_image_details).delete(remove_image),
        )
        .route("/api/v1/upload", post(handle_multipart_upload))
        .route("/ws", get(establish_websocket))
        .nest_service("/images", ServeDir::new(&base_path))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(shared_state);

    // Step 5: Start Listener
    let server_address: std::net::SocketAddr =
        std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    println!("🖼️  Photo Gallery Core active on {}", server_address);
    info!("Starting axum server on {}", server_address);

    let listener: tokio::net::TcpListener = tokio::net::TcpListener::bind(server_address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn broadcast_fs_changes(event: Event, root: &Path, tx: &broadcast::Sender<GalleryEvent>) {
    let action: ChangeType = match event.kind {
        EventKind::Create(_) => {
            debug!("FS: Path created");
            ChangeType::Added
        }
        EventKind::Modify(_) => {
            trace!("FS: Path modified");
            ChangeType::Updated
        }
        EventKind::Remove(_) => {
            debug!("FS: Path removed");
            ChangeType::Removed
        }
        _ => return,
    };

    let paths: Vec<PathBuf> = event.paths;
    for path in paths {
        let path_ref: &Path = &path;

        // Fix: If the action is Removed, we don't call is_valid_image_type because the file is gone.
        // We broadcast it so the UI knows to refresh.
        let is_removal: bool = matches!(action, ChangeType::Removed);

        if is_removal || is_valid_image_type(path_ref) {
            if let Ok(rel) = path_ref.strip_prefix(root) {
                if let Some(rel_str) = rel.to_str() {
                    let gallery_event: GalleryEvent = GalleryEvent {
                        change_type: action.clone(),
                        rel_path: rel_str.to_string(),
                    };
                    trace!("Broadcasting gallery event: {:?}", gallery_event);
                    let _ = tx.send(gallery_event);
                }
            }
        }
    }
}

fn is_valid_image_type(path: &Path) -> bool {
    let extension: String = path
        .extension()
        .and_then(|ext: &std::ffi::OsStr| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    let valid: bool = matches!(
        extension.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "svg" | "bmp" | "tiff"
    );

    if !valid && !extension.is_empty() {
        trace!(
            "Ignoring non-image file: {:?} (extension: {})",
            path, extension
        );
    }
    valid
}

fn resolve_safe_path(root: &Path, input_path: &str) -> Option<PathBuf> {
    trace!(
        "Resolving safe path for input: '{}' relative to {:?}",
        input_path, root
    );
    let target: PathBuf = root.join(input_path);

    match target.canonicalize() {
        Ok(resolved) if resolved.starts_with(root) => {
            trace!("Path resolution success: {:?}", resolved);
            Some(resolved)
        }
        _ => {
            if target.starts_with(root) {
                debug!("Target does not exist but is within root: {:?}", target);
                Some(target)
            } else {
                warn!(
                    "SECURITY: Blocked path traversal attempt for path: '{}'",
                    input_path
                );
                None
            }
        }
    }
}

fn cleanup_empty_parents(path: &Path, root: &Path) {
    if let Some(parent) = path.parent() {
        if parent == root || !parent.starts_with(root) {
            trace!("Cleanup: Reached root or escaped boundary at {:?}", parent);
            return;
        }

        if let Ok(mut entries) = std::fs::read_dir(parent) {
            if entries.next().is_none() {
                debug!("Cleanup: Removing empty directory {:?}", parent);
                if std::fs::remove_dir(parent).is_ok() {
                    cleanup_empty_parents(parent, root);
                }
            }
        }
    }
}

// --- Handlers ---

async fn serve_ui() -> Html<String> {
    trace!("UI Request: Serving index.html");
    let html_content: String = include_str!("../templates/index.html").to_string();
    Html(html_content)
}

async fn list_all_images(State(state): State<Arc<GlobalState>>) -> Json<Vec<String>> {
    debug!("API: Requested full image list");
    state.v_print("Client requested full gallery image list.");

    let list: Vec<String> = WalkDir::new(&state.base_dir)
        .into_iter()
        .filter_map(|e: Result<DirEntry, walkdir::Error>| e.ok())
        .filter(|e: &DirEntry| e.file_type().is_file() && is_valid_image_type(e.path()))
        .filter_map(|e: DirEntry| {
            e.path()
                .strip_prefix(&state.base_dir)
                .ok()
                .and_then(|p: &Path| p.to_str())
                .map(|s: &str| s.to_string())
        })
        .collect::<Vec<String>>();

    trace!("API: Found {} images", list.len());
    Json(list)
}

#[derive(Serialize)]
struct FileInfo {
    path: String,
    size: u64,
    last_modified: Option<u64>,
}

async fn fetch_image_details(
    State(state): State<Arc<GlobalState>>,
    AxumPath(path): AxumPath<String>,
) -> Result<Json<FileInfo>, StatusCode> {
    debug!("API: Fetching details for '{}'", path);
    let full_path: PathBuf = resolve_safe_path(&state.base_dir, &path).ok_or_else(|| {
        warn!("API Detail: Path not found or unsafe: '{}'", path);
        StatusCode::NOT_FOUND
    })?;

    let meta: std::fs::Metadata = std::fs::metadata(&full_path).map_err(|e: std::io::Error| {
        error!("API Detail: Metadata IO error for {:?}: {}", full_path, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let ts: Option<u64> = meta
        .modified()
        .ok()
        .and_then(|m: std::time::SystemTime| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d: std::time::Duration| d.as_secs());

    trace!("API Detail: Success for '{}' ({} bytes)", path, meta.len());
    Ok(Json(FileInfo {
        path,
        size: meta.len(),
        last_modified: ts,
    }))
}

async fn remove_image(
    State(state): State<Arc<GlobalState>>,
    AxumPath(path): AxumPath<String>,
) -> StatusCode {
    info!("API: Delete request for '{}'", path);
    state.v_print(&format!("Processing delete request for file: {}", path));

    let full_path: PathBuf = match resolve_safe_path(&state.base_dir, &path) {
        Some(p) => p,
        None => {
            warn!("API Delete: Rejected forbidden path '{}'", path);
            return StatusCode::FORBIDDEN;
        }
    };

    match std::fs::remove_file(&full_path) {
        Ok(_) => {
            info!("API Delete: Successfully removed {:?}", full_path);
            cleanup_empty_parents(&full_path, &state.base_dir);
            StatusCode::NO_CONTENT
        }
        Err(e) => {
            error!("API Delete: Failed to remove {:?}: {}", full_path, e);
            StatusCode::NOT_FOUND
        }
    }
}

async fn handle_multipart_upload(
    State(state): State<Arc<GlobalState>>,
    mut multipart: AxumMultipart,
) -> Result<StatusCode, (StatusCode, String)> {
    debug!("API: Upload stream started");
    state.v_print("Incoming multipart upload started.");

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename: String = field
            .file_name()
            .map(|s: &str| s.to_string())
            .unwrap_or_default();
        if filename.is_empty() {
            trace!("Upload: Skipping field without filename");
            continue;
        }

        debug!("Upload: Processing file '{}'", filename);
        let bytes: axum::body::Bytes =
            field
                .bytes()
                .await
                .map_err(|err: axum::extract::multipart::MultipartError| {
                    error!("Upload: Stream read error: {}", err);
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Stream read error: {}", err),
                    )
                })?;

        let file_stem: &std::ffi::OsStr = Path::new(&filename).file_name().unwrap_or_default();
        let destination: PathBuf = state.base_dir.join(file_stem);

        trace!("Upload: Writing {} bytes to {:?}", bytes.len(), destination);
        std::fs::write(&destination, bytes).map_err(|err: std::io::Error| {
            error!("Upload: IO Error writing to {:?}: {}", destination, err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("IO Error: {}", err),
            )
        })?;

        info!("Upload: Successfully saved '{}'", filename);
        state.v_print(&format!("Successfully uploaded file: {}", filename));
    }

    Ok(StatusCode::CREATED)
}

async fn establish_websocket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<GlobalState>>,
) -> impl IntoResponse {
    trace!("WS: Upgrade requested");
    ws.on_upgrade(move |socket: WebSocket| async move {
        debug!("WS: Connection established");
        let mut subscription: broadcast::Receiver<GalleryEvent> = state.broadcast_tx.subscribe();
        let mut socket: WebSocket = socket;

        while let Ok(event) = subscription.recv().await {
            let msg: String = serde_json::to_string(&event).unwrap_or_default();
            trace!("WS: Sending event to client: {}", msg);
            if socket.send(Message::Text(msg.into())).await.is_err() {
                debug!("WS: Connection closed by client");
                break;
            }
        }
    })
}
