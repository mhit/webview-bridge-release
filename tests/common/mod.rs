//! Common test utilities for WBP2
//!
//! Shared helpers, fixtures, and test infrastructure.

pub mod test_server;

use axum::Router;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::net::TcpListener;

/// Atomic counter for generating unique port numbers
static PORT_COUNTER: AtomicU16 = AtomicU16::new(19400);

/// Get a unique port for testing
pub fn get_test_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Test server configuration
#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub port: u16,
    pub base_url: String,
}

impl TestServerConfig {
    pub fn new() -> Self {
        let port = get_test_port();
        Self {
            port,
            base_url: format!("http://localhost:{}", port),
        }
    }
    
    pub fn v2_url(&self, path: &str) -> String {
        format!("{}/v2{}", self.base_url, path)
    }
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP client wrapper for testing
pub struct TestClient {
    client: reqwest::Client,
    config: TestServerConfig,
}

impl TestClient {
    pub fn new(config: TestServerConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }
    
    /// POST request to v2 API
    pub async fn post(&self, path: &str, body: serde_json::Value) -> reqwest::Response {
        self.client
            .post(self.config.v2_url(path))
            .json(&body)
            .send()
            .await
            .expect("Failed to send request")
    }
    
    /// GET request to v2 API
    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(self.config.v2_url(path))
            .send()
            .await
            .expect("Failed to send request")
    }
    
    /// DELETE request to v2 API
    pub async fn delete(&self, path: &str) -> reqwest::Response {
        self.client
            .delete(self.config.v2_url(path))
            .send()
            .await
            .expect("Failed to send request")
    }
}

/// Spawn a test server and return a client
pub async fn spawn_test_server(router: Router) -> (TestClient, tokio::task::JoinHandle<()>) {
    let config = TestServerConfig::new();
    let addr = format!("127.0.0.1:{}", config.port);
    
    let listener = TcpListener::bind(&addr).await
        .expect("Failed to bind test server");
    
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = TestClient::new(config);
    (client, handle)
}

/// Performance measurement helper
#[derive(Debug)]
pub struct PerfMeasurement {
    pub operation: String,
    pub duration_ms: f64,
}

impl PerfMeasurement {
    pub fn new(operation: &str, duration: std::time::Duration) -> Self {
        Self {
            operation: operation.to_string(),
            duration_ms: duration.as_secs_f64() * 1000.0,
        }
    }
    
    /// Assert performance is within threshold
    pub fn assert_under(&self, max_ms: f64) {
        assert!(
            self.duration_ms < max_ms,
            "{} took {:.2}ms, expected < {:.2}ms",
            self.operation, self.duration_ms, max_ms
        );
    }
}

/// Macro for measuring operation time
#[macro_export]
macro_rules! measure {
    ($name:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        let measurement = $crate::common::PerfMeasurement::new($name, duration);
        (result, measurement)
    }};
}

/// Check if Gemini API is available
pub fn gemini_api_available() -> bool {
    std::env::var("GEMINI_API_KEY").is_ok()
}

/// Skip test if Gemini API is not available
#[macro_export]
macro_rules! require_gemini_api {
    () => {
        if !$crate::common::gemini_api_available() {
            eprintln!("Skipping test: GEMINI_API_KEY not set");
            return;
        }
    };
}

/// Test fixture paths
pub mod fixtures {
    use std::path::PathBuf;
    
    pub fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }
    
    pub fn pages_dir() -> PathBuf {
        fixtures_dir().join("pages")
    }
    
    pub fn profiles_dir() -> PathBuf {
        fixtures_dir().join("profiles")
    }
    
    pub fn macros_dir() -> PathBuf {
        fixtures_dir().join("macros")
    }
}

/// JSON response assertion helpers
pub mod assert_json {
    use serde_json::Value;
    
    /// Assert response has success: true
    pub fn success(body: &Value) {
        assert_eq!(
            body.get("success").and_then(|v| v.as_bool()),
            Some(true),
            "Expected success: true, got: {:?}",
            body
        );
    }
    
    /// Assert response has success: false with specific error code
    pub fn error_code(body: &Value, expected_code: &str) {
        assert_eq!(
            body.get("success").and_then(|v| v.as_bool()),
            Some(false),
            "Expected success: false"
        );
        
        let code = body.pointer("/error/code")
            .and_then(|v| v.as_str())
            .unwrap_or("NO_CODE");
        
        assert_eq!(code, expected_code, "Expected error code {}", expected_code);
    }
    
    /// Assert response contains a key
    pub fn has_key(body: &Value, key: &str) {
        assert!(
            body.get(key).is_some(),
            "Expected key '{}' in response: {:?}",
            key, body
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_port_uniqueness() {
        let p1 = get_test_port();
        let p2 = get_test_port();
        assert_ne!(p1, p2);
    }
    
    #[test]
    fn test_config_urls() {
        let config = TestServerConfig::new();
        assert!(config.v2_url("/session/list").contains("/v2/session/list"));
    }
    
    #[test]
    fn test_perf_measurement() {
        let m = PerfMeasurement::new("test", std::time::Duration::from_millis(50));
        m.assert_under(100.0);
    }
}
