// tests/integration/common.rs
use axum::{
    Router,
    body::Body as AxumBody,
    http::{Request, Response},
};
use gallery_rs::{config::Config, service::GalleryService, state::AppState};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

pub struct TestApp {
    pub app: Router,
    pub config: Arc<Config>,
    pub state: AppState,
    pub temp_dir: TempDir,
}

impl TestApp {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            storage_dir: temp_dir.path().to_path_buf(),
            port: 0,
            host: "127.0.0.1".into(),
            verbose: false,
        };

        let service = GalleryService::new(config.clone()).await.unwrap();
        let state = service.state.clone();
        let app = service.into_router();

        Self {
            app,
            config: Arc::new(config),
            state,
            temp_dir,
        }
    }

    pub fn create_test_image(&self, name: &str, content: &[u8]) {
        let path = self.config.storage_dir.join(name);
        std::fs::write(path, content).unwrap();
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        let request = Request::builder().uri(uri).body(AxumBody::empty()).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        TestResponse(response)
    }

    pub async fn delete(&self, uri: &str) -> TestResponse {
        let request = Request::builder()
            .method("DELETE")
            .uri(uri)
            .body(AxumBody::empty())
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        TestResponse(response)
    }

    pub async fn request_with_content_type(
        &self,
        method: &str,
        uri: &str,
        body: AxumBody,
        content_type: &str,
    ) -> TestResponse {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", content_type)
            .body(body)
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        TestResponse(response)
    }
}

pub struct TestResponse(Response<AxumBody>);

impl TestResponse {
    pub fn status(&self) -> u16 {
        self.0.status().as_u16()
    }

    pub async fn json_as<T: DeserializeOwned>(self) -> T {
        let body = axum::body::to_bytes(self.0.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}
