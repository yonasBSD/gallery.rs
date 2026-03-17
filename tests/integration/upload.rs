// tests/integration/upload.rs
use crate::common::TestApp;
use axum::body::Body;

#[tokio::test]
async fn test_upload_single_file() {
    let app = TestApp::new().await;

    let boundary = "---------------------------987654321";
    let body = format!(
        "--{boundary}\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"image.jpg\"\r\n\
        Content-Type: image/jpeg\r\n\r\n\
        fake-jpeg-binary-data\r\n\
        --{boundary}--\r\n"
    );

    let content_type = format!("multipart/form-data; boundary={boundary}");
    let response = app
        .request_with_content_type("POST", "/api/v1/upload", Body::from(body), &content_type)
        .await;

    assert_eq!(response.status(), 200);
    // Now returns JSON
    let val: serde_json::Value = response.json_as().await;
    assert_eq!(val["status"], "success");

    let path = app.config.storage_dir.join("image.jpg");
    assert!(path.exists());
}

#[tokio::test]
async fn test_delete_image() {
    let app = TestApp::new().await;
    app.create_test_image("to_delete.jpg", b"stuff");

    let response = app.delete("/api/v1/photos/to_delete.jpg").await;
    assert_eq!(response.status(), 200);

    let path = app.config.storage_dir.join("to_delete.jpg");
    assert!(!path.exists());
}
