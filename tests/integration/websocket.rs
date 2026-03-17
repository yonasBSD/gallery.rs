// tests/integration/websocket.rs
use crate::common::TestApp;
use futures_util::StreamExt;
use std::time::Duration;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn test_websocket_connection_and_update() {
    let app = TestApp::new().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let router = app.app.clone();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let ws_url = format!("ws://{}/ws", addr);

    // Adding a small retry loop for connection stability in CI
    let mut ws_stream = None;
    for _ in 0..5 {
        if let Ok((s, _)) = connect_async(&ws_url).await {
            ws_stream = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut ws_stream = ws_stream.expect("Failed to connect to WS");

    // Trigger update
    app.state.notify();

    // Use a timeout to prevent hanging tests
    let msg = tokio::time::timeout(Duration::from_secs(1), ws_stream.next())
        .await
        .expect("Timeout waiting for WS message")
        .expect("Stream closed")
        .expect("WS Error");

    assert!(msg.is_text() || msg.is_binary());
    if msg.is_text() {
        assert_eq!(msg.to_text().unwrap(), "update");
    }
}
