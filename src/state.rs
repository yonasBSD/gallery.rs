// src/state.rs
use crate::{AppError, AppResult};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    storage_dir: PathBuf,
    tx: broadcast::Sender<()>,
    db: SqlitePool,
}

impl AppState {
    pub async fn new(storage_dir: PathBuf) -> AppResult<Self> {
        // Ensure storage directory exists FIRST
        if !storage_dir.exists() {
            tokio::fs::create_dir_all(&storage_dir).await.map_err(|e| {
                AppError::Internal(format!("Failed to create storage directory: {}", e))
            })?;
            info!("Created storage directory: {}", storage_dir.display());
        }

        // Create database path - ensure it's absolute for SQLite
        let db_path = storage_dir.join("gallery.db");

        // Canonicalize the path to get absolute path (if possible)
        let db_path = if let Ok(abs_path) = db_path.canonicalize() {
            abs_path
        } else {
            // If canonicalize fails (file doesn't exist yet), build absolute from current dir
            let current_dir = std::env::current_dir().map_err(|e| {
                AppError::Internal(format!("Failed to get current directory: {}", e))
            })?;
            let abs_path = current_dir.join(&db_path);
            debug!("Using absolute path: {}", abs_path.display());
            abs_path
        };

        // Ensure parent directory exists (it should from above, but double-check)
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AppError::Internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display()); // rwc = read/write/create
        debug!(db_url = %db_url, "Opening database");

        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to database: {}", e)))?;

        // Verify database is writable
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&db)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to configure database: {}", e)))?;

        // Run migrations from embedded SQL
        let migrations = vec![
            r#"
            CREATE TABLE IF NOT EXISTS images (
                id TEXT PRIMARY KEY,
                original_filename TEXT NOT NULL,
                original_path TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                width INTEGER DEFAULT 0,
                height INTEGER DEFAULT 0
            );
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS variants (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                image_id TEXT NOT NULL,
                resolution TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
            );
            "#,
            "CREATE INDEX IF NOT EXISTS idx_variants_image_id ON variants(image_id);",
            "CREATE INDEX IF NOT EXISTS idx_variants_resolution ON variants(resolution);",
            "CREATE INDEX IF NOT EXISTS idx_variants_lookup ON variants(image_id, resolution);",
        ];

        for migration in migrations {
            sqlx::query(migration)
                .execute(&db)
                .await
                .map_err(|e| AppError::Internal(format!("Migration failed: {}", e)))?;
        }

        // Verify tables were created
        let table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('images', 'variants')"
        )
        .fetch_one(&db)
        .await
        .map_err(|e| AppError::Internal(format!("Verification failed: {}", e)))?;

        if table_count != 2 {
            return Err(AppError::Internal(format!(
                "Expected 2 tables, found {}",
                table_count
            )));
        }

        info!("Database initialized at {}", db_path.display());

        let (tx, _) = broadcast::channel(16);
        let inner = Arc::new(AppStateInner {
            storage_dir,
            tx,
            db,
        });

        Ok(Self { inner })
    }

    pub fn storage_path(&self) -> &PathBuf {
        &self.inner.storage_dir
    }

    pub fn db(&self) -> &SqlitePool {
        &self.inner.db
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.inner.tx.subscribe()
    }

    pub fn notify(&self) {
        let _ = self.inner.tx.send(());
    }
}
