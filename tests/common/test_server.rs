//! Test Server Framework for WBP2
//!
//! Provides utilities for testing API endpoints with axum-test.

use axum::{
    Router,
    routing::{get, post, delete},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use axum_test::TestServer;
use serde_json::{json, Value};

// Re-export for external tests
pub use axum::http::StatusCode as HttpStatusCode;

/// Create a test server with the v2 API router
pub async fn create_test_server() -> TestServer {
    // Import the actual router from the library
    // For now, we create a minimal test router
    let app = create_test_router();
    
    TestServer::new(app).expect("Failed to create test server")
}

/// Create a minimal test router for API testing
/// This mirrors the actual API structure for testing purposes
fn create_test_router() -> Router {
    Router::new()
        // Session endpoints
        .route("/v2/session/list", get(mock_session_list))
        .route("/v2/session/stats", get(mock_session_stats))
        .route("/v2/session/acquire", post(mock_session_acquire))
        .route("/v2/session/:name", get(mock_session_get))
        .route("/v2/session/:name/release", post(mock_session_release))
        .route("/v2/session/:name/destroy", delete(mock_session_destroy))
        // Job endpoints
        .route("/v2/jobs", get(mock_job_list))
        .route("/v2/jobs/:id", get(mock_job_get))
        .route("/v2/jobs/:id", delete(mock_job_cancel))
        // Storage endpoints
        .route("/v2/storage/status", get(mock_storage_status))
}

// ============================================================================
// Mock Handlers
// ============================================================================

async fn mock_session_list() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "success": true,
        "sessions": []
    })))
}

async fn mock_session_stats() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "success": true,
        "stats": {
            "total_sessions": 0,
            "active_sessions": 0,
            "available_sessions": 0
        }
    })))
}

#[derive(serde::Deserialize)]
struct AcquireRequest {
    name: Option<String>,
    profile: Option<String>,
}

async fn mock_session_acquire(
    Json(request): Json<AcquireRequest>,
) -> impl IntoResponse {
    let name = request.name.unwrap_or_else(|| "session_1".to_string());
    (StatusCode::CREATED, Json(json!({
        "success": true,
        "session": name,
        "profile": request.profile.unwrap_or_else(|| "default".to_string()),
        "created": true
    })))
}

async fn mock_session_get(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    if name == "nonexistent" {
        (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_001",
                "name": "SESSION_NOT_FOUND",
                "message": format!("Session '{}' not found", name)
            }
        })))
    } else {
        (StatusCode::OK, Json(json!({
            "success": true,
            "session": {
                "name": name,
                "profile": "default",
                "status": "idle"
            }
        })))
    }
}

async fn mock_session_release(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    if name == "nonexistent" {
        (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_001",
                "name": "SESSION_NOT_FOUND",
                "message": format!("Session '{}' not found", name)
            }
        })))
    } else {
        (StatusCode::OK, Json(json!({
            "success": true,
            "message": "Session released"
        })))
    }
}

async fn mock_session_destroy(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    if name == "nonexistent" {
        (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_001",
                "name": "SESSION_NOT_FOUND",
                "message": format!("Session '{}' not found", name)
            }
        })))
    } else {
        (StatusCode::OK, Json(json!({
            "success": true,
            "message": "Session destroyed"
        })))
    }
}

async fn mock_job_list() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "success": true,
        "jobs": [],
        "count": 0
    })))
}

async fn mock_job_get(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if id == "nonexistent" {
        (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_120",
                "name": "JOB_NOT_FOUND",
                "message": format!("Job '{}' not found", id)
            }
        })))
    } else {
        (StatusCode::OK, Json(json!({
            "success": true,
            "job": {
                "id": id,
                "type": "download",
                "status": "pending"
            }
        })))
    }
}

async fn mock_job_cancel(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if id == "nonexistent" {
        (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_120",
                "name": "JOB_NOT_FOUND",
                "message": format!("Job '{}' not found", id)
            }
        })))
    } else {
        (StatusCode::OK, Json(json!({
            "success": true,
            "message": "Job cancelled"
        })))
    }
}

async fn mock_storage_status() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "success": true,
        "storage": {
            "base_path": "C:\\temp\\wbp2",
            "total_bytes": 0,
            "file_count": 0,
            "max_size_bytes": 1073741824
        }
    })))
}

// ============================================================================
// Test Assertions
// ============================================================================

/// Helper trait for response assertions
pub trait ResponseAssertions {
    fn assert_success(&self);
    fn assert_error_code(&self, code: &str);
    fn get_json(&self) -> Value;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    
    #[tokio::test]
    async fn test_server_creation() {
        let server = create_test_server().await;
        let response = server.get("/v2/session/list").await;
        response.assert_status_ok();
    }
    
    #[tokio::test]
    async fn test_session_acquire() {
        let server = create_test_server().await;
        let response = server
            .post("/v2/session/acquire")
            .json(&json!({"name": "test", "profile": "default"}))
            .await;
        
        response.assert_status(StatusCode::CREATED);
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
        assert_eq!(body["session"], "test");
    }
    
    #[tokio::test]
    async fn test_session_not_found() {
        let server = create_test_server().await;
        let response = server.get("/v2/session/nonexistent").await;
        
        response.assert_status(StatusCode::NOT_FOUND);
        let body: Value = response.json();
        assert!(!body["success"].as_bool().unwrap());
        assert_eq!(body["error"]["code"], "WBP2_001");
    }
    
    #[tokio::test]
    async fn test_session_stats() {
        let server = create_test_server().await;
        let response = server.get("/v2/session/stats").await;
        
        response.assert_status_ok();
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
        assert!(body["stats"].is_object());
    }
    
    #[tokio::test]
    async fn test_session_release() {
        let server = create_test_server().await;
        let response = server.post("/v2/session/main/release").await;
        
        response.assert_status_ok();
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
    }
    
    #[tokio::test]
    async fn test_session_destroy() {
        let server = create_test_server().await;
        let response = server.delete("/v2/session/main/destroy").await;
        
        response.assert_status_ok();
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
    }
    
    #[tokio::test]
    async fn test_job_list() {
        let server = create_test_server().await;
        let response = server.get("/v2/jobs").await;
        
        response.assert_status_ok();
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
        assert!(body["jobs"].is_array());
    }
    
    #[tokio::test]
    async fn test_job_not_found() {
        let server = create_test_server().await;
        let response = server.get("/v2/jobs/nonexistent").await;
        
        response.assert_status(StatusCode::NOT_FOUND);
        let body: Value = response.json();
        assert_eq!(body["error"]["code"], "WBP2_120");
    }
    
    #[tokio::test]
    async fn test_storage_status() {
        let server = create_test_server().await;
        let response = server.get("/v2/storage/status").await;
        
        response.assert_status_ok();
        let body: Value = response.json();
        assert!(body["success"].as_bool().unwrap());
        assert!(body["storage"]["base_path"].is_string());
    }
}
