// tests/integration/summary.rs
use crate::common::TestApp;
use gallery_rs::models::Photo;

#[tokio::test]
async fn test_gallery_summary_is_accurate() {
    let app = TestApp::new().await;
    app.create_test_image("a.jpg", b"123");
    app.create_test_image("b.jpg", b"12345");

    let res = app.get("/api/v1/photos").await;
    let photos: Vec<Photo> = res.json_as().await;

    assert_eq!(photos.len(), 2);
    let total_size: u64 = photos.iter().map(|p| p.size).sum();
    assert_eq!(total_size, 8);
}
