// src/processor.rs
use crate::AppResult;
use image::{ImageFormat, imageops::FilterType};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

// Resolution targets (width in pixels, maintain aspect ratio)
const TARGETS: &[(&str, u32)] = &[
    ("mobile", 1080),  // 1080px wide, ~1-2MB
    ("web", 1920),     // 1080p display
    ("desktop", 2560), // 1440p/2K
    ("4k", 3840),      // 4K UHD
    ("8k", 7680),      // 8K
];

pub struct ImageProcessor {
    storage_path: PathBuf,
    variants_path: PathBuf,
}

impl ImageProcessor {
    pub fn new(storage_path: PathBuf) -> Self {
        let variants_path = storage_path.join("variants");
        Self {
            storage_path,
            variants_path,
        }
    }

    /// Process an uploaded file, generating all variants
    pub async fn process_upload(
        &self,
        image_id: &str,
        original_path: &Path,
    ) -> AppResult<Vec<(String, PathBuf, u64)>> {
        debug!(image_id, original_path = ?original_path, "Processing upload");

        // Load the image in a blocking thread
        let img = tokio::task::spawn_blocking({
            let path = original_path.to_path_buf();
            move || {
                image::open(&path).map_err(|e| {
                    crate::error::AppError::Internal(format!("Failed to decode image: {}", e))
                })
            }
        })
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Blocking task failed: {}", e)))??;

        let original_width = img.width();
        let original_height = img.height();

        debug!(
            image_id,
            original_width, original_height, "Loaded original image"
        );

        let mut results = Vec::new();

        // Store original info first
        let original_size = tokio::fs::metadata(original_path).await?.len();
        results.push((
            "original".to_string(),
            original_path.to_path_buf(),
            original_size,
        ));

        // Generate each variant
        for (label, target_width) in TARGETS {
            // Skip if target is larger than original
            if *target_width >= original_width {
                debug!(%label, "skipping (target larger than original)");
                continue;
            }

            let ratio = *target_width as f32 / original_width as f32;
            let target_height = (original_height as f32 * ratio) as u32;

            let variant_path = self
                .variants_path
                .join(image_id)
                .join(format!("{}.jpg", label));

            // Ensure directory exists
            if let Some(parent) = variant_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            // Resize and encode in blocking thread
            let variant_path_clone = variant_path.clone();
            let img_clone = img.clone();

            let variant_size = tokio::task::spawn_blocking(move || {
                let resized = img_clone.resize_exact(
                    *target_width,
                    target_height,
                    FilterType::Lanczos3, // Highest quality
                );

                // Encode to JPEG with quality 90
                let mut bytes = Vec::new();
                resized
                    .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                // Write to file
                std::fs::write(&variant_path_clone, &bytes)?;

                Ok::<_, std::io::Error>(bytes.len() as u64)
            })
            .await
            .map_err(|e| {
                crate::error::AppError::Internal(format!("Resize/encode failed: {}", e))
            })??;

            info!(
                image_id,
                variant = %label,
                size_mb = variant_size as f64 / 1024.0 / 1024.0,
                "Generated variant"
            );

            results.push((label.to_string(), variant_path, variant_size));
        }

        Ok(results)
    }
}
