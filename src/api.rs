use crate::core::{ActionItem, AppCommand, SessionOptions};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_USER};

pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

#[derive(Clone)]
pub struct AppState {
    pub cmd_tx: mpsc::UnboundedSender<AppCommand>,
    pub main_thread_id: u32,
}

fn wake_main_thread(thread_id: u32) {
    unsafe {
        let _ = PostThreadMessageW(thread_id, WM_CHECK_QUEUE, WPARAM(0), LPARAM(0));
    }
}



// ============================================================================
// Profile Management Endpoints
// ============================================================================

use crate::core::profile::ProfileManager;
use std::sync::OnceLock;

// Global profile manager instance
fn get_profile_manager() -> &'static ProfileManager {
    static PROFILE_MANAGER: OnceLock<ProfileManager> = OnceLock::new();
    PROFILE_MANAGER.get_or_init(|| {
        ProfileManager::default().expect("Failed to initialize ProfileManager")
    })
}

// GET /profile/list
async fn list_profiles() -> impl IntoResponse {
    let manager = get_profile_manager();
    
    match manager.list_profiles() {
        Ok(profiles) => (StatusCode::OK, Json(json!({ "profiles": profiles }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ 
            "error": e.code,
            "message": e.message 
        }))).into_response(),
    }
}

// POST /profile/create
#[derive(serde::Deserialize)]
struct CreateProfileRequest {
    name: String,
}

async fn create_profile(
    Json(payload): Json<CreateProfileRequest>,
) -> impl IntoResponse {
    let manager = get_profile_manager();
    
    match manager.create_profile(&payload.name) {
        Ok(profile) => (StatusCode::CREATED, Json(json!({ 
            "name": profile.name,
            "path": profile.path 
        }))).into_response(),
        Err(e) => {
            let status = match e.code.as_str() {
                "PROFILE_EXISTS" => StatusCode::CONFLICT,
                "INVALID_NAME" => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(json!({ 
                "error": e.code,
                "message": e.message 
            }))).into_response()
        }
    }
}

// DELETE /profile/:name
async fn delete_profile(Path(name): Path<String>) -> impl IntoResponse {
    let manager = get_profile_manager();
    
    match manager.delete_profile(&name) {
        Ok(_) => (StatusCode::OK, Json(json!({ 
            "message": format!("Profile '{}' deleted", name) 
        }))).into_response(),
        Err(e) => {
            let status = match e.code.as_str() {
                "PROFILE_NOT_FOUND" => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(json!({ 
                "error": e.code,
                "message": e.message 
            }))).into_response()
        }
    }
}

pub fn create_router(cmd_tx: mpsc::UnboundedSender<AppCommand>, main_thread_id: u32) -> Router {
    let state = AppState {
        cmd_tx,
        main_thread_id,
    };
    Router::new()
        .route("/health", get(health_check))
        .route("/create", post(create_session))
        .route("/navigate/:id", post(navigate))
        .route("/status/:id", get(get_status))
        .route("/execute/:id", post(execute_script))
        .route("/close/:id", delete(close_session))
        .route("/act/:id", post(act))
        // Profile management endpoints
        .route("/profile/list", get(list_profiles))
        .route("/profile/create", post(create_profile))
        .route("/profile/:name", delete(delete_profile))
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}

// POST /create
async fn create_session(
    State(state): State<AppState>,
    Json(options): Json<SessionOptions>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::CreateSession {
        options,
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
        Ok(Ok(Ok(id))) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Ok(Ok(Err(e))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            "Timeout waiting for session creation",
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct NavigatePayload {
    url: String,
}

// POST /navigate/:id
async fn navigate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<NavigatePayload>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Navigate {
        id,
        url: payload.url,
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
        Ok(Ok(Ok(_))) => (StatusCode::OK, "Navigated").into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            "Timeout waiting for navigation",
        )
            .into_response(),
    }
}

// GET /status/:id
async fn get_status(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::GetStatus { id, resp_tx: tx };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(info))) => (StatusCode::OK, Json(info)).into_response(),
        Ok(Ok(Err(e))) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ExecuteScriptPayload {
    script: String,
}

// POST /execute/:id
async fn execute_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<ExecuteScriptPayload>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request_id = Uuid::new_v4().to_string();
    let cmd = AppCommand::ExecuteScript {
        id,
        script: payload.script,
        request_id: request_id.clone(),
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
        Ok(Ok(Ok(result))) => (StatusCode::OK, Json(json!({ "result": result }))).into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

// DELETE /close/:id
async fn close_session(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::CloseSession { id, resp_tx: tx };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(_))) => (StatusCode::OK, "Closed").into_response(),
        Ok(Ok(Err(e))) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

// POST /act/:id
async fn act(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(action): Json<ActionItem>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Act {
        id,
        action,
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
        Ok(Ok(Ok(result))) => (StatusCode::OK, Json(json!({ "result": result }))).into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}
