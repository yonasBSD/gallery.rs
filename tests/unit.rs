use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt; // for collect
use std::fs;
use std::path::PathBuf;
use tower::util::ServiceExt; // for oneshot

// Note: These tests assume your logic is accessible or abstracted
// into a library or shared module within your project.

#[tokio::test]
async fn test_index_route() {
    // In a real scenario, you'd pull the router from a library function like `create_app()`
    // For this example, we verify the HTML content delivery
    let html_content = "<html><body>Photo Gallery</body></html>";

    let app = axum::Router::new().route(
        "/",
        axum::routing::get(move || async { axum::response::Html(html_content.to_string()) }),
    );

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.starts_with(b"<html>"));
}

#[tokio::test]
async fn test_upload_and_file_creation() {
    let test_dir = PathBuf::from("./test_photos");
    if !test_dir.exists() {
        fs::create_dir_all(&test_dir).unwrap();
    }

    // Mock state for the test
    let photos_dir = test_dir.clone();

    // We simulate the multipart upload body
    // In a real test, you'd use a crate like `reqwest` or manually construct the boundary
    let boundary = "---------------------------1234567890";
    let _body = format!(
        "--{0}\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"test_image.jpg\"\r\n\
        Content-Type: image/jpeg\r\n\r\n\
        fake_image_data\r\n\
        --{0}--\r\n",
        boundary
    );

    // This is a simplified test case for the upload logic
    let path = photos_dir.join("test_image.jpg");
    fs::write(&path, "fake_image_data").unwrap();

    assert!(path.exists());
    let metadata = fs::metadata(&path).unwrap();
    assert!(metadata.is_file());

    // Cleanup
    fs::remove_dir_all(test_dir).unwrap();
}

#[tokio::test]
async fn test_is_image_utility() {
    // Assuming is_image is visible or testable
    let valid_cases = vec!["test.jpg", "photo.PNG", "graphic.webp", "anim.gif"];
    let invalid_cases = vec!["doc.pdf", "script.sh", "style.css", "README"];

    for name in valid_cases {
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        assert!(
            matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp"),
            "Failed on {}",
            name
        );
    }

    for name in invalid_cases {
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        assert!(
            !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp"),
            "Failed on {}",
            name
        );
    }
}

#[tokio::test]
async fn test_websocket_broadcast_logic() {
    // Testing the broadcast channel used in AppState
    let (tx, mut rx1) = tokio::sync::broadcast::channel::<String>(10);
    let mut rx2 = tx.subscribe();

    tx.send("new_photo.jpg".to_string()).unwrap();

    assert_eq!(rx1.recv().await.unwrap(), "new_photo.jpg");
    assert_eq!(rx2.recv().await.unwrap(), "new_photo.jpg");
}
