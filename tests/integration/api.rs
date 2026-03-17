// tests/integration/api.rs
use crate::common::TestApp;
use gallery_rs::models::Photo;
use std::time::Duration;

#[tokio::test]
async fn test_list_empty_images() {
    let app = TestApp::new().await;
    let res = app.get("/api/v1/photos").await;
    assert_eq!(res.status(), 200);
    let photos: Vec<Photo> = res.json_as().await;
    assert!(photos.is_empty());
}

#[tokio::test]
async fn test_list_images_with_files() {
    let app = TestApp::new().await;

    // Create first image
    app.create_test_image("test1.jpg", b"content1");

    // Ensure a different timestamp for sorting reliability
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create second image (should be "newer")
    app.create_test_image("test2.jpg", b"content2");

    let res = app.get("/api/v1/photos").await;
    assert_eq!(res.status(), 200);
    let photos: Vec<Photo> = res.json_as().await;

    assert_eq!(photos.len(), 2);
    // Sort is descending by modified time, so test2.jpg should be first
    assert_eq!(photos[0].name, "test2.jpg");
    assert_eq!(photos[1].name, "test1.jpg");
}
