//! WBP2 API v2 Endpoints
//!
//! REST API endpoints for WebView Bridge Protocol v2
//! See: docs/PROTOCOL_V2.md

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::core::session_v2::{
    AcquireRequest, SessionManagerV2,
};
use crate::core::{SessionHandle, SessionOptions};

// ============================================================================
// Global SessionManagerV2 Instance
// ============================================================================

static SESSION_MANAGER_V2: OnceLock<SessionManagerV2> = OnceLock::new();

/// Initialize the v2 session manager
pub fn init_session_manager_v2(data_dir: PathBuf, max_sessions: usize) {
    let _ = SESSION_MANAGER_V2.set(SessionManagerV2::new(data_dir, max_sessions));
}

/// Get the v2 session manager
fn get_session_manager_v2() -> &'static SessionManagerV2 {
    SESSION_MANAGER_V2.get().expect("SessionManagerV2 not initialized")
}

// ============================================================================
// WBP2 Error Response
// ============================================================================

/// WBP2 Error codes (see PROTOCOL_V2.md Section 18.5)
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Wbp2Error {
    SessionNotFound,      // WBP2_001
    SessionBusy,          // WBP2_002
    SessionClosed,        // WBP2_003
    InvalidRequest,       // WBP2_090
    MissingParameter,     // WBP2_091
    InternalError,        // WBP2_099
}

impl Wbp2Error {
    fn code(&self) -> &'static str {
        match self {
            Wbp2Error::SessionNotFound => "WBP2_001",
            Wbp2Error::SessionBusy => "WBP2_002",
            Wbp2Error::SessionClosed => "WBP2_003",
            Wbp2Error::InvalidRequest => "WBP2_090",
            Wbp2Error::MissingParameter => "WBP2_091",
            Wbp2Error::InternalError => "WBP2_099",
        }
    }
    
    fn name(&self) -> &'static str {
        match self {
            Wbp2Error::SessionNotFound => "SESSION_NOT_FOUND",
            Wbp2Error::SessionBusy => "SESSION_BUSY",
            Wbp2Error::SessionClosed => "SESSION_CLOSED",
            Wbp2Error::InvalidRequest => "INVALID_REQUEST",
            Wbp2Error::MissingParameter => "MISSING_PARAMETER",
            Wbp2Error::InternalError => "INTERNAL_ERROR",
        }
    }
    
    fn status_code(&self) -> StatusCode {
        match self {
            Wbp2Error::SessionNotFound => StatusCode::NOT_FOUND,
            Wbp2Error::SessionBusy => StatusCode::CONFLICT,
            Wbp2Error::SessionClosed => StatusCode::GONE,
            Wbp2Error::InvalidRequest => StatusCode::BAD_REQUEST,
            Wbp2Error::MissingParameter => StatusCode::BAD_REQUEST,
            Wbp2Error::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[allow(dead_code)]
fn error_response(error: Wbp2Error, message: &str) -> impl IntoResponse {
    (
        error.status_code(),
        Json(json!({
            "success": false,
            "error": {
                "code": error.code(),
                "name": error.name(),
                "message": message
            }
        })),
    )
}

// ============================================================================
// V2 Router
// ============================================================================

/// App state for v2 API (session creation callback)
#[derive(Clone)]
pub struct V2AppState {
    /// Callback to create a new session using the v1 session manager
    pub create_session_fn: Arc<dyn Fn(SessionOptions) -> Result<(String, SessionHandle), String> + Send + Sync>,
}

/// Create the v2 API router
pub fn create_v2_router(state: V2AppState) -> Router {
    Router::new()
        // Session management
        .route("/session/acquire", post(session_acquire))
        .route("/session/release", post(session_release))
        .route("/session/destroy", delete(session_destroy))
        .route("/session/list", get(session_list))
        .route("/session/stats", get(session_stats))
        .route("/session/:name", get(session_get))
        .with_state(state)
}

// ============================================================================
// Session Endpoints
// ============================================================================

/// POST /v2/session/acquire
async fn session_acquire(
    State(state): State<V2AppState>,
    Json(request): Json<AcquireRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let create_fn = |options: SessionOptions| -> Result<(String, SessionHandle), String> {
        (state.create_session_fn)(options)
    };
    
    match manager.acquire(request, create_fn).await {
        Ok(response) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": response.session,
                "is_new": response.is_new,
                "profile": response.profile,
                "auth_status": response.auth_status
            })),
        ),
        Err(e) => {
            let error = if e.contains("not found") {
                Wbp2Error::SessionNotFound
            } else if e.contains("Maximum sessions") {
                Wbp2Error::SessionBusy
            } else {
                Wbp2Error::InternalError
            };
            (error.status_code(), Json(json!({
                "success": false,
                "error": {
                    "code": error.code(),
                    "name": error.name(),
                    "message": e
                }
            })))
        }
    }
}

/// Request body for release
#[derive(serde::Deserialize)]
struct ReleaseRequest {
    name: String,
}

/// POST /v2/session/release
async fn session_release(
    Json(request): Json<ReleaseRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    match manager.release(&request.name) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("Session '{}' released", request.name)
            })),
        ),
        Err(e) => {
            let (code, name) = if e.contains("not found") {
                ("WBP2_001", "SESSION_NOT_FOUND")
            } else {
                ("WBP2_099", "INTERNAL_ERROR")
            };
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": code,
                        "name": name,
                        "message": e
                    }
                })),
            )
        }
    }
}

/// DELETE /v2/session/destroy (or DELETE /v2/session/:name)
async fn session_destroy(
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    match manager.destroy(&name) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("Session '{}' destroyed", name)
            })),
        ),
        Err(e) => {
            let (status, code, name_str) = if e.contains("not found") {
                (StatusCode::NOT_FOUND, "WBP2_001", "SESSION_NOT_FOUND")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "WBP2_099", "INTERNAL_ERROR")
            };
            (
                status,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": code,
                        "name": name_str,
                        "message": e
                    }
                })),
            )
        }
    }
}

/// GET /v2/session/list
async fn session_list() -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    match manager.list() {
        Ok(response) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "sessions": response.sessions,
                "total": response.total
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": e
                }
            })),
        ),
    }
}

/// GET /v2/session/:name
async fn session_get(
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    match manager.get(&name) {
        Some(session) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": {
                    "name": session.meta.name,
                    "profile": session.meta.profile,
                    "auth_status": session.meta.auth_status,
                    "last_accessed": session.meta.last_accessed,
                    "created_at": session.meta.created_at,
                    "active": session.handle.is_some(),
                    "acquired": session.acquired
                }
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_001",
                    "name": "SESSION_NOT_FOUND",
                    "message": format!("Session '{}' not found", name)
                }
            })),
        ),
    }
}

/// GET /v2/session/stats
async fn session_stats() -> impl IntoResponse {
    let manager = get_session_manager_v2();
    let stats = manager.stats();
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "stats": stats
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_codes() {
        assert_eq!(Wbp2Error::SessionNotFound.code(), "WBP2_001");
        assert_eq!(Wbp2Error::SessionBusy.code(), "WBP2_002");
        assert_eq!(Wbp2Error::InternalError.code(), "WBP2_099");
    }
}
