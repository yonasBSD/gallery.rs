-- Initial schema for gallery-rs with resolution variants

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    original_filename TEXT NOT NULL,
    original_path TEXT NOT NULL,
    uploaded_at INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    width INTEGER DEFAULT 0,
    height INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS variants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_id TEXT NOT NULL,
    resolution TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
);

CREATE INDEX idx_variants_image_id ON variants(image_id);
CREATE INDEX idx_variants_resolution ON variants(resolution);

-- For fast lookups by resolution
CREATE INDEX idx_variants_lookup ON variants(image_id, resolution);
