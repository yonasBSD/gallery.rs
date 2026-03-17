// tests/integration/security.rs
use crate::common::TestApp;
use axum::body::Body;

#[tokio::test]
async fn test_prevent_directory_traversal_upload() {
    let app = TestApp::new().await;

    // Proper multipart format with correct line endings
    let boundary = "boundary123";
    let body = format!(
        "--{boundary}\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"../../etc/passwd\"\r\n\
        Content-Type: text/plain\r\n\r\n\
        malicious content\r\n\
        --{boundary}--\r\n"
    );

    let content_type = format!("multipart/form-data; boundary={boundary}");
    let response = app
        .request_with_content_type("POST", "/api/v1/upload", Body::from(body), &content_type)
        .await;

    // Handler should return 200 but sanitizes the filename
    assert_eq!(response.status(), 200);

    // The storage directory
    let storage_dir = std::fs::canonicalize(&app.config.storage_dir).unwrap();

    // Verify that NO file exists outside the storage directory root
    // We check if any file was created that doesn't belong to the storage_dir
    let mut entries = std::fs::read_dir(&storage_dir).unwrap();
    while let Some(Ok(entry)) = entries.next() {
        let path = entry.path();
        // The filename should have been stripped to just "passwd" or similar
        assert!(
            path.starts_with(&storage_dir),
            "File created outside storage directory!"
        );
        assert!(
            !path.to_string_lossy().contains(".."),
            "Filename still contains traversal components"
        );
    }

    // Explicitly check the sensitive location relative to the temp dir
    let leaked_path = app.temp_dir.path().join("etc/passwd");
    assert!(
        !leaked_path.exists(),
        "Traversal upload succeeded in writing to a relative sensitive path"
    );
}

#[tokio::test]
async fn test_cors_headers_present() {
    let app = TestApp::new().await;
    let request = axum::http::Request::builder()
        .uri("/api/v1/photos")
        .header("Origin", "http://localhost:3020")
        .body(Body::empty())
        .unwrap();

    let response = tower::ServiceExt::oneshot(app.app.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin")
    );
}

#[tokio::test]
async fn test_delete_traversal_protection() {
    let app = TestApp::new().await;

    let secret_file = app.temp_dir.path().join("system_secret.txt");
    std::fs::write(&secret_file, "secret data").unwrap();

    // Use encoded traversal in path
    let response = app
        .delete("/api/v1/photos/..%2F..%2Fsystem_secret.txt")
        .await;

    assert_ne!(response.status(), 200);
    assert!(
        secret_file.exists(),
        "Secret file should not have been deleted via traversal"
    );
}
