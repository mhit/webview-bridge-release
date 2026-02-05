use crate::core::{ActionItem, AppCommand, WM_CHECK_QUEUE, SessionOptions};
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
use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

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
        cmd_tx: cmd_tx.clone(),
        main_thread_id,
    };
    
    // Create bounded channel for WebDriver/MCP
    let (bounded_tx, mut bounded_rx) = mpsc::channel::<AppCommand>(100);
    
    // Forward bounded to unbounded
    let unbounded_tx = cmd_tx.clone();
    tokio::spawn(async move {
        while let Some(cmd) = bounded_rx.recv().await {
            let _ = unbounded_tx.send(cmd);
        }
    });
    
    Router::new()
        .route("/health", get(health_check))
        .route("/create", post(create_session))
        .route("/navigate/:id", post(navigate))
        .route("/status/:id", get(get_status))
        .route("/execute/:id", post(execute_script))
        .route("/close/:id", delete(close_session))
        .route("/act/:id", post(act))
        .route("/snapshot/:id", get(snapshot))
        .route("/screenshot/:id", get(screenshot))
        .route("/cookies/:id", get(get_cookies))
        .route("/cookies/:id", post(set_cookies))
        .route("/wait/:id", post(wait_for_selector))
        .route("/extract/:id", post(extract))
        // Profile management endpoints
        .route("/profile/list", get(list_profiles))
        .route("/profile/create", post(create_profile))
        .route("/profile/:name", delete(delete_profile))
        .with_state(state)
        // WebDriver Protocol endpoints (Selenium compatible)
        .merge(crate::webdriver::webdriver_router(bounded_tx.clone()))
        // MCP (Model Context Protocol) endpoints
        .merge(crate::mcp::mcp_router(bounded_tx))
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

#[derive(serde::Deserialize)]
struct SnapshotQuery {
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "html".to_string()
}

// GET /snapshot/:id?format=html|text|aria
async fn snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SnapshotQuery>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Snapshot {
        id,
        format: query.format.clone(),
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(result))) => (
            StatusCode::OK,
            Json(json!({ 
                "format": query.format,
                "content": result 
            })),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

// GET /screenshot/:id
async fn screenshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Screenshot { id, resp_tx: tx };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    // Longer timeout for screenshot (can take time to render)
    match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
        Ok(Ok(Ok(data))) => (
            StatusCode::OK,
            Json(json!({ 
                "format": "png",
                "data": data
            })),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Screenshot timeout").into_response(),
    }
}

// GET /cookies/:id
async fn get_cookies(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::GetCookies { id, resp_tx: tx };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(data))) => (
            StatusCode::OK,
            Json(json!({ "cookies": serde_json::from_str::<serde_json::Value>(&data).unwrap_or(json!(data)) })),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct SetCookiesRequest {
    cookies: serde_json::Value,
}

// POST /cookies/:id
async fn set_cookies(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SetCookiesRequest>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::SetCookies {
        id,
        cookies: body.cookies.to_string(),
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(()))) => (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct WaitForSelectorRequest {
    selector: String,
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_timeout() -> u64 {
    15000
}

// POST /wait/:id
async fn wait_for_selector(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<WaitForSelectorRequest>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::WaitForSelector {
        id,
        selector: body.selector.clone(),
        timeout_ms: body.timeout,
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    // Wait longer than the JS timeout
    let api_timeout = std::time::Duration::from_millis(body.timeout + 10000);
    match tokio::time::timeout(api_timeout, rx).await {
        Ok(Ok(Ok(found))) => (
            StatusCode::OK,
            Json(json!({ "found": found, "selector": body.selector })),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e, "found": false }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, Json(json!({ "error": "Timeout", "found": false }))).into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct ExtractRequest {
    selector: String,
    #[serde(default = "default_attribute")]
    attribute: String,
    #[serde(default)]
    extract_all: bool,
}

fn default_attribute() -> String {
    "text".to_string()
}

// POST /extract/:id
async fn extract(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ExtractRequest>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Extract {
        id,
        selector: body.selector.clone(),
        attribute: body.attribute.clone(),
        extract_all: body.extract_all,
        resp_tx: tx,
    };

    if let Err(_) = state.cmd_tx.send(cmd) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command").into_response();
    }

    wake_main_thread(state.main_thread_id);

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(data))) => (
            StatusCode::OK,
            Json(json!({ 
                "data": serde_json::from_str::<serde_json::Value>(&data).unwrap_or(json!(data)),
                "selector": body.selector,
                "attribute": body.attribute
            })),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response channel closed").into_response()
        }
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Timeout").into_response(),
    }
}
