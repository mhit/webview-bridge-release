//! WBP2 API v2 Endpoints
//!
//! REST API endpoints for WebView Bridge Protocol v2

use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::core::session_v2::{
    AcquireRequest, SessionManagerV2, SessionHandle,
};
use crate::core::SessionOptions;

// ============================================================================
// Global SessionManagerV2 Instance
// ============================================================================

static SESSION_MANAGER_V2: OnceLock<SessionManagerV2> = OnceLock::new();
static CORE_SESSION_MANAGER: OnceLock<Arc<crate::core::SessionManager>> = OnceLock::new();
static AUTH_TOKEN: OnceLock<String> = OnceLock::new();

/// Initialize the auth token (called from main at startup)
pub fn init_auth_token(token: String) {
    let _ = AUTH_TOKEN.set(token);
}

/// Get the auth token
fn get_auth_token() -> Option<&'static str> {
    AUTH_TOKEN.get().map(|s| s.as_str())
}

/// Initialize the v2 session manager
pub fn init_session_manager_v2(data_dir: PathBuf, max_sessions: usize) {
    let _ = SESSION_MANAGER_V2.set(SessionManagerV2::new(data_dir, max_sessions));
}

/// Set the core session manager reference (called from main)
pub fn set_core_session_manager(manager: Arc<crate::core::SessionManager>) {
    let _ = CORE_SESSION_MANAGER.set(manager);
}

/// Get the v2 session manager
pub fn get_session_manager_v2() -> &'static SessionManagerV2 {
    SESSION_MANAGER_V2.get().expect("SessionManagerV2 not initialized")
}

/// Get the core session manager (internal)
fn get_core_session_manager() -> Option<&'static Arc<crate::core::SessionManager>> {
    CORE_SESSION_MANAGER.get()
}

/// Get the core session manager (public, for cleanup from main)
pub fn get_core_session_manager_pub() -> Option<&'static Arc<crate::core::SessionManager>> {
    CORE_SESSION_MANAGER.get()
}

// ============================================================================
// WBP2 Error Response
// ============================================================================

/// WBP2 Error codes
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
fn error_response(error: Wbp2Error, message: &str) -> axum::response::Response {
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
    ).into_response()
}

// ============================================================================
// V2 Router
// ============================================================================

use tokio::sync::{mpsc, oneshot};
use crate::core::AppCommand;

/// App state for v2 API (session creation callback)
#[derive(Clone)]
pub struct V2AppState {
    /// Callback to create a new session using the v1 session manager
    pub create_session_fn: Arc<dyn Fn(SessionOptions) -> Result<(String, SessionHandle), String> + Send + Sync>,
    /// Command sender to communicate with session threads (same as v1)
    pub cmd_tx: mpsc::UnboundedSender<AppCommand>,
}

/// Auth middleware: checks Bearer token on non-public routes
async fn auth_middleware(req: Request, next: Next) -> impl IntoResponse {
    // Public endpoints that don't require auth
    let path = req.uri().path();
    if matches!(path, "/" | "/favicon.ico" | "/assets/icon.png" | "/health")
        || path.starts_with("/media/screenshots/")
    {
        return next.run(req).await;
    }

    // Check if auth is disabled via config (runtime toggle)
    if crate::core::config::get_config().server.no_auth {
        return next.run(req).await;
    }

    // Check Bearer token
    if let Some(token) = get_auth_token() {
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let provided = &header[7..];
                if provided == token {
                    return next.run(req).await;
                }
            }
            _ => {}
        }

        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": {
                    "code": "AUTH_001",
                    "name": "UNAUTHORIZED",
                    "message": "Missing or invalid Bearer token"
                }
            })),
        )
            .into_response();
    }

    // No token configured: allow all (backwards compatibility)
    next.run(req).await
}

/// Create the v2 API router
pub fn create_v2_router(state: V2AppState) -> Router {
    Router::new()
        // Root, favicon, and health
        .route("/", get(root_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/assets/icon.png", get(icon_png_handler))
        .route("/health", get(health_check))
        // Session management
        .route("/session/acquire", post(session_acquire))
        .route("/session/release", post(session_release))
        .route("/session/destroy", delete(session_destroy))
        .route("/session/cleanup", post(session_cleanup))
        .route("/session/clone", post(session_clone))
        .route("/session/visibility", post(session_visibility))
        .route("/session/focus", post(session_focus))
        .route("/session/list", get(session_list))
        .route("/session/stats", get(session_stats))
        .route("/session/:name", get(session_get))
        .route("/session/state/url", post(session_state_url))
        .route("/session/state/history", get(session_state_history))
        .route("/session/import", post(session_import_cookies))
        .route("/session/import/profiles", get(session_import_profiles))
        .route("/session/:name/cookies", get(session_get_cookies))
        .route("/session/:name/cookies", post(session_set_cookies))
        // Navigation (synchronous, waits for load)
        .route("/navigate", post(navigate_v2))
        .route("/click", post(click_v2))
        .route("/type", post(type_v2))
        .route("/execute", post(execute_v2))
        // Wait v2
        .route("/wait", post(wait_v2))
        // Screenshot v2
        .route("/screenshot", post(screenshot_v2))
        .route("/screenshot/devices", get(screenshot_devices))
        // Goal API
        .route("/goal", post(goal_execute))
        .route("/goal/flows", get(goal_list_flows))
        // Macro API
        .route("/macro", post(macro_execute))
        .route("/macro/list", get(macro_list))
        .route("/macro/register", post(macro_register))
        .route("/macro/detect-spa", post(macro_detect_spa))
        // Media API
        .route("/media/images", post(media_images))
        .route("/media/youtube/subtitles", post(media_youtube_subtitles))
        .route("/media/youtube/download", post(media_youtube_download))
        .route("/media/analyze", post(media_analyze))
        .route("/media/files/:ref", get(media_files_list))
        .route("/media/screenshots", get(media_screenshots_list))
        .route("/media/screenshots/:session/:filename", get(media_screenshots_get))
        .route("/media/persist", post(media_persist))
        .route("/media/extend", post(media_extend_ttl))
        // AI API
        .route("/ai/config", post(ai_config_update))
        .route("/ai/config", get(ai_config_get))
        .route("/ai/login", post(ai_login))
        .route("/ai/images/analyze", post(ai_images_analyze))
        .route("/ai/extract", post(ai_extract))
        .route("/ai/usage", get(ai_usage_stats))
        .route("/ai/models", get(ai_models_list))
        // Download API
        .route("/download/trigger", post(download_trigger))
        .route("/download/status/:id", get(download_status))
        .route("/download/batch", post(download_batch))
        // Storage API
        .route("/storage/status", get(storage_status))
        .route("/storage/cleanup", post(storage_cleanup))
        .route("/config/storage", post(config_storage))
        // Config API (Admin Dashboard)
        .route("/v2/config", get(get_config))
        .route("/v2/config", post(update_config))
        .route("/v2/ai/test", post(test_ai_connection))
        // Job Management API
        .route("/jobs/:id", get(job_get))
        .route("/jobs/:id", delete(job_cancel))
        .route("/jobs", get(job_list))
        // Batch API
        .route("/batch", post(batch_execute))
        // MCP API (Model Context Protocol - v3 consolidated 8 tools)
        .route("/mcp", post(mcp_v3_handler))
        .route("/mcp/tools", get(mcp_v3_tools_list))
        .layer(axum::middleware::from_fn(auth_middleware))
        .with_state(state)
}

// ============================================================================
// Root and Health Endpoints
// ============================================================================

/// GET / - Admin Dashboard (injects auth token into HTML)
async fn root_handler() -> Html<String> {
    let html = include_str!("dashboard.html");
    let token_script = if let Some(token) = get_auth_token() {
        format!("<script>window.__WB_TOKEN='{}';</script>", token)
    } else {
        String::new()
    };
    // Inject token script before </head>
    Html(html.replace("</head>", &format!("{}</head>", token_script)))
}

/// GET /favicon.ico - Embedded application icon
async fn favicon_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "image/x-icon"), ("cache-control", "public, max-age=86400")],
        include_bytes!("../docs/img/icon.ico").as_slice(),
    )
}

/// GET /assets/icon.png - Embedded application icon (PNG)
async fn icon_png_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "image/png"), ("cache-control", "public, max-age=86400")],
        include_bytes!("../docs/img/icon-128.png").as_slice(),
    )
}



/// GET /health - Health check
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "WBP2"
    }))
}

// ============================================================================
// Config Endpoints
// ============================================================================

/// GET /v2/config - Get current configuration
async fn get_config() -> impl IntoResponse {
    let config = crate::core::config::get_config();
    Json(json!({
        "server": {
            "bind": config.server.bind,
            "port": config.server.port,
            "max_sessions": config.server.max_sessions,
            "no_auth": config.server.no_auth
        },
        "ai": {
            "provider": config.ai.provider,
            "api_key_configured": config.ai.api_key.is_some(),
            "model": config.ai.model,
            "ollama_host": config.ai.ollama_host,
            "enabled": config.ai.enabled,
            "timeout_ms": config.ai.timeout_ms,
            "daily_budget_usd": config.ai.daily_budget_usd
        },
        "session": {
            "default_headless": config.session.default_headless,
            "default_width": config.session.default_width,
            "default_height": config.session.default_height,
            "timeout_seconds": config.session.timeout_seconds,
            "persist_profiles": config.session.persist_profiles
        },
        "media": {
            "download_dir": config.media.download_dir,
            "screenshots_dir": config.media.screenshots_dir,
            "max_download_size": config.media.max_download_size,
            "default_video_quality": config.media.default_video_quality
        }
    }))
}

/// POST /v2/config - Update configuration (partial update)
async fn update_config(
    Json(updates): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::core::config::update_config;
    
    let result = update_config(|config| {
        // Update AI settings
        if let Some(ai) = updates.get("ai") {
            if let Some(enabled) = ai.get("enabled").and_then(|v| v.as_bool()) {
                config.ai.enabled = enabled;
            }
            if let Some(provider) = ai.get("provider").and_then(|v| v.as_str()) {
                config.ai.provider = provider.to_string();
            }
            if let Some(model) = ai.get("model").and_then(|v| v.as_str()) {
                config.ai.model = model.to_string();
            }
            if let Some(api_key) = ai.get("api_key") {
                config.ai.api_key = api_key.as_str().map(|s| s.to_string());
            }
            if let Some(timeout) = ai.get("timeout_ms").and_then(|v| v.as_u64()) {
                config.ai.timeout_ms = timeout;
            }
            if let Some(budget) = ai.get("daily_budget_usd") {
                config.ai.daily_budget_usd = budget.as_f64().map(|f| f as f32);
            }
            if let Some(host) = ai.get("ollama_host") {
                config.ai.ollama_host = host.as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
            }
        }
        
        // Update server settings
        if let Some(server) = updates.get("server") {
            if let Some(bind) = server.get("bind").and_then(|v| v.as_str()) {
                config.server.bind = bind.to_string();
            }
            if let Some(port) = server.get("port").and_then(|v| v.as_u64()) {
                config.server.port = port as u16;
            }
            if let Some(max) = server.get("max_sessions").and_then(|v| v.as_u64()) {
                config.server.max_sessions = max as usize;
            }
            if let Some(no_auth) = server.get("no_auth").and_then(|v| v.as_bool()) {
                config.server.no_auth = no_auth;
            }
        }
        
        // Update session settings
        if let Some(session) = updates.get("session") {
            if let Some(headless) = session.get("default_headless").and_then(|v| v.as_bool()) {
                config.session.default_headless = headless;
            }
            if let Some(width) = session.get("default_width").and_then(|v| v.as_u64()) {
                config.session.default_width = width as u32;
            }
            if let Some(height) = session.get("default_height").and_then(|v| v.as_u64()) {
                config.session.default_height = height as u32;
            }
        }
        
        // Update media settings
        if let Some(media) = updates.get("media") {
            if let Some(dir) = media.get("download_dir") {
                config.media.download_dir = dir.as_str().map(|s| s.to_string());
            }
            if let Some(dir) = media.get("screenshots_dir") {
                config.media.screenshots_dir = dir.as_str().map(|s| s.to_string());
            }
            if let Some(size) = media.get("max_download_size").and_then(|v| v.as_u64()) {
                config.media.max_download_size = size;
            }
            if let Some(quality) = media.get("default_video_quality").and_then(|v| v.as_str()) {
                config.media.default_video_quality = quality.to_string();
            }
        }
    });
    
    match result {
        Ok(()) => (StatusCode::OK, Json(json!({
            "success": true,
            "message": "Configuration saved"
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": e
        })))
    }
}

/// Resolve Ollama host: request param > config > env > default
/// Normalizes the result to a proper http(s) URL (OLLAMA_HOST may be just a bind address like "0.0.0.0")
fn resolve_ollama_host(override_host: Option<&str>, config: &crate::core::config::AppConfig) -> String {
    let raw = override_host
        .filter(|h| !h.is_empty())
        .map(String::from)
        .or_else(|| config.ai.ollama_host.clone().filter(|h| !h.is_empty()))
        .or_else(|| std::env::var("OLLAMA_HOST").ok().filter(|h| !h.is_empty()))
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    normalize_ollama_url(&raw)
}

/// Delegate to OllamaClient::normalize_host for URL normalization
fn normalize_ollama_url(raw: &str) -> String {
    crate::core::ai::OllamaClient::normalize_host(raw)
}

/// POST /v2/ai/test - Test AI connection (supports both Gemini and Ollama)
/// Accepts optional JSON body: { "provider": "ollama", "host": "...", "model": "..." }
async fn test_ai_connection(
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let config = crate::core::config::get_config();
    let body = body.map(|b| b.0).unwrap_or(json!({}));

    // Use body values if provided, otherwise fall back to saved config
    let provider = body.get("provider").and_then(|v| v.as_str())
        .unwrap_or(&config.ai.provider);
    let model = body.get("model").and_then(|v| v.as_str())
        .unwrap_or(&config.ai.model);
    let host_override = body.get("host").and_then(|v| v.as_str());

    if provider == "ollama" {
        let host = resolve_ollama_host(host_override, &config);
        let ollama = crate::core::ai::OllamaClient {
            base_url: host.clone(),
            model: model.to_string(),
            timeout_ms: 10000,
        };
        if !ollama.is_available() {
            return Json(json!({
                "success": false,
                "error": format!("Ollama is not running at {}. Check the host URL and that Ollama is started.", host)
            }));
        }
        match ollama.call("Reply with only: OK", None) {
            Ok(response) => {
                return Json(json!({
                    "success": true,
                    "message": format!("Ollama connection successful ({})", model),
                    "response": response
                }));
            }
            Err(e) => {
                return Json(json!({
                    "success": false,
                    "error": format!("Ollama call failed (model: {}): {}", model, e)
                }));
            }
        }
    }

    // Gemini path
    if provider == "gemini" || provider.is_empty() {
        if let Some(api_key) = config.get_api_key() {
            let client = crate::core::ai::GeminiClient::new(&crate::core::ai::AiConfig {
                api_key: Some(api_key),
                model: model.to_string(),
                ..Default::default()
            });
            if let Some(client) = client {
                match client.call("Reply with only: OK", None) {
                    Ok(response) => {
                        return Json(json!({
                            "success": true,
                            "message": format!("Gemini connection successful ({})", model),
                            "response": response
                        }));
                    }
                    Err(e) => {
                        return Json(json!({
                            "success": false,
                            "error": format!("Gemini API call failed: {}", e)
                        }));
                    }
                }
            }
        }
        return Json(json!({
            "success": false,
            "error": "Gemini API key not configured. Set it in AI設定 or GEMINI_API_KEY env var."
        }));
    }

    Json(json!({
        "success": false,
        "error": format!("Unknown AI provider: '{}'. Supported: gemini, ollama", provider)
    }))
}

/// GET /ai/models?provider=ollama&host=http://localhost:11434
/// List available AI models for the specified (or configured) provider
async fn ai_models_list(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let config = crate::core::config::get_config();
    let provider = params.get("provider")
        .map(|s| s.as_str())
        .unwrap_or(&config.ai.provider);

    match provider {
        "ollama" => {
            let host = resolve_ollama_host(
                params.get("host").map(|s| s.as_str()),
                &config,
            );
            let ollama = crate::core::ai::OllamaClient {
                base_url: host,
                model: config.ai.model.clone(),
                timeout_ms: 5000,
            };
            match ollama.list_models() {
                Ok(models) => Json(json!({
                    "provider": "ollama",
                    "available": true,
                    "models": models,
                    "current_model": config.ai.model
                })),
                Err(e) => Json(json!({
                    "provider": "ollama",
                    "available": false,
                    "models": [],
                    "error": e
                })),
            }
        }
        "gemini" => {
            Json(json!({
                "provider": "gemini",
                "available": config.get_api_key().is_some(),
                "models": [
                    "gemini-2.0-flash",
                    "gemini-1.5-flash",
                    "gemini-1.5-pro",
                    "gemini-pro-vision"
                ],
                "current_model": config.ai.model
            }))
        }
        _ => {
            Json(json!({
                "provider": provider,
                "available": false,
                "models": [],
                "error": format!("Unknown provider: {}. Supported: gemini, ollama", provider)
            }))
        }
    }
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
    let should_restore = request.restore;
    let session_name = request.name.clone();
    
    let create_fn = |options: SessionOptions| -> Result<(String, SessionHandle), String> {
        (state.create_session_fn)(options)
    };
    
    match manager.acquire(request, create_fn).await {
        Ok(mut response) => {
            // Auto-restore: navigate to last URL if requested and available
            let mut restored_url = None;
            if should_restore && !response.is_new {
                if let Some(url) = manager.get_last_url(&session_name) {
                    // Perform navigation to restore last page
                    let nav_result = state.cmd_tx.send(AppCommand::Navigate {
                        id: session_name.clone(),
                        url: url.clone(),
                        resp_tx: {
                            let (tx, _rx) = oneshot::channel();
                            tx
                        },
                    });
                    
                    if nav_result.is_ok() {
                        restored_url = Some(url);
                    }
                }
            }
            response.restored_url = restored_url.clone();
            
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "session": response.session,
                    "is_new": response.is_new,
                    "profile": response.profile,
                    "auth_status": response.auth_status,
                    "restored_url": restored_url
                })),
            )
        },
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
    /// If true, close the WebView window and stop the session process
    #[serde(default)]
    close_window: bool,
}

/// POST /v2/session/release
async fn session_release(
    Json(request): Json<ReleaseRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    if request.close_window {
        // Close WebView window + release session
        match manager.close_session(&request.name) {
            Ok(session_id) => {
                // Also close in core SessionManager
                if let Some(id) = session_id {
                    if let Some(core_mgr) = get_core_session_manager() {
                        let _ = core_mgr.remove_session(&id).await;
                    }
                }
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": format!("Session '{}' stopped and window closed", request.name),
                        "closed": true
                    })),
                )
            },
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
    } else {
        // Legacy behavior: just mark as not acquired
        match manager.release(&request.name) {
            Ok(()) => (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Session '{}' released", request.name),
                    "closed": false
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
}

/// DELETE /session/destroy?name=xxx - Destroy a session and its profile
async fn session_destroy(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let name = match params.get("name") {
        Some(n) if !n.is_empty() => n.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_002",
                        "name": "MISSING_PARAMETER",
                        "message": "Missing required query parameter: name"
                    }
                })),
            );
        }
    };
    let manager = get_session_manager_v2();

    match manager.destroy(&name) {
        Ok(session_id) => {
            // Close WebView window if it was running
            if let Some(ref id) = session_id {
                if let Some(core_mgr) = get_core_session_manager() {
                    let _ = core_mgr.remove_session(id).await;
                }
                eprintln!("[session_destroy] Destroyed session '{}' (webview id: {})", name, id);
            }
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Session '{}' destroyed", name)
                })),
            )
        },
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

/// POST /v2/session/visibility - Set window visibility (pseudo-headless toggle)
async fn session_visibility(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match request.get("name").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_001",
                    "name": "INVALID_REQUEST",
                    "message": "Missing 'name' field"
                }
            })),
        ),
    };
    
    let visible = request.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
    
    // Get session ID from SessionManagerV2
    let manager_v2 = get_session_manager_v2();
    let session_id = match manager_v2.get_handle(name) {
        Some(h) => h.id.clone(),
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_010",
                    "name": "SESSION_NOT_FOUND",
                    "message": format!("Session '{}' not found or not active", name)
                }
            })),
        ),
    };
    
    // Use core SessionManager
    let core_manager = match get_core_session_manager() {
        Some(m) => m,
        None => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "MANAGER_NOT_INITIALIZED",
                    "message": "Core session manager not initialized"
                }
            })),
        ),
    };
    
    match core_manager.set_visibility(&session_id, visible).await {
        Ok(is_visible) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": name,
                "visible": is_visible,
                "message": if is_visible { "Window is now visible" } else { "Window is now hidden (pseudo-headless)" }
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "VISIBILITY_ERROR",
                    "message": e
                }
            })),
        ),
    }
}

/// POST /v2/session/focus - Bring window to front for user interaction
async fn session_focus(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match request.get("name").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_001",
                    "name": "INVALID_REQUEST",
                    "message": "Missing 'name' field"
                }
            })),
        ),
    };
    
    // Get session ID from SessionManagerV2
    let manager_v2 = get_session_manager_v2();
    let session_id = match manager_v2.get_handle(name) {
        Some(h) => h.id.clone(),
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_010",
                    "name": "SESSION_NOT_FOUND",
                    "message": format!("Session '{}' not found or not active", name)
                }
            })),
        ),
    };
    
    // Use core SessionManager
    let core_manager = match get_core_session_manager() {
        Some(m) => m,
        None => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "MANAGER_NOT_INITIALIZED",
                    "message": "Core session manager not initialized"
                }
            })),
        ),
    };
    
    match core_manager.bring_to_front(&session_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": name,
                "message": "Window brought to front"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "FOCUS_ERROR",
                    "message": e
                }
            })),
        ),
    }
}

/// POST /session/state/url - Update session's last URL (for restore feature)
async fn session_state_url(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match request.get("name").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Missing 'name' field"
            })),
        ),
    };
    
    let url = match request.get("url").and_then(|s| s.as_str()) {
        Some(u) => u,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Missing 'url' field"
            })),
        ),
    };
    
    let manager = get_session_manager_v2();
    
    match manager.update_last_url(name, url) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": name,
                "url": url
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": e
            })),
        ),
    }
}

/// GET /session/state/history - Get session's navigation history
async fn session_state_history(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let name = match params.get("name") {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Missing 'name' parameter"
            })),
        ),
    };
    
    let manager = get_session_manager_v2();
    let history = manager.get_navigation_history(name);
    let last_url = manager.get_last_url(name);
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "session": name,
            "last_url": last_url,
            "history": history,
            "count": history.len()
        })),
    )
}

/// GET /session/import/profiles - List available browser profiles for import
async fn session_import_profiles(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    use crate::core::cookie_import::{BrowserType, list_browser_profiles, get_cookie_db_path};
    
    let browser_str = params.get("browser").map(|s| s.as_str()).unwrap_or("chrome");
    let browser = match browser_str.to_lowercase().as_str() {
        "chrome" => BrowserType::Chrome,
        "edge" => BrowserType::Edge,
        "firefox" => BrowserType::Firefox,
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("Unknown browser: {}. Use 'chrome', 'edge', or 'firefox'", browser_str)
            })),
        ),
    };
    
    let profiles = list_browser_profiles(browser);
    let profiles_with_paths: Vec<_> = profiles.iter()
        .filter_map(|p| {
            let path = get_cookie_db_path(browser, p)?;
            Some(json!({
                "name": p,
                "cookie_db_exists": path.exists()
            }))
        })
        .collect();
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "browser": browser_str,
            "profiles": profiles_with_paths
        })),
    )
}

/// GET /session/:name/cookies - Get all cookies from a WebView session
async fn session_get_cookies(
    Path(name): Path<String>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    let handle = match manager.get_handle(&name) {
        Some(h) => h,
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Session '{}' not found", name) })),
        ),
    };

    let core_manager = match get_core_session_manager() {
        Some(m) => m,
        None => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "Core session manager not initialized" })),
        ),
    };

    match core_manager.get_cookies(&handle.id).await {
        Ok(cookies_json) => {
            let cookies: serde_json::Value = serde_json::from_str(&cookies_json).unwrap_or(json!([]));
            (StatusCode::OK, Json(json!({ "success": true, "session": name, "cookies": cookies })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// POST /session/:name/cookies - Set cookies on a WebView session
/// Body: { "cookies": [ { "name": "...", "value": "...", "domain": "...", ... } ] }
async fn session_set_cookies(
    Path(name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    let handle = match manager.get_handle(&name) {
        Some(h) => h,
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Session '{}' not found", name) })),
        ),
    };

    let cookies = match body.get("cookies") {
        Some(c) if c.is_array() => c,
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "Missing or invalid 'cookies' array in request body" })),
        ),
    };

    let core_manager = match get_core_session_manager() {
        Some(m) => m,
        None => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "Core session manager not initialized" })),
        ),
    };

    let cookies_json = serde_json::to_string(cookies).unwrap_or_else(|_| "[]".to_string());
    let count = cookies.as_array().map(|a| a.len()).unwrap_or(0);

    match core_manager.set_cookies(&handle.id, cookies_json).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "success": true, "session": name, "set_count": count })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// POST /session/import - Import cookies from browser to session
async fn session_import_cookies(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::core::cookie_import::{
        BrowserType, get_cookie_db_path, read_firefox_cookies, summarize_cookies
    };
    
    // Parse request
    let session = match request.get("session").and_then(|s| s.as_str()) {
        Some(s) => s.to_string(),
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Missing 'session' field"
            })),
        ),
    };
    
    let browser_str = request.get("browser").and_then(|s| s.as_str()).unwrap_or("chrome");
    let browser = match browser_str.to_lowercase().as_str() {
        "chrome" => BrowserType::Chrome,
        "edge" => BrowserType::Edge,
        "firefox" => BrowserType::Firefox,
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("Unknown browser: {}. Use 'chrome', 'edge', or 'firefox'", browser_str)
            })),
        ),
    };
    
    let profile = request.get("profile").and_then(|s| s.as_str()).unwrap_or("Default").to_string();
    let domains: Vec<String> = request.get("domains")
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    
    // Get cookie database path
    let db_path = match get_cookie_db_path(browser, &profile) {
        Some(p) if p.exists() => p,
        Some(p) => return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": format!("Cookie database not found at: {}", p.display())
            })),
        ),
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": format!("Could not determine cookie database path for {} profile '{}'", browser_str, profile)
            })),
        ),
    };
    
    // Read cookies based on browser type
    let cookies = match browser {
        BrowserType::Firefox => {
            match read_firefox_cookies(&db_path, &domains) {
                Ok(c) => c,
                Err(e) => return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": format!("Failed to read Firefox cookies: {}", e)
                    })),
                ),
            }
        }
        BrowserType::Chrome | BrowserType::Edge => {
            // Chromium cookies are encrypted with DPAPI
            // For now, return info about what would be imported
            return (
                StatusCode::OK,
                Json(json!({
                    "success": false,
                    "session": session,
                    "browser": browser_str,
                    "profile": profile,
                    "error": "Chrome/Edge cookie import requires DPAPI decryption (not yet implemented). Firefox import is fully supported.",
                    "workaround": "For Chromium browsers, you can manually copy cookies using browser DevTools or use Firefox for now."
                })),
            );
        }
    };
    
    // Summarize what was found
    let domain_counts = summarize_cookies(&cookies);
    let domains_found: Vec<_> = domain_counts.keys().cloned().collect();

    if cookies.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({
                "success": false,
                "session": session,
                "browser": browser_str,
                "error": "No cookies found for the specified domains"
            })),
        );
    }

    // Convert ImportedCookie → CDP CookieInfo JSON and set on WebView session
    let manager = get_session_manager_v2();
    let handle = match manager.get_handle(&session) {
        Some(h) => h,
        None => return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": format!("Session '{}' not found. Acquire it first.", session)
            })),
        ),
    };

    let cdp_cookies: Vec<serde_json::Value> = cookies.iter().map(|c| {
        let mut obj = serde_json::json!({
            "name": c.name,
            "value": c.value,
            "domain": c.domain,
            "path": c.path,
            "secure": c.secure,
            "http_only": c.http_only,
        });
        if let Some(exp) = c.expires {
            obj["expires"] = serde_json::json!(exp as f64);
        }
        obj
    }).collect();
    let cdp_json = serde_json::to_string(&cdp_cookies).unwrap_or_else(|_| "[]".to_string());

    let core_manager = match get_core_session_manager() {
        Some(m) => m,
        None => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "Core session manager not initialized"
            })),
        ),
    };

    match core_manager.set_cookies(&handle.id, cdp_json).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": session,
                "browser": browser_str,
                "profile": profile,
                "imported_count": cookies.len(),
                "domains_found": domains_found,
                "domain_counts": domain_counts,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Cookies read OK but failed to set on session: {}", e)
            })),
        ),
    }
}

/// POST /v2/session/clone - Clone a session (copy profile and data)
async fn session_clone(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let source = match request.get("source").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_001",
                    "name": "INVALID_REQUEST",
                    "message": "Missing 'source' field"
                }
            })),
        ),
    };
    
    let new_name = match request.get("new_name").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_001",
                    "name": "INVALID_REQUEST",
                    "message": "Missing 'new_name' field"
                }
            })),
        ),
    };
    
    let manager = get_session_manager_v2();
    
    match manager.clone_session(source, new_name) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "source": source,
                "new_session": new_name,
                "message": format!("Session '{}' cloned to '{}'", source, new_name)
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "CLONE_FAILED",
                    "message": e
                }
            })),
        ),
    }
}

/// POST /v2/session/cleanup - Cleanup inactive or old sessions
async fn session_cleanup(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let mode = request.get("mode")
        .and_then(|m| m.as_str())
        .unwrap_or("inactive");
    
    let max_age_hours = request.get("max_age_hours")
        .and_then(|h| h.as_u64())
        .unwrap_or(24);
    
    let result = match mode {
        "inactive" => manager.cleanup_inactive(),
        "old" => manager.cleanup_old(max_age_hours),
        "expired" => manager.cleanup_expired(),
        "all" => {
            // Cleanup all non-acquired sessions
            let mut _count = 0;
            if let Ok(ids) = manager.cleanup_expired() {
                _count += ids.len();
            }
            if let Ok(ids) = manager.cleanup_inactive() {
                _count += ids.len();
            }
            Ok(Vec::new()) // Return empty, count is in message
        },
        _ => Err(format!("Unknown cleanup mode: {}", mode)),
    };
    
    match result {
        Ok(session_ids) => {
            let count = session_ids.len();
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Cleaned up {} sessions", count),
                    "cleaned_count": count
                })),
            )
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": e
            })),
        ),
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

// ============================================================================
// Navigation Endpoints (Synchronous - wait until complete)
// ============================================================================

/// Request for POST /v2/navigate
#[derive(serde::Deserialize)]
struct NavigateRequest {
    session: String,
    url: String,
    #[allow(dead_code)]
    #[serde(default = "default_wait_until")]
    wait_until: String,  // "load" | "domready" | "none"
    #[serde(default = "default_nav_timeout")]
    timeout_ms: u64,
}

fn default_wait_until() -> String { "load".to_string() }
fn default_nav_timeout() -> u64 { 30000 }

/// POST /v2/navigate - Navigate to URL and wait for load
async fn navigate_v2(
    State(state): State<V2AppState>,
    Json(request): Json<NavigateRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let manager = get_session_manager_v2();
    
    // Get session handle
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", request.session)),
    };
    
    let session_id = handle.id.clone();
    
    // Send navigate command
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Navigate {
        id: session_id,
        url: request.url.clone(),
        resp_tx: tx,
    };
    
    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }
    
    // Wait for navigation to complete
    match tokio::time::timeout(
        std::time::Duration::from_millis(request.timeout_ms),
        rx
    ).await {
        Ok(Ok(Ok(()))) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": request.session,
                "url": request.url,
                "load_time_ms": start.elapsed().as_millis() as u64
            })),
        ).into_response(),
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Navigation channel closed"),
        Err(_) => error_response(Wbp2Error::InternalError, "Navigation timed out"),
    }
}

/// Request for POST /v2/click
#[derive(serde::Deserialize)]
struct ClickRequest {
    session: String,
    selector: String,
    #[serde(default)]
    wait_after_ms: Option<u64>,
}

/// POST /v2/click - Click element and wait
async fn click_v2(
    State(state): State<V2AppState>,
    Json(request): Json<ClickRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", request.session)),
    };
    
    // Execute click via JavaScript
    let script = format!(
        r#"(function(){{ 
            var el = document.querySelector("{}"); 
            if(!el) return JSON.stringify({{error:"Element not found"}}); 
            el.click(); 
            return JSON.stringify({{clicked:true}}); 
        }})()"#,
        request.selector.replace('\\', r#"\\"#).replace('"', r#"\""#)
    );

    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ExecuteScript {
        id: handle.id.clone(),
        script,
        resp_tx: tx,
    };

    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(result))) => {
            // Wait after click if specified
            if let Some(wait_ms) = request.wait_after_ms {
                tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
            }
            
            // Parse result
            if result.contains("error") {
                error_response(Wbp2Error::InvalidRequest, &result)
            } else {
                (StatusCode::OK, Json(json!({
                    "success": true,
                    "session": request.session,
                    "selector": request.selector,
                    "clicked": true
                }))).into_response()
            }
        }
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Channel closed"),
        Err(_) => error_response(Wbp2Error::InternalError, "Click timed out"),
    }
}

/// Request for POST /v2/type
#[derive(serde::Deserialize)]
struct TypeRequest {
    session: String,
    selector: String,
    text: String,
    #[serde(default)]
    clear_first: bool,
}

/// POST /v2/type - Type text into input
async fn type_v2(
    State(state): State<V2AppState>,
    Json(request): Json<TypeRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", request.session)),
    };
    
    let clear_code = if request.clear_first { "el.value = '';" } else { "" };
    let script = format!(
        r#"(function(){{ 
            var el = document.querySelector("{}"); 
            if(!el) return JSON.stringify({{error:"Element not found"}}); 
            {} 
            el.value = "{}"; 
            el.dispatchEvent(new Event('input', {{bubbles:true}})); 
            return JSON.stringify({{typed:true}}); 
        }})()"#,
        request.selector.replace('\\', r#"\\"#).replace('"', r#"\""#),
        clear_code,
        request.text.replace('\\', r#"\\"#).replace('"', r#"\""#).replace('\n', r#"\n"#)
    );
    
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ExecuteScript {
        id: handle.id.clone(),
        script,
        resp_tx: tx,
    };
    
    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }
    
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(result))) => {
            if result.contains("error") {
                error_response(Wbp2Error::InvalidRequest, &result)
            } else {
                (StatusCode::OK, Json(json!({
                    "success": true,
                    "session": request.session,
                    "selector": request.selector,
                    "text_length": request.text.len()
                }))).into_response()
            }
        }
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Channel closed"),
        Err(_) => error_response(Wbp2Error::InternalError, "Type timed out"),
    }
}

/// Request for POST /v2/execute
#[derive(serde::Deserialize)]
struct ExecuteRequest {
    session: String,
    script: String,
    #[serde(default = "default_execute_timeout")]
    timeout_ms: u64,
}

fn default_execute_timeout() -> u64 { 30000 }

/// POST /v2/execute - Execute JavaScript and return result
async fn execute_v2(
    State(state): State<V2AppState>,
    Json(request): Json<ExecuteRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let manager = get_session_manager_v2();
    
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", request.session)),
    };
    
    // Auto-wrap in IIFE if script uses top-level `return` but isn't already wrapped
    let script = {
        let trimmed = request.script.trim();
        let needs_wrap = trimmed.contains("return ")
            && !trimmed.starts_with("(function")
            && !trimmed.starts_with("(async")
            && !trimmed.starts_with("((");
        if needs_wrap {
            format!("(function(){{{}}})();", request.script)
        } else {
            request.script.clone()
        }
    };

    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ExecuteScript {
        id: handle.id.clone(),
        script,
        resp_tx: tx,
    };

    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }

    match tokio::time::timeout(
        std::time::Duration::from_millis(request.timeout_ms),
        rx
    ).await {
        Ok(Ok(Ok(result))) => {
            // Parse result back to proper JSON type (number, bool, null, object, array)
            // Falls back to string if not valid JSON (e.g. plain text from document.title)
            let typed_result = serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result));
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "session": request.session,
                    "result": typed_result,
                    "elapsed_ms": start.elapsed().as_millis() as u64
                })),
            ).into_response()
        }
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Channel closed"),
        Err(_) => error_response(Wbp2Error::InternalError, "Execution timed out"),
    }
}

// ============================================================================
// Wait v2 Endpoints
// ============================================================================

use crate::core::wait_v2::WaitRequest;

/// POST /v2/wait - Smart wait with multiple condition types
async fn wait_v2(
    State(state): State<V2AppState>,
    Json(request): Json<WaitRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    // Get session handle (contains v1 session ID)
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let session_id = handle.id.clone();
    
    // Use WaitForSelector command
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::WaitForSelector {
        id: session_id,
        selector: request.selector.clone(),
        timeout_ms: request.timeout_ms,
        resp_tx: tx,
    };
    
    if state.cmd_tx.send(cmd).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Failed to send command"
                }
            })),
        );
    }
    
    match tokio::time::timeout(
        std::time::Duration::from_millis(request.timeout_ms + 1000),
        rx
    ).await {
        Ok(Ok(Ok(found))) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "found": found,
                "session": request.session,
                "selector": request.selector,
                "condition": format!("{:?}", request.condition)
            })),
        ),
        Ok(Ok(Err(e))) => (
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
        Ok(Err(_)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Response channel closed"
                }
            })),
        ),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({
                "success": false,
                "found": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "TIMEOUT",
                    "message": "Wait timed out"
                }
            })),
        ),
    }
}

// ============================================================================
// Screenshot v2 Endpoints
// ============================================================================

use crate::core::screenshot_v2::{
    ScreenshotRequest, CaptureMode, get_device_presets, find_device_preset,
};

/// POST /v2/screenshot - Advanced screenshot with modes and device emulation
async fn screenshot_v2(
    State(state): State<V2AppState>,
    Json(request): Json<ScreenshotRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    // Build response based on capture mode
    let mode_info = match request.mode {
        CaptureMode::Viewport => "viewport",
        CaptureMode::FullPage => "full_page",
        CaptureMode::Element => "element",
    };
    
    // Device emulation info
    let device_info = if let Some(ref device_name) = request.device {
        if let Some(preset) = find_device_preset(device_name) {
            Some(json!({
                "name": preset.name,
                "viewport": {
                    "width": preset.viewport.width,
                    "height": preset.viewport.height
                },
                "is_mobile": preset.viewport.is_mobile
            }))
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_090",
                        "name": "INVALID_REQUEST",
                        "message": format!("Unknown device preset: {}", device_name)
                    }
                })),
            );
        }
    } else {
        None
    };
    
    // Get session handle (contains v1 session ID)
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let session_id = handle.id.clone();
    
    // Use Screenshot command
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Screenshot {
        id: session_id,
        resp_tx: tx,
    };
    
    if state.cmd_tx.send(cmd).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Failed to send command"
                }
            })),
        );
    }
    
    match tokio::time::timeout(
        std::time::Duration::from_millis(request.timeout_ms),
        rx
    ).await {
        Ok(Ok(Ok(base64_data))) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session": request.session,
                "mode": mode_info,
                "format": format!("{:?}", request.format).to_lowercase(),
                "quality": request.quality,
                "device": device_info,
                "image": base64_data
            })),
        ),
        Ok(Ok(Err(e))) => (
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
        Ok(Err(_)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Response channel closed"
                }
            })),
        ),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "TIMEOUT",
                    "message": "Screenshot timed out"
                }
            })),
        ),
    }
}

/// GET /v2/screenshot/devices - List available device presets
async fn screenshot_devices() -> impl IntoResponse {
    let presets = get_device_presets();
    
    let devices: Vec<serde_json::Value> = presets.iter().map(|p| {
        json!({
            "name": p.name,
            "viewport": {
                "width": p.viewport.width,
                "height": p.viewport.height,
                "device_scale_factor": p.viewport.device_scale_factor
            },
            "is_mobile": p.viewport.is_mobile,
            "has_touch": p.viewport.has_touch
        })
    }).collect();
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "devices": devices,
            "count": devices.len()
        })),
    )
}

// ============================================================================
// Goal API Endpoints
// ============================================================================

use crate::core::goal::{
    GoalRequest, GoalType, get_preset_flows, generate_goal_script,
};

/// POST /v2/goal - Execute a declarative goal
async fn goal_execute(
    State(state): State<V2AppState>,
    Json(request): Json<GoalRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    // Get session handle (contains the v1 session ID)
    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    // The handle.id is the v1 session ID
    let session_id = handle.id.clone();
    
    // Generate script for the goal
    let script = generate_goal_script(&request);
    
    // Get goal type as string
    let goal_type_str = match request.goal_type {
        GoalType::Navigate => "navigate",
        GoalType::Click => "click",
        GoalType::Fill => "fill",
        GoalType::Submit => "submit",
        GoalType::Wait => "wait",
        GoalType::Extract => "extract",
        GoalType::Login => "login",
        GoalType::Search => "search",
        GoalType::Scroll => "scroll",
        GoalType::Screenshot => "screenshot",
        GoalType::Custom => "custom",
    };
    
    // Execute based on goal type
    match request.goal_type {
        GoalType::Navigate => {
            // Use Navigate command
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::Navigate {
                id: session_id.clone(),
                url: request.target.clone(),
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                rx
            ).await {
                Ok(Ok(Ok(()))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "target": request.target,
                        "session": request.session
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Navigation timed out"
                        }
                    })),
                ),
            }
        }
        
        GoalType::Click | GoalType::Fill | GoalType::Submit | GoalType::Scroll => {
            // Execute script via ExecuteScript command
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::ExecuteScript {
                id: session_id.clone(),
                script: script.clone(),
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                rx
            ).await {
                Ok(Ok(Ok(result))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "target": request.target,
                        "session": request.session,
                        "result": result
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Script execution timed out"
                        }
                    })),
                ),
            }
        }
        
        GoalType::Extract => {
            // Use ExecuteScript for extraction
            let extract_script = format!(r#"
                (function() {{
                    var els = document.querySelectorAll("{}");
                    var results = [];
                    els.forEach(function(el) {{ results.push(el.textContent.trim()); }});
                    return JSON.stringify(results);
                }})()
            "#, request.target.replace('\\', r#"\\"#).replace('"', r#"\""#));
            
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::ExecuteScript {
                id: session_id.clone(),
                script: extract_script,
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                rx
            ).await {
                Ok(Ok(Ok(result))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "target": request.target,
                        "session": request.session,
                        "extracted": result
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Extraction timed out"
                        }
                    })),
                ),
            }
        }
        
        GoalType::Wait => {
            // Use WaitForSelector command
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::WaitForSelector {
                id: session_id.clone(),
                selector: request.target.clone(),
                timeout_ms: request.timeout_ms,
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms + 1000), // Extra buffer
                rx
            ).await {
                Ok(Ok(Ok(found))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "target": request.target,
                        "session": request.session,
                        "found": found
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Wait timed out"
                        }
                    })),
                ),
            }
        }
        
        GoalType::Screenshot => {
            // Use Screenshot command
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::Screenshot {
                id: session_id.clone(),
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                rx
            ).await {
                Ok(Ok(Ok(base64_data))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "session": request.session,
                        "image": base64_data
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Screenshot timed out"
                        }
                    })),
                ),
            }
        }
        
        // For other goal types, use script execution
        _ => {
            let (tx, rx) = oneshot::channel();
            let cmd = AppCommand::ExecuteScript {
                id: session_id.clone(),
                script: script.clone(),
                resp_tx: tx,
            };
            
            if state.cmd_tx.send(cmd).is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Failed to send command"
                        }
                    })),
                );
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                rx
            ).await {
                Ok(Ok(Ok(result))) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "goal_type": goal_type_str,
                        "target": request.target,
                        "session": request.session,
                        "result": result
                    })),
                ),
                Ok(Ok(Err(e))) => (
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
                Ok(Err(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "INTERNAL_ERROR",
                            "message": "Response channel closed"
                        }
                    })),
                ),
                Err(_) => (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_099",
                            "name": "TIMEOUT",
                            "message": "Execution timed out"
                        }
                    })),
                ),
            }
        }
    }
}

/// GET /v2/goal/flows - List available preset flows
async fn goal_list_flows() -> impl IntoResponse {
    let presets = get_preset_flows();
    
    let flows: Vec<serde_json::Value> = presets.iter().map(|f| {
        json!({
            "name": f.name,
            "description": f.description,
            "steps_count": f.steps.len()
        })
    }).collect();
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "flows": flows,
            "count": flows.len()
        })),
    )
}

// ============================================================================
// Macro API Endpoints
// ============================================================================

use crate::core::macro_engine::{
    MacroExecuteRequest, MacroRegisterRequest, get_preset_macros, 
    generate_macro_script, generate_spa_detection_script,
};

/// Simple session request for SPA detection
#[derive(Debug, Clone, Deserialize)]
struct SpaDetectRequest {
    session: String,
}

/// POST /v2/macro - Execute a macro
async fn macro_execute(
    State(_state): State<V2AppState>,
    Json(request): Json<MacroExecuteRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    // Get session handle
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    // Find macro
    let macro_def = match crate::core::macro_engine::find_preset_macro(&request.name) {
        Some(m) => m,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_080",
                        "name": "MACRO_NOT_FOUND",
                        "message": format!("Macro '{}' not found", request.name)
                    }
                })),
            );
        }
    };
    
    // Generate executable script
    let script = generate_macro_script(&macro_def, &request.params);
    
    // TODO: Execute script through session handle
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Macro execution queued",
            "session": request.session,
            "macro_name": request.name,
            "script_length": script.len(),
            "timeout_ms": request.timeout_ms.unwrap_or(macro_def.timeout_ms),
            "_note": "Full execution pending - script generated"
        })),
    )
}

/// GET /v2/macro/list - List available macros
async fn macro_list() -> impl IntoResponse {
    let macros = get_preset_macros();
    
    let list: Vec<serde_json::Value> = macros.iter().map(|m| {
        json!({
            "name": m.name,
            "description": m.description,
            "required_params": m.required_params,
            "optional_params": m.optional_params.keys().collect::<Vec<_>>(),
            "timeout_ms": m.timeout_ms,
            "builtin": m.builtin
        })
    }).collect();
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "macros": list,
            "count": list.len()
        })),
    )
}

/// POST /v2/macro/register - Register a custom macro
async fn macro_register(
    Json(request): Json<MacroRegisterRequest>,
) -> impl IntoResponse {
    // Validate
    if request.macro_def.name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_090",
                    "name": "INVALID_REQUEST",
                    "message": "Macro name cannot be empty"
                }
            })),
        );
    }
    
    // TODO: Actually register to persistent storage
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Macro registered",
            "name": request.macro_def.name,
            "_note": "Persistence pending implementation"
        })),
    )
}

/// POST /v2/macro/detect-spa - Detect SPA framework
async fn macro_detect_spa(
    Json(request): Json<SpaDetectRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    // Get session handle
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let script = generate_spa_detection_script();
    
    // TODO: Execute script through session handle
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "SPA detection queued",
            "session": request.session,
            "script_length": script.len(),
            "_note": "Full execution pending - script generated"
        })),
    )
}

// ============================================================================
// Media API Endpoints
// ============================================================================

use crate::core::media::{
    ImageCollectRequest, SubtitleRequest, VideoDownloadRequest, 
    VideoAnalyzeRequest, generate_image_extract_script,
};

/// POST /v2/media/images - Collect images from page
async fn media_images(
    Json(request): Json<ImageCollectRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let script = generate_image_extract_script(&request);
    
    // TODO: Execute script and collect images
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Image collection queued",
            "session": request.session,
            "output_format": format!("{:?}", request.output),
            "script_length": script.len(),
            "_note": "Full execution pending - script generated"
        })),
    )
}

/// POST /v2/media/youtube/subtitles - Extract YouTube subtitles
async fn media_youtube_subtitles(
    Json(request): Json<SubtitleRequest>,
) -> impl IntoResponse {
    // Extract video ID from URL
    let video_id = extract_youtube_id(&request.url);
    
    let languages = if request.languages.is_empty() {
        vec!["en".to_string(), "ja".to_string()]
    } else {
        request.languages.clone()
    };
    
    // TODO: Execute yt-dlp for subtitle extraction
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Subtitle extraction queued",
            "video_id": video_id,
            "languages": languages,
            "auto_generated": request.auto_generated,
            "format": format!("{:?}", request.format),
            "_note": "Requires yt-dlp installation"
        })),
    )
}

/// POST /v2/media/youtube/download - Download YouTube video
async fn media_youtube_download(
    Json(request): Json<VideoDownloadRequest>,
) -> impl IntoResponse {
    let video_id = extract_youtube_id(&request.url);
    let reference = uuid::Uuid::new_v4().to_string();
    
    // TODO: Execute yt-dlp for video download
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Video download queued",
            "video_id": video_id,
            "reference": reference,
            "quality": format!("{:?}", request.quality),
            "audio_only": request.audio_only,
            "status": "queued",
            "_note": "Requires yt-dlp installation"
        })),
    )
}

/// POST /v2/media/analyze - Analyze video with FFmpeg
async fn media_analyze(
    Json(request): Json<VideoAnalyzeRequest>,
) -> impl IntoResponse {
    let reference = uuid::Uuid::new_v4().to_string();
    
    // TODO: Execute FFmpeg analysis
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Video analysis queued",
            "source": request.source,
            "reference": reference,
            "analysis_types": request.analysis.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            "_note": "Requires FFmpeg installation"
        })),
    )
}

/// GET /v2/media/files/:ref - List files in reference
async fn media_files_list(
    Path(reference): Path<String>,
) -> impl IntoResponse {
    // TODO: Lookup reference in media cache
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "reference": reference,
            "files": [],
            "total_size": 0,
            "_note": "Reference not found or empty"
        })),
    )
}

/// Extract YouTube video ID from URL
fn extract_youtube_id(url: &str) -> String {
    // Handle various YouTube URL formats
    if let Some(pos) = url.find("v=") {
        let start = pos + 2;
        let end = url[start..].find('&').map(|p| start + p).unwrap_or(url.len());
        return url[start..end].to_string();
    }
    if let Some(pos) = url.find("youtu.be/") {
        let start = pos + 9;
        let end = url[start..].find('?').map(|p| start + p).unwrap_or(url.len());
        return url[start..end].to_string();
    }
    // Assume it's already a video ID
    url.to_string()
}

// ============================================================================
// AI API Endpoints
// ============================================================================

use crate::core::ai::{
    AiConfig, AiLoginRequest, AiImageAnalyzeRequest, AiExtractRequest,
    AiUsageTracker,
};

/// Global AI config
static AI_CONFIG: std::sync::OnceLock<std::sync::RwLock<AiConfig>> = std::sync::OnceLock::new();
static AI_USAGE: std::sync::OnceLock<std::sync::RwLock<AiUsageTracker>> = std::sync::OnceLock::new();

fn get_ai_config() -> &'static std::sync::RwLock<AiConfig> {
    AI_CONFIG.get_or_init(|| std::sync::RwLock::new(AiConfig::default()))
}

fn get_ai_usage() -> &'static std::sync::RwLock<AiUsageTracker> {
    AI_USAGE.get_or_init(|| std::sync::RwLock::new(AiUsageTracker::new()))
}

/// POST /v2/ai/config - Update AI configuration
async fn ai_config_update(
    Json(config): Json<AiConfig>,
) -> impl IntoResponse {
    let ai_config = get_ai_config();
    
    if let Ok(mut cfg) = ai_config.write() {
        *cfg = config.clone();
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "AI configuration updated",
                "provider": cfg.provider,
                "model": cfg.model,
                "enabled": cfg.enabled,
                "has_api_key": cfg.api_key.is_some()
            })),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Failed to update AI configuration"
                }
            })),
        )
    }
}

/// GET /v2/ai/config - Get AI configuration
async fn ai_config_get() -> impl IntoResponse {
    let ai_config = get_ai_config();
    
    if let Ok(cfg) = ai_config.read() {
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "provider": cfg.provider,
                "model": cfg.model,
                "enabled": cfg.enabled,
                "has_api_key": cfg.api_key.is_some(),
                "daily_budget_usd": cfg.daily_budget_usd,
                "daily_usage_usd": cfg.daily_usage_usd,
                "is_available": cfg.is_available()
            })),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_099",
                    "name": "INTERNAL_ERROR",
                    "message": "Failed to read AI configuration"
                }
            })),
        )
    }
}

/// POST /v2/ai/login - AI-assisted login
async fn ai_login(
    Json(request): Json<AiLoginRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    // Check AI availability
    let ai_config = get_ai_config();
    let is_available = ai_config.read().map(|c| c.is_available()).unwrap_or(false);
    
    if !is_available {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_100",
                    "name": "AI_NOT_AVAILABLE",
                    "message": "AI is not configured or API key is missing"
                }
            })),
        );
    }
    
    // TODO: Implement actual AI login flow
    // 1. Take screenshot
    // 2. Send to Gemini for form detection
    // 3. Fill credentials
    // 4. Detect CAPTCHA/2FA
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "AI login initiated",
            "session": request.session,
            "status": "in_progress",
            "url": request.url,
            "_note": "Full AI login flow pending Gemini integration"
        })),
    )
}

/// POST /v2/ai/images/analyze - AI image analysis
async fn ai_images_analyze(
    Json(request): Json<AiImageAnalyzeRequest>,
) -> impl IntoResponse {
    // Check AI availability
    let ai_config = get_ai_config();
    let is_available = ai_config.read().map(|c| c.is_available()).unwrap_or(false);
    
    if !is_available {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_100",
                    "name": "AI_NOT_AVAILABLE",
                    "message": "AI is not configured or API key is missing"
                }
            })),
        );
    }
    
    // TODO: Implement Gemini image analysis
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Image analysis queued",
            "image_count": request.images.len(),
            "analysis_type": format!("{:?}", request.analysis_type),
            "_note": "Full AI analysis pending Gemini integration"
        })),
    )
}

/// POST /v2/ai/extract - AI-assisted data extraction
async fn ai_extract(
    Json(request): Json<AiExtractRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    // Check AI availability
    let ai_config = get_ai_config();
    let is_available = ai_config.read().map(|c| c.is_available()).unwrap_or(false);
    
    if !is_available {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "error": {
                    "code": "WBP2_100",
                    "name": "AI_NOT_AVAILABLE",
                    "message": "AI is not configured or API key is missing"
                }
            })),
        );
    }
    
    // TODO: Implement AI extraction
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "AI extraction initiated",
            "session": request.session,
            "description": request.description,
            "auto_scroll": request.auto_scroll,
            "_note": "Full AI extraction pending Gemini integration"
        })),
    )
}

/// GET /v2/ai/usage - Get AI usage statistics
async fn ai_usage_stats() -> impl IntoResponse {
    let usage = get_ai_usage();
    
    if let Ok(tracker) = usage.read() {
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "stats": tracker.get_daily_stats()
            })),
        )
    } else {
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "stats": {
                    "total_requests": 0,
                    "total_input_tokens": 0,
                    "total_output_tokens": 0,
                    "total_cost_usd": 0.0
                }
            })),
        )
    }
}

// ============================================================================
// Download & Storage API Endpoints
// ============================================================================

use crate::core::download::{
    DownloadTriggerRequest, BatchDownloadRequest, CleanupRequest,
    StorageConfig, DownloadManager, StorageManager,
    PersistRequest, ExtendTtlRequest,
};

/// Global download manager
static DOWNLOAD_MANAGER: std::sync::OnceLock<std::sync::RwLock<DownloadManager>> = std::sync::OnceLock::new();
static STORAGE_MANAGER: std::sync::OnceLock<std::sync::RwLock<StorageManager>> = std::sync::OnceLock::new();

fn get_download_manager() -> &'static std::sync::RwLock<DownloadManager> {
    DOWNLOAD_MANAGER.get_or_init(|| std::sync::RwLock::new(DownloadManager::new()))
}

fn get_storage_manager() -> &'static std::sync::RwLock<StorageManager> {
    STORAGE_MANAGER.get_or_init(|| std::sync::RwLock::new(StorageManager::default()))
}

/// POST /v2/download/trigger - Trigger a download
async fn download_trigger(
    Json(request): Json<DownloadTriggerRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let download_manager = get_download_manager();
    let download_id = download_manager.write()
        .map(|mut mgr| mgr.start_download(&request.url, request.filename.clone()))
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    
    // TODO: Trigger actual WebView2 download
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "download_id": download_id,
            "status": "pending",
            "url": request.url,
            "filename": request.filename,
            "_note": "WebView2 download integration pending"
        })),
    )
}

/// GET /v2/download/status/:id - Get download status
async fn download_status(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let download_manager = get_download_manager();
    
    if let Ok(mgr) = download_manager.read() {
        if let Some(progress) = mgr.get_progress(&id) {
            return (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "download_id": id,
                    "status": format!("{:?}", progress.status),
                    "url": progress.url,
                    "filename": progress.filename,
                    "bytes_received": progress.bytes_received,
                    "total_bytes": progress.total_bytes,
                    "percent": progress.percent
                })),
            );
        }
    }
    
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_110",
                "name": "DOWNLOAD_NOT_FOUND",
                "message": format!("Download '{}' not found", id)
            }
        })),
    )
}

/// POST /v2/download/batch - Start batch download
async fn download_batch(
    Json(request): Json<BatchDownloadRequest>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    
    let _handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_001",
                        "name": "SESSION_NOT_FOUND",
                        "message": format!("Session '{}' not found", request.session)
                    }
                })),
            );
        }
    };
    
    let download_manager = get_download_manager();
    let (batch_id, download_ids) = download_manager.write()
        .map(|mut mgr| mgr.create_batch(&request.urls))
        .unwrap_or_else(|_| (uuid::Uuid::new_v4().to_string(), Vec::new()));
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "batch_id": batch_id,
            "download_ids": download_ids,
            "total": request.urls.len(),
            "parallel": request.parallel,
            "_note": "Batch download tracking created"
        })),
    )
}

/// GET /v2/storage/status - Get storage status
async fn storage_status() -> impl IntoResponse {
    let storage_manager = get_storage_manager();
    
    if let Ok(mgr) = storage_manager.read() {
        let status = mgr.get_status();
        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data_path": status.data_path,
                "used_bytes": status.used_bytes,
                "max_bytes": status.max_bytes,
                "usage_percent": status.usage_percent,
                "alert_level": format!("{:?}", status.alert_level)
            })),
        );
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to read storage status"
            }
        })),
    )
}

/// POST /v2/storage/cleanup - Cleanup expired files
async fn storage_cleanup(
    Json(request): Json<CleanupRequest>,
) -> impl IntoResponse {
    let storage_manager = get_storage_manager();
    
    if let Ok(mut mgr) = storage_manager.write() {
        let result = mgr.cleanup_expired(request.dry_run);
        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "files_deleted": result.files_deleted,
                "bytes_freed": result.bytes_freed,
                "dry_run": result.dry_run,
                "details": result.details
            })),
        );
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to run storage cleanup"
            }
        })),
    )
}

/// POST /v2/config/storage - Update storage configuration
async fn config_storage(
    Json(config): Json<StorageConfig>,
) -> impl IntoResponse {
    let storage_manager = get_storage_manager();
    
    if let Ok(mut mgr) = storage_manager.write() {
        mgr.config = config.clone();
        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Storage configuration updated",
                "data_path": config.data_path,
                "max_storage_bytes": config.max_storage_bytes,
                "default_ttl_seconds": config.default_ttl_seconds
            })),
        );
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to update storage configuration"
            }
        })),
    )
}

/// GET /v2/media/screenshots - List saved screenshots across all sessions
async fn media_screenshots_list() -> impl IntoResponse {
    let profiles_dir = crate::core::config::AppConfig::profiles_dir();

    let mut files = Vec::new();
    // Scan all session profile directories
    if let Ok(sessions) = std::fs::read_dir(&profiles_dir) {
        for session_entry in sessions.flatten() {
            if let Ok(meta) = session_entry.metadata() {
                if meta.is_dir() {
                    let session_name = session_entry.file_name().to_string_lossy().to_string();
                    let screenshots_dir = session_entry.path().join("screenshots");
                    if let Ok(entries) = std::fs::read_dir(&screenshots_dir) {
                        for entry in entries.flatten() {
                            if let Ok(file_meta) = entry.metadata() {
                                if file_meta.is_file() {
                                    if let Some(name) = entry.file_name().to_str() {
                                        files.push(serde_json::json!({
                                            "filename": name,
                                            "session": session_name,
                                            "size_bytes": file_meta.len(),
                                            "url": format!("/media/screenshots/{}/{}", session_name, name),
                                            "uri": format!("browser://screenshots/{}/{}", session_name, name)
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "screenshots": files,
            "count": files.len()
        })),
    )
}

/// GET /v2/media/screenshots/:session/:filename - Get a screenshot file
async fn media_screenshots_get(
    Path((session, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    use axum::body::Body;
    use axum::http::header;
    
    // Security: prevent path traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') 
        || session.contains("..") || session.contains('/') || session.contains('\\') {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(r#"{"error": "Invalid path"}"#),
        ).into_response();
    }
    
    let screenshots_dir = crate::core::config::AppConfig::profile_screenshots_dir(&session);
    
    let filepath = screenshots_dir.join(&filename);
    
    match std::fs::read(&filepath) {
        Ok(data) => {
            let content_type = if filename.ends_with(".webp") {
                "image/webp"
            } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
                "image/jpeg"
            } else {
                "image/png"
            };
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type)],
                Body::from(data),
            ).into_response()
        }
        Err(e) => {
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "application/json")],
                Body::from(format!(r#"{{"error": "Screenshot not found: {}"}}"#, e)),
            ).into_response()
        }
    }
}

/// POST /v2/media/persist - Persist a file reference
async fn media_persist(
    Json(request): Json<PersistRequest>,
) -> impl IntoResponse {
    let storage_manager = get_storage_manager();
    
    if let Ok(mut mgr) = storage_manager.write() {
        match mgr.persist(&request.file_ref) {
            Ok(()) => {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "file_ref": request.file_ref,
                        "persistent": true,
                        "message": "File reference marked as persistent"
                    })),
                );
            }
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_111",
                            "name": "FILE_REF_NOT_FOUND",
                            "message": e
                        }
                    })),
                );
            }
        }
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to persist file reference"
            }
        })),
    )
}

/// POST /v2/media/extend - Extend TTL of a file reference
async fn media_extend_ttl(
    Json(request): Json<ExtendTtlRequest>,
) -> impl IntoResponse {
    let storage_manager = get_storage_manager();
    
    if let Ok(mut mgr) = storage_manager.write() {
        match mgr.extend_ttl(&request.file_ref, request.additional_seconds) {
            Ok(new_expires) => {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "file_ref": request.file_ref,
                        "new_expires_at": new_expires,
                        "extended_by_seconds": request.additional_seconds
                    })),
                );
            }
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "success": false,
                        "error": {
                            "code": "WBP2_111",
                            "name": "FILE_REF_NOT_FOUND",
                            "message": e
                        }
                    })),
                );
            }
        }
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to extend TTL"
            }
        })),
    )
}

// ============================================================================
// Job Management & Batch API Endpoints
// ============================================================================

use crate::core::comm::{
    JobManager, JobStatus, JobType, BatchRequest, BatchOperationResult,
};

/// Global job manager
static JOB_MANAGER: std::sync::OnceLock<std::sync::RwLock<JobManager>> = std::sync::OnceLock::new();

fn get_job_manager() -> &'static std::sync::RwLock<JobManager> {
    JOB_MANAGER.get_or_init(|| std::sync::RwLock::new(JobManager::new()))
}

/// GET /v2/jobs/:id - Get job status
async fn job_get(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let job_manager = get_job_manager();
    
    if let Ok(mgr) = job_manager.read() {
        if let Some(job) = mgr.get_job(&id) {
            return (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "job": {
                        "id": job.id,
                        "type": format!("{:?}", job.job_type),
                        "status": format!("{:?}", job.status),
                        "created_at": job.created_at,
                        "started_at": job.started_at,
                        "completed_at": job.completed_at,
                        "percent": job.percent,
                        "eta_seconds": job.eta_seconds,
                        "result": job.result,
                        "error": job.error
                    }
                })),
            );
        }
    }
    
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_120",
                "name": "JOB_NOT_FOUND",
                "message": format!("Job '{}' not found", id)
            }
        })),
    )
}

/// DELETE /v2/jobs/:id - Cancel a job
async fn job_cancel(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let job_manager = get_job_manager();
    
    if let Ok(mut mgr) = job_manager.write() {
        if mgr.cancel_job(&id) {
            return (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Job cancelled",
                    "job_id": id
                })),
            );
        }
        
        // Check if job exists but cannot be cancelled
        if mgr.get_job(&id).is_some() {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_121",
                        "name": "JOB_ALREADY_COMPLETED",
                        "message": "Job is already completed or failed"
                    }
                })),
            );
        }
    }
    
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_120",
                "name": "JOB_NOT_FOUND",
                "message": format!("Job '{}' not found", id)
            }
        })),
    )
}

/// Query parameters for job list
#[derive(Debug, Deserialize)]
struct JobListQuery {
    #[serde(default)]
    status: Option<String>,
}

/// GET /v2/jobs - List jobs
async fn job_list(
    axum::extract::Query(query): axum::extract::Query<JobListQuery>,
) -> impl IntoResponse {
    let job_manager = get_job_manager();
    
    if let Ok(mgr) = job_manager.read() {
        let status_filter = query.status.as_ref().and_then(|s| match s.as_str() {
            "pending" => Some(JobStatus::Pending),
            "running" => Some(JobStatus::Running),
            "completed" => Some(JobStatus::Completed),
            "failed" => Some(JobStatus::Failed),
            "cancelled" => Some(JobStatus::Cancelled),
            _ => None,
        });
        
        let jobs: Vec<_> = mgr.list_jobs(status_filter)
            .iter()
            .map(|job| json!({
                "id": job.id,
                "type": format!("{:?}", job.job_type),
                "status": format!("{:?}", job.status),
                "created_at": job.created_at,
                "percent": job.percent
            }))
            .collect();
        
        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "jobs": jobs,
                "count": jobs.len()
            })),
        );
    }
    
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": {
                "code": "WBP2_099",
                "name": "INTERNAL_ERROR",
                "message": "Failed to list jobs"
            }
        })),
    )
}

/// POST /v2/batch - Execute batch operations
async fn batch_execute(
    Json(request): Json<BatchRequest>,
) -> impl IntoResponse {
    let job_manager = get_job_manager();
    
    // Create a batch job
    let job_id = job_manager.write()
        .map(|mut mgr| mgr.create_job(JobType::Batch, request.webhook.clone()))
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    
    // TODO: Implement actual batch execution with dependency resolution
    let results: Vec<BatchOperationResult> = request.operations.iter()
        .map(|op| BatchOperationResult {
            id: op.id.clone(),
            success: true,
            status_code: 200,
            response: json!({
                "message": "Operation queued",
                "method": op.method,
                "path": op.path
            }),
            error: None,
        })
        .collect();
    
    let completed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - completed;
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "job_id": job_id,
            "results": results,
            "completed": completed,
            "failed": failed,
            "stop_on_error": request.stop_on_error,
            "parallel": request.parallel,
            "_note": "Batch execution framework ready - actual execution pending"
        })),
    )
}

// ============================================================================
// MCP (Model Context Protocol) HTTP Handler
// ============================================================================

/// MCP JSON-RPC response
#[derive(Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Serialize)]
struct McpError {
    code: i32,
    message: String,
}

// ============================================================================
// MCP v3 API (Consolidated 8 Tools)
// ============================================================================

/// MCP v3 Request format
#[derive(Debug, serde::Deserialize)]
struct McpV3Request {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

/// POST /mcp/v3 - MCP v3 JSON-RPC handler with consolidated tools
async fn mcp_v3_handler(
    State(state): State<V2AppState>,
    Json(request): Json<McpV3Request>,
) -> Json<McpResponse> {
    let id = request.id.clone().unwrap_or(serde_json::Value::Null);
    
    tracing::info!("MCP v3 request: {}", request.method);
    
    let response = match request.method.as_str() {
        "initialize" => McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "listChanged": false, "subscribe": false }
                },
                "serverInfo": {
                    "name": "webview-bridge-v3",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },
        
        "initialized" | "notifications/initialized" => McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        },
        
        "tools/list" => McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "tools": crate::mcp_v3::tools::get_mcp_tools()
            })),
            error: None,
        },
        
        "tools/call" => {
            let params = request.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::json!({}));
            
            tracing::info!("MCP v3 tool call: {} with args: {}", tool_name, arguments);
            
            // Route to MCP v3 tools
            let result = crate::mcp_v3::tools::route_tool(tool_name, arguments, &state).await;
            
            if result.success {
                let content: Vec<serde_json::Value> = result.content
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| match c {
                        crate::mcp_v3::types::McpContent::Text { text } => {
                            serde_json::json!({"type": "text", "text": text})
                        }
                        crate::mcp_v3::types::McpContent::Image { data, mime_type } => {
                            serde_json::json!({"type": "image", "data": data, "mimeType": mime_type})
                        }
                    })
                    .collect();
                
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": content,
                        "isError": false
                    })),
                    error: None,
                }
            } else {
                let error = result.error.unwrap_or(crate::mcp_v3::types::McpError {
                    code: "UNKNOWN".to_string(),
                    message: "Unknown error".to_string(),
                    details: None,
                });
                
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("[{}] {}", error.code, error.message)
                        }],
                        "isError": true
                    })),
                    error: None,
                }
            }
        }
        
        "resources/list" => {
            // Dynamically list screenshots as MCP resources
            let profiles_dir = crate::core::config::AppConfig::profiles_dir();

            let mut resources = Vec::new();
            if let Ok(sessions) = std::fs::read_dir(&profiles_dir) {
                for session_entry in sessions.flatten() {
                    if session_entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                        let session_name = session_entry.file_name().to_string_lossy().to_string();
                        let screenshots_dir = session_entry.path().join("screenshots");
                        if let Ok(entries) = std::fs::read_dir(&screenshots_dir) {
                            for entry in entries.flatten() {
                                if let Some(name) = entry.file_name().to_str() {
                                    if name.ends_with(".png") || name.ends_with(".webp") || name.ends_with(".jpg") {
                                        let cfg = crate::core::config::get_config();
                                        let port = cfg.server.port;
                                        resources.push(serde_json::json!({
                                            "uri": format!("browser://screenshots/{}/{}", session_name, name),
                                            "name": name,
                                            "description": format!("Screenshot from session '{}'. HTTP: http://127.0.0.1:{}/media/screenshots/{}/{}", session_name, port, session_name, name),
                                            "mimeType": "image/png"
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "resources": resources
                })),
                error: None,
            }
        },
        
        "ping" => McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        },
        
        _ => McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    };
    
    Json(response)
}

/// GET /mcp/v3/tools - Get MCP v3 tool list (simple REST endpoint)
async fn mcp_v3_tools_list() -> impl IntoResponse {
    Json(crate::mcp_v3::tools::get_mcp_tools())
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
