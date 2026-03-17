// tests/unit/models.rs
use chrono::{TimeZone, Utc};
use gallery_rs::models::{ChangeType, FileInfo, FormatCount, GalleryEvent, GallerySummary};

#[test]
fn test_gallery_event_creation() {
    let event = GalleryEvent::new(ChangeType::Added, "test.jpg".to_string());

    assert_eq!(event.change_type, ChangeType::Added);
    assert_eq!(event.rel_path, "test.jpg");
    assert!(event.timestamp <= Utc::now());
    assert!(!event.id.to_string().is_empty());
}

#[test]
fn test_gallery_event_serialization() {
    let event = GalleryEvent::new(ChangeType::Removed, "old.jpg".to_string());

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("removed"));
    assert!(json.contains("old.jpg"));
    assert!(json.contains("timestamp"));
    assert!(json.contains("id"));

    let deserialized: GalleryEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.change_type, ChangeType::Removed);
    assert_eq!(deserialized.rel_path, "old.jpg");
}

#[test]
fn test_file_info_creation() {
    let dt = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    let info = FileInfo {
        path: "test.jpg".to_string(),
        name: "test.jpg".to_string(),
        size: 1024,
        last_modified: Some(dt),
        mime_type: Some("image/jpeg".to_string()),
        dimensions: None,
    };

    assert_eq!(info.size, 1024);
    assert_eq!(info.mime_type.unwrap(), "image/jpeg");
}

#[test]
fn test_gallery_summary() {
    let now = Utc::now();
    let earlier = now - chrono::Duration::days(1);

    let summary = GallerySummary {
        total_images: 10,
        total_size: 10240,
        oldest_image: Some(earlier),
        newest_image: Some(now),
        formats: vec![
            FormatCount {
                extension: "jpg".to_string(),
                count: 5,
                total_size: 5120,
            },
            FormatCount {
                extension: "png".to_string(),
                count: 5,
                total_size: 5120,
            },
        ],
    };

    assert_eq!(summary.total_images, 10);
    assert_eq!(summary.formats.len(), 2);

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("total_images"));
    assert!(json.contains("jpg"));
}
