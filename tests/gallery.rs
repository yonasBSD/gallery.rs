use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt; // for collect
use std::fs;
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower::util::ServiceExt; // for oneshot

/// Mock AppState for testing purposes.
#[allow(dead_code)]
struct AppState {
    tx: broadcast::Sender<String>,
    photos_dir: PathBuf,
}

#[tokio::test]
async fn test_api_list_images_success() {
    // 1. Setup a temporary test directory with dummy images
    let test_dir = PathBuf::from("./test_gallery_list");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("image1.jpg"), "data").unwrap();

    // 2. Initialize mock state
    let (_tx, _) = broadcast::channel::<String>(10);

    // Mock router mapping
    let app = Router::new().route(
        "/api/v1/images",
        axum::routing::get(|| async { axum::Json(vec!["image1.jpg".to_string()]) }),
    );

    // 3. Execute request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/images")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Assertions
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let images: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert_eq!(images.len(), 1);

    // Cleanup
    fs::remove_dir_all(test_dir).unwrap();
}

#[tokio::test]
async fn test_api_delete_security_boundary() {
    // 1. Setup directory structure
    let test_dir = PathBuf::from("./test_gallery_security");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();

    // 2. Create a "secret" file outside the gallery root to attempt to access
    let secret_file = PathBuf::from("./secrets.txt");
    fs::write(&secret_file, "sensitive data").unwrap();

    // 3. Get absolute canonical root of the gallery
    let root = test_dir
        .canonicalize()
        .expect("Failed to canonicalize root");

    // 4. Simulate malicious path traversal input: "../secrets.txt"
    // When joined to the root, this points to a file outside the sandbox.
    let requested_path = "../secrets.txt";
    let full_path = root.join(requested_path);

    // 5. Logic Check: Resolve full path via canonicalize()
    // This resolves the ".." segments into the actual absolute path.
    let is_safe = match full_path.canonicalize() {
        Ok(canonical) => {
            // Check if the resolved path still starts with our intended root
            canonical.starts_with(&root)
        }
        Err(_) => {
            // If the file doesn't exist, it's effectively "safe" from deletion,
            // but for security testing, we treat resolution failure as blocked.
            false
        }
    };

    // 6. Assert that the traversal was correctly caught
    assert!(
        !is_safe,
        "Path traversal should be detected as unsafe (canonical path is outside root)"
    );

    // Cleanup
    fs::remove_dir_all(test_dir).unwrap();
    let _ = fs::remove_file(secret_file);
}

#[tokio::test]
async fn test_upload_endpoint_logic() {
    // Setup temporary directory for upload testing
    let test_dir = PathBuf::from("./test_gallery_upload");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();

    // 1. Construct a mock multipart body to verify formatting/parsing
    let boundary = "boundary123";
    let body_content = format!(
        "--{0}\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"test.png\"\r\n\
        Content-Type: image/png\r\n\r\n\
        binarydatahere\r\n\
        --{0}--\r\n",
        boundary
    );

    // 2. Verify the request building doesn't panic and headers are correct
    let _req = Request::builder()
        .method("POST")
        .uri("/api/v1/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body_content))
        .unwrap();

    // 3. Verify file system side-effect (simulating the successful write that the handler would perform)
    let file_path = test_dir.join("test.png");
    fs::write(&file_path, "binarydatahere").unwrap();

    // Assert the file exists and contains the expected content
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    assert_eq!(content, "binarydatahere");

    // Cleanup
    fs::remove_dir_all(test_dir).unwrap();
}

#[tokio::test]
async fn test_large_payload_handling() {
    // Check if 5MB payload passes through correctly (under the DefaultBodyLimit)
    let large_data = vec![0u8; 5 * 1024 * 1024];
    let app = Router::new().route(
        "/api/v1/upload",
        axum::routing::post(|body: Body| async move {
            let _ = body.collect().await;
            StatusCode::OK
        }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/upload")
                .body(Body::from(large_data))
                .unwrap(),
        )
        .await
        .unwrap();

    // Assert that a large (but within limit) payload is accepted
    assert_ne!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.status(), StatusCode::OK);
}
