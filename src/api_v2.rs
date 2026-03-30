//! WBP2 API v2 Endpoints
//!
//! REST API endpoints for WebView Bridge Protocol v2

use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse},
    routing::{delete, get, post, put},
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
    // Also inject into V2 manager for get_handle() health checks
    if let Some(v2) = SESSION_MANAGER_V2.get() {
        v2.set_core_manager(Arc::clone(&manager));
    }
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
        || path.starts_with("/uploads/")
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
                    "message": "Missing or invalid Bearer token. Use 'wb auth save <TOKEN>' or set WB_TOKEN env var. The token is shown at server startup."
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
        .route("/session/auto-login", post(session_auto_login))
        .route("/session/auto-login", get(session_auto_login_status))
        .route("/session/auto-login/list", get(session_auto_login_list))
        .route("/session/auto-login/config", get(session_auto_login_config_get))
        .route("/session/auto-login/config", put(session_auto_login_config_set))
        // Navigation (synchronous, waits for load)
        .route("/navigate", post(navigate_v2))
        // Snapshot — DOM element extraction (prefer over screenshot for AI navigation)
        .route("/snapshot", post(snapshot_v2))
        .route("/click", post(click_v2))
        .route("/type", post(type_v2))
        .route("/execute", post(execute_v2))
        // Frame (iframe) operations
        .route("/frames", get(frames_list))
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
        // Upload API
        .route("/upload", post(upload_file))
        .route("/upload/base64", post(upload_base64))
        .route("/uploads/:session", get(upload_list))
        .route("/uploads/:session/:filename", get(upload_serve))
        .route("/uploads/:session/:filename", delete(upload_delete))
        // Form injection API
        .route("/form/inject-file", post(form_inject_file))
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
            // Get the session handle to read current_url and send navigate
            let handle = manager.get_handle(&session_name);

            // Auto-restore: navigate to last URL if requested and available
            let mut restored_url = None;
            if should_restore && !response.is_new {
                if let Some(url) = manager.get_last_url(&session_name) {
                    if let Some(ref h) = handle {
                        let (tx, rx) = oneshot::channel();
                        let nav_result = state.cmd_tx.send(AppCommand::Navigate {
                            id: h.id.clone(),
                            url: url.clone(),
                            resp_tx: tx,
                        });
                        if nav_result.is_ok() {
                            // Wait up to 15s for restore navigation
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_secs(15),
                                rx
                            ).await;
                            restored_url = Some(url);
                        }
                    }
                }
            }
            response.restored_url = restored_url.clone();

            // Include current URL so AI knows which page is open
            // last_url is kept in sync with every successful navigate call
            let current_url = manager.get_last_url(&session_name);

            // Build contextual hints for AI agents
            let auto_login_configured = crate::core::config::load_session_auto_login(&session_name).is_some();
            let logged_in = response.auth_status.as_ref()
                .map(|a| a.logged_in);
            let mut hints: Vec<&str> = Vec::new();

            if auto_login_configured && logged_in != Some(true) {
                hints.push("Auto-login is configured. Run POST /session/auto-login {\"name\":\"<session>\"} to authenticate without manual steps.");
            } else if !auto_login_configured {
                hints.push("No auto-login configured. Set up PUT /session/auto-login/config to automate logins.");
            }
            if response.is_new {
                hints.push("New session created. Sessions are persistent: cookies and login state survive server restarts.");
                hints.push("Use POST /navigate to open a URL, or POST /session/auto-login if credentials are configured.");
            } else {
                hints.push("Session resumed. Cookies and login state are intact from the previous run.");
                hints.push("Use POST /snapshot {\"session\":\"<session>\"} to see the current page without taking a screenshot.");
            }

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "session": response.session,
                    "is_new": response.is_new,
                    "profile": response.profile,
                    "auth_status": response.auth_status,
                    "restored_url": restored_url,
                    "current_url": current_url,
                    "hints": hints
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

// ============================================================================
// POST /session/auto-login  — 1Password auto-login
// ============================================================================

#[derive(serde::Deserialize)]
struct AutoLoginRequest {
    /// Session name
    name: String,
    /// If true, ignore the cached op_item_id and re-fetch from 1Password.
    /// Use this when saved credentials are stale (e.g. password changed).
    #[serde(default)]
    force: bool,
    /// Override the 1Password item name/ID for this request only.
    /// Takes priority over both config and cache.
    #[serde(default)]
    op_item: Option<String>,
    /// If true, take a screenshot after each key step and include URLs in the response.
    /// Useful for debugging selector/timing issues without needing server logs.
    #[serde(default)]
    debug: bool,
}

/// Build JS to fill an input field — sets .value property AND fires input/change events.
/// Works with React, Vue, Angular (which track state via events, not attribute mutations).
/// Take a debug screenshot during auto-login and save it to the profile's screenshot dir.
/// Returns the URL path like `/media/screenshots/{session}/autologin_{label}.png`.
/// No-ops (returns None) when `enabled` is false.
async fn auto_login_debug_screenshot(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AppCommand>,
    session: &str,
    label: &str,
    enabled: bool,
) -> Option<String> {
    if !enabled { return None; }
    let screenshot_dir = crate::core::config::AppConfig::profile_screenshots_dir(session);
    let _ = std::fs::create_dir_all(&screenshot_dir);
    let filename = format!("autologin_{}.png", label);
    let save_path = screenshot_dir.join(&filename);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = cmd_tx.send(AppCommand::ScreenshotCdp {
        id: session.to_string(),
        full_page: false,
        format: "png".to_string(),
        quality: None,
        frame: None,
        resp_tx: tx,
    });
    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(bytes))) => {
            if std::fs::write(&save_path, &bytes).is_ok() {
                tracing::debug!("[AutoLogin/debug] screenshot saved: {}", save_path.display());
                return Some(format!("/media/screenshots/{}/{}", session, filename));
            }
            None
        }
        _ => None,
    }
}

fn js_fill_input(selector: &str, value: &str) -> String {
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    let val_json = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string());
    format!(r#"(function(){{
        var sel={sel};var val={val};
        var el=document.querySelector(sel);
        if(!el)return JSON.stringify({{error:"Element not found: "+sel}});
        try{{var d=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value');if(d&&d.set)d.set.call(el,val);else el.value=val;}}catch(e){{el.value=val;}}
        el.dispatchEvent(new Event('input',{{bubbles:true}}));
        el.dispatchEvent(new Event('change',{{bubbles:true}}));
        return JSON.stringify({{ok:true}});
    }})()"#, sel=sel_json, val=val_json)
}


/// POST /session/auto-login
///
/// Performs automatic login using credentials stored in 1Password.
/// Requires `[session.auto_login.SESSION_NAME]` to be configured in config.toml.
///
/// Cache behaviour:
/// - On success: the working 1Password item ID is saved to the session DB.
/// - On failure: the cached item ID is cleared so the next attempt starts fresh.
/// - `force: true`: skip the cache entirely and fetch fresh from 1Password.
///
/// Re-authentication: call with `force: true` when you suspect the cached
/// session is stale (session expired, password changed, etc.).
async fn session_auto_login(
    State(state): State<V2AppState>,
    Json(request): Json<AutoLoginRequest>,
) -> impl IntoResponse {
    use std::time::Duration;

    let debug = request.debug;
    let mut debug_screenshots: Vec<serde_json::Value> = Vec::new();

    let manager = get_session_manager_v2();

    // --- 1. Load auto-login config for this session ---
    // Priority: profiles/{name}/auto_login.toml > config.toml [session.auto_login.{name}]
    let app_cfg = crate::core::config::get_config();
    let op_path_cfg = app_cfg.session.op_path.clone();
    let auto_login_cfg = match crate::core::config::load_session_auto_login(&request.name) {
        Some(c) => c,
        None => return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": {
                "code": "AUTO_LOGIN_NOT_CONFIGURED",
                "message": format!(
                    "No auto_login config for session '{}'. \
                     Create '%APPDATA%\\webview-bridge\\profiles\\{}\\auto_login.toml' \
                     or add [session.auto_login.{}] to config.toml.",
                    request.name, request.name, request.name
                ),
                "next_action": format!(
                    "Create the file '%APPDATA%\\webview-bridge\\profiles\\{}\\auto_login.toml' \
                     with fields: op_item, login_url, username_selector, password_selector, submit_selector",
                    request.name
                )
            }
        }))).into_response(),
    };

    // --- 2. Resolve credentials (1Password or plaintext fallback) ---
    // Priority: 1Password (op_item in config/request) > plaintext username/password in config
    let op_path_opt = crate::auto_login::find_op_binary(op_path_cfg.as_deref());
    let has_op = op_path_opt.is_some();
    let has_plaintext = auto_login_cfg.username.is_some() && auto_login_cfg.password.is_some();
    let wants_op = request.op_item.is_some() || auto_login_cfg.op_item.is_some();

    // Determine credential source
    let creds = if has_op && (wants_op || !has_plaintext) {
        // --- OP path ---
        let op_path = op_path_opt.as_deref().unwrap().to_string();

        // Determine which 1Password item to use
        // Priority: request.op_item > config op_item > cached op_item_id > session name
        let op_item = if let Some(ref item) = request.op_item {
            item.clone()
        } else if request.force {
            match auto_login_cfg.op_item.clone() {
                Some(i) => i,
                None => return (StatusCode::BAD_REQUEST, Json(json!({
                    "success": false,
                    "error": {
                        "code": "OP_ITEM_REQUIRED",
                        "message": "force=true requires op_item in config or in the request body"
                    }
                }))).into_response(),
            }
        } else {
            manager.get_auto_login_op_item_id(&request.name)
                .or_else(|| auto_login_cfg.op_item.clone())
                .unwrap_or_else(|| request.name.clone())
        };

        let op_path_clone = op_path.clone();
        let op_item_clone = op_item.clone();
        let op_vault_clone = auto_login_cfg.op_vault.clone();
        match tokio::task::spawn_blocking(move || {
            crate::auto_login::fetch_credentials(&op_path_clone, &op_item_clone, op_vault_clone.as_deref())
        }).await {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": {
                    "code": "CREDENTIALS_FETCH_FAILED",
                    "message": format!("Failed to get credentials from 1Password: {}", e)
                }
            }))).into_response(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": {
                    "code": "TASK_PANIC",
                    "message": format!("Credential fetch task panicked: {}", e)
                }
            }))).into_response(),
        }
    } else if has_plaintext {
        // --- Plaintext fallback ---
        tracing::warn!("[AutoLogin] Session '{}': using plaintext credentials (no 1Password). Consider using op_item for security.", request.name);
        crate::auto_login::Credentials {
            op_item_id: String::new(),
            username: auto_login_cfg.username.clone().unwrap(),
            password: auto_login_cfg.password.clone().unwrap(),
        }
    } else {
        // --- Neither OP nor plaintext configured ---
        let msg = if !has_op {
            "1Password CLI (op) not found and no plaintext credentials configured. \
             Install op via: winget install 1password-cli, \
             or set username/password in auto_login.toml (insecure fallback)."
        } else {
            "No credentials configured. Set op_item (recommended) or username+password in auto_login.toml."
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": { "code": "NO_CREDENTIALS", "message": msg }
        }))).into_response();
    };

    // Keep op_path for OTP use later (may be None if plaintext path was taken)
    let op_path = crate::auto_login::find_op_binary(op_path_cfg.as_deref()).unwrap_or_default();

    // --- 5. Get session handle ---
    let handle = match manager.get_handle(&request.name) {
        Some(h) => h,
        None => return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": {
                "code": "SESSION_NOT_ACTIVE",
                "message": format!(
                    "Session '{}' is not active. Call POST /session/acquire first.",
                    request.name
                )
            }
        }))).into_response(),
    };

    // --- 6. Check if already logged in (skip if logged_in_selector found) ---
    if let Some(ref sel) = auto_login_cfg.logged_in_selector {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
            id: handle.id.clone(),
            selector: sel.clone(),
            timeout_ms: 2000,
            frame: None,
            resp_tx: tx,
        });
        if let Ok(Ok(Ok(_))) = tokio::time::timeout(Duration::from_secs(3), rx).await {
            let _ = manager.record_auto_login_skipped(&request.name);
            return (StatusCode::OK, Json(json!({
                "success": true,
                "logged_in": true,
                "already_logged_in": true,
                "username": creds.username,
            }))).into_response();
        }
    }

    // --- 7. Navigate to login URL if configured ---
    if let Some(ref login_url) = auto_login_cfg.login_url {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::Navigate {
            id: handle.id.clone(),
            url: login_url.clone(),
            resp_tx: tx,
        });
        let _ = tokio::time::timeout(Duration::from_secs(15), rx).await;
    }

    // --- 8. Wait for username field ---
    {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
            id: handle.id.clone(),
            selector: auto_login_cfg.username_selector.clone(),
            timeout_ms: 10000,
            frame: None,
            resp_tx: tx,
        });
        if let Err(_) | Ok(Ok(Err(_))) | Ok(Err(_)) =
            tokio::time::timeout(Duration::from_secs(12), rx).await
        {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "success": false,
                "error": {
                    "code": "LOGIN_FORM_NOT_FOUND",
                    "message": format!(
                        "Username field '{}' not found within 10s. Check login_url and username_selector in config.",
                        auto_login_cfg.username_selector
                    )
                }
            }))).into_response();
        }
    }

    // --- 9. Fill username via JS (sets .value + fires input/change events, works with SPAs) ---
    {
        let (tx, rx) = oneshot::channel();
        let script = js_fill_input(&auto_login_cfg.username_selector, &creds.username);
        let _ = state.cmd_tx.send(AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        });
        if let Ok(Ok(Ok(result))) = tokio::time::timeout(Duration::from_secs(5), rx).await {
            tracing::debug!("[AutoLogin] fill username result: {}", result);
        }
    }
    if let Some(url) = auto_login_debug_screenshot(&state.cmd_tx, &handle.id, "01_username_filled", debug).await {
        debug_screenshots.push(json!({"step": "username_filled", "url": url}));
    }

    // --- 10. Fill password via CDP key events (TypeCdp fires keydown/char/keyup per char) ---
    {
        // Click field first to focus
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::ClickCdp {
            id: handle.id.clone(),
            selector: auto_login_cfg.password_selector.clone(),
            human_mode: false,
            resp_tx: tx,
        });
        let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
    }
    {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::TypeCdp {
            id: handle.id.clone(),
            text: creds.password.clone(),
            char_delay_ms: 30,
            human_mode: false,
            resp_tx: tx,
        });
        if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(15), rx).await {
            tracing::debug!("[AutoLogin] fill password: {:?}", r);
        }
    }
    if let Some(url) = auto_login_debug_screenshot(&state.cmd_tx, &handle.id, "02_password_filled", debug).await {
        debug_screenshots.push(json!({"step": "password_filled", "url": url}));
    }

    // --- 11. Submit via CDP physical click (get_element_center prefers visible element) ---
    {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::ClickCdp {
            id: handle.id.clone(),
            selector: auto_login_cfg.submit_selector.clone(),
            human_mode: false,
            resp_tx: tx,
        });
        if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(5), rx).await {
            tracing::info!("[AutoLogin] submit click: {:?}", r);
        }
    }
    if let Some(url) = auto_login_debug_screenshot(&state.cmd_tx, &handle.id, "03_after_submit", debug).await {
        debug_screenshots.push(json!({"step": "after_submit", "url": url}));
    }

    // --- 12. Handle OTP/TOTP if configured ---
    if auto_login_cfg.otp_selector.is_some() && auto_login_cfg.otp_op_ref.is_some() {
        let otp_sel = auto_login_cfg.otp_selector.as_ref().unwrap().clone();
        let otp_ref = auto_login_cfg.otp_op_ref.as_ref().unwrap().clone();

        // Wait for OTP field to appear
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
            id: handle.id.clone(),
            selector: otp_sel.clone(),
            timeout_ms: 10000,
            frame: None,
            resp_tx: tx,
        });
        if let Ok(Ok(Ok(_))) = tokio::time::timeout(Duration::from_secs(12), rx).await {
            // Fetch TOTP code from 1Password
            let op_path_otp = op_path.clone();
            let totp_code = tokio::task::spawn_blocking(move || {
                crate::auto_login::read_secret(&op_path_otp, &otp_ref)
            }).await;

            if let Ok(Ok(code)) = totp_code {
                // Click OTP field
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::ClickCdp {
                    id: handle.id.clone(),
                    selector: otp_sel.clone(),
                    human_mode: false,
                    resp_tx: tx,
                });
                let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;

                // Type OTP code
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::TypeCdp {
                    id: handle.id.clone(),
                    text: code,
                    char_delay_ms: 0,
                    human_mode: false,
                    resp_tx: tx,
                });
                let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;

                // Press Enter to submit OTP
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::PressKeyCdp {
                    id: handle.id.clone(),
                    key: "Return".to_string(),
                    resp_tx: tx,
                });
                let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
            }
        }
    }

    // --- 13. Verify login success ---
    let login_success = if let Some(ref sel) = auto_login_cfg.logged_in_selector {
        let (tx, rx) = oneshot::channel();
        let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
            id: handle.id.clone(),
            selector: sel.clone(),
            timeout_ms: 15000,
            frame: None,
            resp_tx: tx,
        });
        matches!(
            tokio::time::timeout(Duration::from_secs(17), rx).await,
            Ok(Ok(Ok(_)))
        )
    } else {
        // No selector to verify — assume success after waiting briefly for navigation
        tokio::time::sleep(Duration::from_secs(2)).await;
        true
    };

    if !login_success {
        let err = "Login form submitted but logged_in_selector was not found within 15s.";
        let _ = manager.record_auto_login_failure(&request.name, err);
        tracing::warn!("[AutoLogin] Session '{}' login failed. Cache cleared.", request.name);
        return (StatusCode::OK, Json(json!({
            "success": false,
            "logged_in": false,
            "error": {
                "code": "LOGIN_VERIFICATION_FAILED",
                "message": err,
                "next_action": "Retry with force=true to re-fetch credentials, or verify logged_in_selector and selectors in auto_login.toml."
            }
        }))).into_response();
    }

    // --- 14. Process extra_steps (multi-stage auth, e.g. RMS → Rakuten SSO) ---
    let mut steps_completed: usize = 0;
    for (step_idx, step) in auto_login_cfg.extra_steps.iter().enumerate() {
        tracing::info!("[AutoLogin] Session '{}' — extra step {}", request.name, step_idx + 1);

        // Wait for URL to contain the expected pattern
        if let Some(ref url_fragment) = step.wait_url_contains {
            let frag = url_fragment.clone();
            let session_id = handle.id.clone();
            // Poll current URL every 500ms for up to 15s
            let mut found = false;
            for _ in 0..30 {
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::ExecuteScript {
                    id: session_id.clone(),
                    script: "location.href".to_string(),
                    resp_tx: tx,
                });
                if let Ok(Ok(Ok(url_str))) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    let url_clean = url_str.trim_matches('"').to_string();
                    if url_clean.contains(frag.as_str()) {
                        found = true;
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            if !found {
                tracing::warn!(
                    "[AutoLogin] Step {}: URL pattern '{}' not reached within 15s. Skipping remaining steps.",
                    step_idx + 1, url_fragment
                );
                break;
            }
            tracing::info!("[AutoLogin] Step {}: URL pattern '{}' matched.", step_idx + 1, url_fragment);
        }

        // Fetch credentials for this step
        // Priority: step.op_item (OP) > step.username/password (plaintext) > reuse main creds
        let step_creds = if let Some(ref item) = step.op_item {
            let op_path_s = op_path.clone();
            let item_s = item.clone();
            let vault_s = step.op_vault.clone().or_else(|| auto_login_cfg.op_vault.clone());
            match tokio::task::spawn_blocking(move || {
                crate::auto_login::fetch_credentials(&op_path_s, &item_s, vault_s.as_deref())
            }).await {
                Ok(Ok(c)) => Some(c),
                Ok(Err(e)) => {
                    tracing::warn!("[AutoLogin] Step {}: Failed to fetch 1Password item '{}': {}", step_idx + 1, item, e);
                    None
                }
                Err(_) => None,
            }
        } else if step.username.is_some() && step.password.is_some() {
            // Plaintext fallback for this step
            Some(crate::auto_login::Credentials {
                op_item_id: String::new(),
                username: step.username.clone().unwrap(),
                password: step.password.clone().unwrap(),
            })
        } else {
            // Reuse main credentials if no separate config
            Some(crate::auto_login::Credentials {
                op_item_id: creds.op_item_id.clone(),
                username: creds.username.clone(),
                password: creds.password.clone(),
            })
        };

        // Fill username if selector provided
        if let Some(ref sel) = step.username_selector {
            // Wait for the username field
            let (tx, rx) = oneshot::channel();
            let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
                id: handle.id.clone(),
                selector: sel.clone(),
                timeout_ms: 10000,
                frame: None,
                resp_tx: tx,
            });
            let _ = tokio::time::timeout(Duration::from_secs(12), rx).await;

            // Fill username via JS
            if let Some(ref sc) = step_creds {
                let (tx, rx) = oneshot::channel();
                let script = js_fill_input(sel, &sc.username);
                let _ = state.cmd_tx.send(AppCommand::ExecuteScript {
                    id: handle.id.clone(),
                    script,
                    resp_tx: tx,
                });
                if let Ok(Ok(Ok(r))) = tokio::time::timeout(Duration::from_secs(5), rx).await {
                    tracing::debug!("[AutoLogin] step {} fill username: {}", step_idx + 1, r);
                }
            }
            if let Some(url) = auto_login_debug_screenshot(
                &state.cmd_tx, &handle.id,
                &format!("s{}_username_filled", step_idx + 1), debug).await {
                debug_screenshots.push(json!({"step": format!("extra_{}_username_filled", step_idx+1), "url": url}));
            }

            // Click "Next" button — use CDP physical click (get_element_center picks visible element)
            if let Some(ref next_sel) = step.next_selector {
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::ClickCdp {
                    id: handle.id.clone(),
                    selector: next_sel.clone(),
                    human_mode: false,
                    resp_tx: tx,
                });
                if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(5), rx).await {
                    tracing::info!("[AutoLogin] step {} next click: {:?}", step_idx + 1, r);
                }
                if let Some(url) = auto_login_debug_screenshot(
                    &state.cmd_tx, &handle.id,
                    &format!("s{}_next_clicked", step_idx + 1), debug).await {
                    debug_screenshots.push(json!({"step": format!("extra_{}_next_clicked", step_idx+1), "url": url}));
                }

                // Wait for the password field to appear
                if let Some(ref wait_sel) = step.wait_password_selector {
                    let (tx, rx) = oneshot::channel();
                    let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
                        id: handle.id.clone(),
                        selector: wait_sel.clone(),
                        timeout_ms: 10000,
                        frame: None,
                        resp_tx: tx,
                    });
                    let _ = tokio::time::timeout(Duration::from_secs(12), rx).await;
                    // Extra wait for SPA transition animation to fully complete before interacting.
                    // Without this, TypeCdp events land on the pre-animation DOM element which
                    // React discards when it re-mounts the input after the transition.
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
            }
        }

        // Fill password via CDP key events (TypeCdp) — SPAs may require keydown/keyup to enable submit
        if let Some(ref sel) = step.password_selector {
            if let Some(ref sc) = step_creds {
                // Click to focus the field
                {
                    let (tx, rx) = oneshot::channel();
                    let _ = state.cmd_tx.send(AppCommand::ClickCdp {
                        id: handle.id.clone(),
                        selector: sel.clone(),
                        human_mode: false,
                        resp_tx: tx,
                    });
                    let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
                }
                // Type password via CDP keyboard events (fires keydown/char/keyup per char)
                {
                    let (tx, rx) = oneshot::channel();
                    let _ = state.cmd_tx.send(AppCommand::TypeCdp {
                        id: handle.id.clone(),
                        text: sc.password.clone(),
                        char_delay_ms: 30,
                        human_mode: false,
                        resp_tx: tx,
                    });
                    if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(15), rx).await {
                        tracing::debug!("[AutoLogin] step {} type password: {:?}", step_idx + 1, r);
                    }
                }
                // Wait for SPA to process key events and enable the submit button
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        if let Some(url) = auto_login_debug_screenshot(
            &state.cmd_tx, &handle.id,
            &format!("s{}_password_filled", step_idx + 1), debug).await {
            debug_screenshots.push(json!({"step": format!("extra_{}_password_filled", step_idx+1), "url": url}));
        }

        // --- Pre-submit: wait for async bot-detection challenges (PoW etc.) ---
        // 1. If challenge_done_js is set, poll until it returns truthy.
        if let Some(ref challenge_js) = step.challenge_done_js {
            let deadline = tokio::time::Instant::now()
                + Duration::from_millis(step.challenge_timeout_ms);
            let mut challenge_ready = false;
            while tokio::time::Instant::now() < deadline {
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::ExecuteScript {
                    id: handle.id.clone(),
                    script: format!("(function(){{ try {{ return !!({js}); }} catch(e) {{ return false; }} }})()", js = challenge_js),
                    resp_tx: tx,
                });
                if let Ok(Ok(Ok(result))) = tokio::time::timeout(Duration::from_secs(3), rx).await {
                    let ready = result.trim() == "true";
                    if ready {
                        challenge_ready = true;
                        tracing::info!("[AutoLogin] step {} challenge ready.", step_idx + 1);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            if !challenge_ready {
                tracing::warn!("[AutoLogin] step {} challenge_done_js timed out — submitting anyway.", step_idx + 1);
            }
        }
        // 2. Fixed pre-submit wait (catches timing issues even without challenge_done_js).
        if step.pre_submit_wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(step.pre_submit_wait_ms)).await;
        }

        // Click submit — use CDP physical click (get_element_center prefers visible elements)
        if let Some(ref sel) = step.submit_selector {
            let (tx, rx) = oneshot::channel();
            let _ = state.cmd_tx.send(AppCommand::ClickCdp {
                id: handle.id.clone(),
                selector: sel.clone(),
                human_mode: false,
                resp_tx: tx,
            });
            if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(5), rx).await {
                tracing::info!("[AutoLogin] step {} submit: {:?}", step_idx + 1, r);
            }
            // Also try Enter key as fallback (some SPAs respond to keyboard submit)
            tokio::time::sleep(Duration::from_millis(300)).await;
            {
                let (tx, rx) = oneshot::channel();
                let _ = state.cmd_tx.send(AppCommand::PressKeyCdp {
                    id: handle.id.clone(),
                    key: "Return".to_string(),
                    resp_tx: tx,
                });
                let _ = tokio::time::timeout(Duration::from_secs(3), rx).await;
            }
            // Small delay to let navigation start before snapping
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Some(url) = auto_login_debug_screenshot(
                &state.cmd_tx, &handle.id,
                &format!("s{}_after_submit", step_idx + 1), debug).await {
                debug_screenshots.push(json!({"step": format!("extra_{}_after_submit", step_idx+1), "url": url}));
            }
        }

        // Handle OTP for this step
        if step.otp_selector.is_some() && step.otp_op_ref.is_some() {
            let otp_sel = step.otp_selector.as_ref().unwrap().clone();
            let otp_ref = step.otp_op_ref.as_ref().unwrap().clone();
            let (tx, rx) = oneshot::channel();
            let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
                id: handle.id.clone(),
                selector: otp_sel.clone(),
                timeout_ms: 10000,
                frame: None,
                resp_tx: tx,
            });
            if let Ok(Ok(Ok(_))) = tokio::time::timeout(Duration::from_secs(12), rx).await {
                let op_path_otp = op_path.clone();
                if let Ok(Ok(code)) = tokio::task::spawn_blocking(move || {
                    crate::auto_login::read_secret(&op_path_otp, &otp_ref)
                }).await {
                    let (tx, rx) = oneshot::channel();
                    let script = js_fill_input(&otp_sel, &code);
                    let _ = state.cmd_tx.send(AppCommand::ExecuteScript {
                        id: handle.id.clone(),
                        script,
                        resp_tx: tx,
                    });
                    let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
                    let (tx, rx) = oneshot::channel();
                    let _ = state.cmd_tx.send(AppCommand::PressKeyCdp {
                        id: handle.id.clone(),
                        key: "Return".to_string(),
                        resp_tx: tx,
                    });
                    let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
                }
            }
        }

        // Verify step completion
        if let Some(ref done_sel) = step.done_selector {
            let (tx, rx) = oneshot::channel();
            let _ = state.cmd_tx.send(AppCommand::WaitForSelector {
                id: handle.id.clone(),
                selector: done_sel.clone(),
                timeout_ms: 15000,
                frame: None,
                resp_tx: tx,
            });
            let step_ok = matches!(
                tokio::time::timeout(Duration::from_secs(17), rx).await,
                Ok(Ok(Ok(_)))
            );
            if step_ok {
                tracing::info!("[AutoLogin] Step {} completed (done_selector found).", step_idx + 1);
            } else {
                tracing::warn!("[AutoLogin] Step {}: done_selector '{}' not found after submit.", step_idx + 1, done_sel);
            }
        } else {
            // No done_selector — wait briefly for navigation
            tokio::time::sleep(Duration::from_secs(2)).await;
            tracing::info!("[AutoLogin] Step {} done (no done_selector, waited 2s).", step_idx + 1);
        }
        steps_completed += 1;
    }

    // --- 15. All steps completed — record success and return ---
    let _ = manager.record_auto_login_success(&request.name, &creds.op_item_id, &creds.username);
    tracing::info!(
        "[AutoLogin] Session '{}' logged in (item_id='{}', user='{}'), {} extra steps",
        request.name, creds.op_item_id, creds.username, steps_completed
    );
    let mut resp = json!({
        "success": true,
        "logged_in": true,
        "username": creds.username,
        "op_item_id": creds.op_item_id,
        "extra_steps_completed": steps_completed,
    });
    if debug && !debug_screenshots.is_empty() {
        resp["debug_screenshots"] = json!(debug_screenshots);
    }
    (StatusCode::OK, Json(resp)).into_response()
}

// ============================================================================
// GET /session/auto-login — Auth status for a single session
// GET /session/auto-login/list — Auth status for all sessions
// ============================================================================

/// Query params for GET /session/auto-login
#[derive(serde::Deserialize)]
struct AutoLoginStatusQuery {
    name: String,
}

/// GET /session/auto-login?name=<session>
///
/// Returns the auto-login configuration status and login history for a session.
/// Use this to determine whether to call POST /session/auto-login.
///
/// AI usage: call this before attempting any action that requires authentication.
async fn session_auto_login_status(
    axum::extract::Query(query): axum::extract::Query<AutoLoginStatusQuery>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();
    let name = &query.name;

    // Check if session exists
    let session_exists = manager.get_handle(name).is_some() || {
        manager.list().ok()
            .map(|r| r.sessions.into_iter().any(|s| s.name == *name))
            .unwrap_or(false)
    };

    // Check if auto-login is configured
    let configured = crate::core::config::load_session_auto_login(name).is_some();
    let config_path = crate::core::config::AppConfig::session_auto_login_path(name);
    let has_session_file = config_path.exists();

    // Get login history
    let state = manager.get_auto_login_state(name);

    let next_action = build_next_action(name, configured, &state);

    (StatusCode::OK, Json(json!({
        "session": name,
        "session_exists": session_exists,
        "configured": configured,
        "config_source": if has_session_file { "session_file" } else if configured { "config_toml" } else { "none" },
        "config_file_path": config_path.to_string_lossy(),
        "last_result": state.as_ref().and_then(|s| s.last_result.as_deref()),
        "last_at": state.as_ref().and_then(|s| s.last_at.as_deref()),
        "last_username": state.as_ref().and_then(|s| s.last_username.as_deref()),
        "last_error": state.as_ref().and_then(|s| s.last_error.as_deref()),
        "attempt_count": state.as_ref().map(|s| s.attempt_count).unwrap_or(0),
        "consecutive_failures": state.as_ref().map(|s| s.consecutive_failures).unwrap_or(0),
        "cached_op_item_id": state.as_ref().and_then(|s| s.op_item_id.as_deref()),
        "next_action": next_action,
    }))).into_response()
}

/// GET /session/auto-login/list
///
/// Returns auto-login status for ALL sessions that have been configured or attempted.
/// Use this to audit which sessions need re-authentication.
async fn session_auto_login_list() -> impl IntoResponse {
    let manager = get_session_manager_v2();
    let all_sessions = manager.list().map(|r| r.sessions).unwrap_or_default();

    let mut items = Vec::new();
    for session_info in &all_sessions {
        let name = &session_info.name;
        let configured = crate::core::config::load_session_auto_login(name).is_some();
        let state = manager.get_auto_login_state(name);
        let next_action = build_next_action(name, configured, &state);
        let config_path = crate::core::config::AppConfig::session_auto_login_path(name);

        items.push(json!({
            "session": name,
            "active": session_info.active,
            "configured": configured,
            "config_source": if config_path.exists() { "session_file" } else if configured { "config_toml" } else { "none" },
            "last_result": state.as_ref().and_then(|s| s.last_result.as_deref()),
            "last_at": state.as_ref().and_then(|s| s.last_at.as_deref()),
            "last_username": state.as_ref().and_then(|s| s.last_username.as_deref()),
            "consecutive_failures": state.as_ref().map(|s| s.consecutive_failures).unwrap_or(0),
            "next_action": next_action,
        }));
    }

    (StatusCode::OK, Json(json!({
        "success": true,
        "count": items.len(),
        "sessions": items,
    }))).into_response()
}

/// Build a human+AI readable `next_action` hint based on current state
fn build_next_action(
    name: &str,
    configured: bool,
    state: &Option<crate::core::session_v2::AutoLoginState>,
) -> String {
    if !configured {
        return format!(
            "Create '%APPDATA%\\webview-bridge\\profiles\\{}\\auto_login.toml' with login selectors and op_item",
            name
        );
    }
    match state.as_ref().and_then(|s| s.last_result.as_deref()) {
        None => format!("No login attempt yet. Call POST /session/auto-login {{\"name\": \"{}\"}}", name),
        Some("success") | Some("already_logged_in") => format!(
            "Last login succeeded. If session expired, call POST /session/auto-login {{\"name\": \"{}\", \"force\": true}}",
            name
        ),
        Some("failed") => {
            let failures = state.as_ref().map(|s| s.consecutive_failures).unwrap_or(0);
            if failures >= 3 {
                format!(
                    "Login failed {} times. Check 1Password credentials and selectors in auto_login.toml, then retry with force=true",
                    failures
                )
            } else {
                format!(
                    "Last login failed. Retry: POST /session/auto-login {{\"name\": \"{}\", \"force\": true}}",
                    name
                )
            }
        }
        _ => format!("Call POST /session/auto-login {{\"name\": \"{}\"}}", name),
    }
}

// ============================================================================
// GET /session/auto-login/config  — read config
// PUT /session/auto-login/config  — write config
// ============================================================================

/// Query params for GET /session/auto-login/config
#[derive(serde::Deserialize)]
struct AutoLoginConfigQuery {
    name: String,
}

/// GET /session/auto-login/config?name=<session>
///
/// Returns the auto-login configuration for a session as JSON.
/// Returns 404 if not configured.
async fn session_auto_login_config_get(
    Query(q): Query<AutoLoginConfigQuery>,
) -> impl IntoResponse {
    let path = crate::core::config::AppConfig::session_auto_login_path(&q.name);
    match crate::core::config::load_session_auto_login(&q.name) {
        Some(cfg) => Json(json!({
            "success": true,
            "session": q.name,
            "config": cfg,
            "path": path.to_string_lossy(),
        })).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({
            "success": false,
            "error": {
                "code": "AUTO_LOGIN_NOT_CONFIGURED",
                "message": format!("No auto_login config for session '{}'.", q.name),
                "path": path.to_string_lossy(),
            }
        }))).into_response(),
    }
}

/// Request body for PUT /session/auto-login/config
#[derive(serde::Deserialize)]
struct AutoLoginConfigSetRequest {
    /// Session name
    name: String,
    /// Config fields (merged into existing or created fresh)
    #[serde(flatten)]
    config: crate::core::config::AutoLoginConfig,
}

/// PUT /session/auto-login/config
///
/// Saves auto-login configuration for a session.
/// Creates or overwrites `profiles/{name}/auto_login.toml`.
async fn session_auto_login_config_set(
    Json(request): Json<AutoLoginConfigSetRequest>,
) -> impl IntoResponse {
    let path = crate::core::config::AppConfig::session_auto_login_path(&request.name);

    // Ensure profile directory exists
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": { "code": "DIR_CREATE_FAILED", "message": format!("{}", e) }
            }))).into_response();
        }
    }

    // Serialize config to TOML
    let toml_str = match toml::to_string_pretty(&request.config) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": { "code": "SERIALIZE_FAILED", "message": format!("{}", e) }
        }))).into_response(),
    };

    if let Err(e) = std::fs::write(&path, &toml_str) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": { "code": "WRITE_FAILED", "message": format!("{}", e) }
        }))).into_response();
    }

    Json(json!({
        "success": true,
        "session": request.name,
        "path": path.to_string_lossy(),
        "config": request.config,
    })).into_response()
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
                    "message": format!("Session '{}' not found. Run 'wb session acquire {}' or POST /session/acquire to create/activate it.", name, name)
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
                    "message": format!("Session '{}' not found. Run 'wb session acquire {}' or POST /session/acquire to create/activate it.", name, name)
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
    /// Extra wait after page load completes (ms). Useful for JS-heavy pages that render
    /// charts/graphs after the load event. Defaults to 0.
    #[serde(default)]
    post_load_wait_ms: u64,
    /// If true (default), include a snapshot of interactive elements in the response.
    /// Eliminates the need for a separate POST /snapshot call after navigation.
    #[serde(default = "default_snapshot_after_nav")]
    snapshot: bool,
}

fn default_wait_until() -> String { "load".to_string() }
fn default_nav_timeout() -> u64 { 30000 }
fn default_snapshot_after_nav() -> bool { true }

/// Build contextual hints for navigate response.
/// Detects login-page redirects and surfaces auto-login availability.
pub fn build_navigate_hints(session: &str, requested_url: &str, final_url: &str) -> Vec<String> {
    let mut hints = Vec::new();

    // Check if we were redirected away from the requested URL
    let was_redirected = !final_url.is_empty()
        && requested_url != final_url
        && !requested_url.starts_with(final_url)
        && !final_url.starts_with(requested_url);

    // Detect login page by URL keywords
    let login_keywords = ["login", "signin", "sign-in", "auth", "sso", "glogin", "r-login"];
    let final_lower = final_url.to_lowercase();
    let looks_like_login = login_keywords.iter().any(|kw| final_lower.contains(kw));

    if was_redirected && looks_like_login {
        // Check if auto-login is configured for this session
        if let Some(cfg) = crate::core::config::load_session_auto_login(session) {
            let login_url = cfg.login_url.as_deref().unwrap_or(final_url);
            hints.push(format!(
                "Redirected to login page ({final_url}). \
                Auto-login is configured — call POST /session/auto-login {{\"name\":\"{session}\"}} \
                to authenticate automatically instead of filling the form manually."
            ));
            let _ = login_url; // suppress unused warning
        } else {
            hints.push(format!(
                "Redirected to login page ({final_url}). \
                Configure auto-login with PUT /session/auto-login/config to automate future logins."
            ));
        }
    } else if looks_like_login {
        // Navigated directly to a login page
        if crate::core::config::load_session_auto_login(session).is_some() {
            hints.push(format!(
                "This looks like a login page. \
                Auto-login is configured — call POST /session/auto-login {{\"name\":\"{session}\"}} \
                to authenticate automatically."
            ));
        }
    }

    hints
}

/// Execute the standard interactive-element snapshot JS and return the parsed result.
/// Used by navigate_v2 to bundle snapshot into navigation response.
pub async fn run_snapshot_for_session(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AppCommand>,
    handle_id: &str,
) -> Option<serde_json::Value> {
    let script = format!(
        r#"(function(){{
  document.querySelectorAll('[data-wb-ref]').forEach(el => el.removeAttribute('data-wb-ref'));
  const scope = document.body;
  const sels = 'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="radio"],[onclick],[tabindex]:not([tabindex="-1"])';
  const els = [...scope.querySelectorAll(sels)].filter(el => {{
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) return false;
    const s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden';
  }}).slice(0, 200);
  let id = 1;
  const results = els.map(el => {{
    const ref = 'e' + id++;
    el.setAttribute('data-wb-ref', ref);
    return {{
      ref, tag: el.tagName.toLowerCase(),
      type: el.type || null,
      role: el.getAttribute('role'),
      text: (el.textContent||'').trim().slice(0,80),
      name: el.getAttribute('name') || el.getAttribute('aria-label'),
      href: el.href || null,
      value: el.type === 'password' ? null : (el.value || null),
      placeholder: el.placeholder || null,
      checked: el.checked === true ? true : null,
      disabled: el.disabled === true ? true : null
    }};
  }});
  return JSON.stringify({{ title: document.title, url: location.href, elements: results }});
}})()"#
    );
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ExecuteScript {
        id: handle_id.to_string(),
        script,
        resp_tx: tx,
    };
    if cmd_tx.send(cmd).is_err() {
        return None;
    }
    match tokio::time::timeout(std::time::Duration::from_secs(8), rx).await {
        Ok(Ok(Ok(result))) => serde_json::from_str(&result).ok(),
        _ => None,
    }
}

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
        id: session_id.clone(),
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
        Ok(Ok(Ok(()))) => {
            // Auto-save last URL for restore-on-acquire
            let _ = manager.update_last_url(&request.session, &request.url);
            // Optional post-load idle wait for JS-heavy pages (charts, realtime dashboards)
            if request.post_load_wait_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(request.post_load_wait_ms)).await;
            }
            // Include snapshot of interactive elements unless caller opted out
            let snapshot = if request.snapshot {
                run_snapshot_for_session(&state.cmd_tx, &session_id).await
            } else {
                None
            };
            let elem_count = snapshot.as_ref()
                .and_then(|s| s.get("elements"))
                .and_then(|e| e.as_array())
                .map(|a| a.len());
            // Detect login-page redirect and surface auto-login hint
            let final_url = snapshot.as_ref()
                .and_then(|s| s.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or(&request.url);
            let hints = build_navigate_hints(&request.session, &request.url, final_url);
            let mut resp = json!({
                "success": true,
                "session": request.session,
                "url": request.url,
                "final_url": final_url,
                "load_time_ms": start.elapsed().as_millis() as u64
            });
            if !hints.is_empty() {
                resp["hints"] = json!(hints);
            }
            if let Some(snap) = snapshot {
                resp["snapshot"] = snap;
                if let Some(n) = elem_count {
                    resp["element_count"] = json!(n);
                }
            }
            (StatusCode::OK, Json(resp)).into_response()
        },
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost. The session may have crashed. Try: POST /session/acquire to re-acquire, or 'wb session acquire <name>'."),
        Err(_) => error_response(Wbp2Error::InternalError, "Navigation timed out. The page may be slow or unresponsive. Try: increase timeout_ms or check the URL."),
    }
}

/// Request for POST /v2/click
#[derive(serde::Deserialize)]
struct ClickRequest {
    session: String,
    selector: String,
    #[serde(default)]
    wait_after_ms: Option<u64>,
    #[serde(default)]
    frame: Option<String>,
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
    let cmd = if let Some(ref frame) = request.frame {
        AppCommand::ExecuteInFrame {
            id: handle.id.clone(),
            script,
            frame: frame.clone(),
            resp_tx: tx,
        }
    } else {
        AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        }
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
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost. The session may have crashed. Try: POST /session/acquire to re-acquire, or 'wb session acquire <name>'."),
        Err(_) => error_response(Wbp2Error::InternalError, "Click timed out. The element may be missing or hidden. Try: check selector, wait for page load, or increase timeout."),
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
    #[serde(default)]
    frame: Option<String>,
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
    let cmd = if let Some(ref frame) = request.frame {
        AppCommand::ExecuteInFrame {
            id: handle.id.clone(),
            script,
            frame: frame.clone(),
            resp_tx: tx,
        }
    } else {
        AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        }
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
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost. The session may have crashed. Try: POST /session/acquire to re-acquire, or 'wb session acquire <name>'."),
        Err(_) => error_response(Wbp2Error::InternalError, "Type timed out. The input element may not be focused or visible. Try: click the element first, then type."),
    }
}

/// Request for POST /v2/execute
#[derive(serde::Deserialize)]
struct ExecuteRequest {
    session: String,
    script: String,
    #[serde(default = "default_execute_timeout")]
    timeout_ms: u64,
    /// Optional frame specifier (URL substring, frame name, or frame ID).
    /// When set, executes the script inside the matching iframe via CDP.
    #[serde(default)]
    frame: Option<String>,
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
    
    // Auto-wrap in IIFE when the script uses top-level `return` or `await`.
    //
    // Rules:
    //  • Contains `await ` → async IIFE: `(async function(){ ... })()`
    //    WebView2 awaits the returned Promise, so `return await fetch(...)` works correctly.
    //  • Contains `return ` but no `await` → sync IIFE: `(function(){ ... })()`
    //  • Already wrapped (starts with `(function`, `(async`, `((`) → leave as-is
    //
    // This covers the common AI-generated patterns:
    //   "return await fetch(url).then(r => r.json())"
    //   "return await new Promise(resolve => setTimeout(resolve, 500))"
    let script = {
        let trimmed = request.script.trim();
        let already_wrapped = trimmed.starts_with("(function")
            || trimmed.starts_with("(async")
            || trimmed.starts_with("((");
        let uses_await = trimmed.contains("await ");
        let uses_return = trimmed.contains("return ");
        if !already_wrapped && (uses_return || uses_await) {
            if uses_await {
                format!("(async function(){{{}}})();", request.script)
            } else {
                format!("(function(){{{}}})();", request.script)
            }
        } else {
            request.script.clone()
        }
    };

    let (tx, rx) = oneshot::channel();
    let cmd = if let Some(ref frame) = request.frame {
        AppCommand::ExecuteInFrame {
            id: handle.id.clone(),
            script,
            frame: frame.clone(),
            resp_tx: tx,
        }
    } else {
        AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        }
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
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost. The session may have crashed. Try: POST /session/acquire to re-acquire, or 'wb session acquire <name>'."),
        Err(_) => error_response(Wbp2Error::InternalError, "JavaScript execution timed out. The script may have an infinite loop or be waiting for a resource. Try: simplify the script or increase timeout_ms."),
    }
}

// ============================================================================
// POST /snapshot — DOM element extraction (use instead of screenshots for AI)
// ============================================================================

/// Request body for POST /snapshot
#[derive(serde::Deserialize)]
struct SnapshotRequest {
    /// Session name
    session: String,
    /// If true, include non-interactive elements (headings, paragraphs, images, tables)
    #[serde(default)]
    all: bool,
    /// CSS selector to scope extraction to a subtree (default: body)
    #[serde(default)]
    within: Option<String>,
    /// Maximum number of elements to return (default: 200)
    #[serde(default = "default_snapshot_limit")]
    limit: usize,
    /// Optional frame specifier (URL substring, frame name, or frame ID)
    #[serde(default)]
    frame: Option<String>,
}

fn default_snapshot_limit() -> usize { 200 }

/// POST /snapshot
///
/// Extracts all interactive elements from the page DOM and returns them as
/// numbered references (e1, e2, ...) that can be used with /click, /type, etc.
///
/// **Prefer this over /screenshot** for AI navigation — it's faster, uses less
/// bandwidth, and gives structured data (selector refs, text, type, href, value).
///
/// The returned `elements[].ref` values (e.g. "e3") can be passed as `selector`
/// to /click and /type without needing to identify CSS selectors manually.
async fn snapshot_v2(
    State(state): State<V2AppState>,
    Json(request): Json<SnapshotRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let manager = get_session_manager_v2();

    let handle = match manager.get_handle(&request.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", request.session)),
    };

    // Build the same snapshot JS used by the wb CLI
    let scope_selector_json = serde_json::to_string(
        request.within.as_deref().unwrap_or("body")
    ).unwrap_or_else(|_| "\"body\"".to_string());

    let element_selectors = if request.all {
        r#"'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="radio"],[onclick],[tabindex]:not([tabindex="-1"]),h1,h2,h3,h4,h5,h6,p,li,img,table,th,td,label,span[class],div[class]'"#
    } else {
        r#"'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="radio"],[onclick],[tabindex]:not([tabindex="-1"])'"#
    };

    let limit = request.limit;
    let script = format!(
        r#"(function(){{
  document.querySelectorAll('[data-wb-ref]').forEach(el => el.removeAttribute('data-wb-ref'));
  const scope = document.querySelector({scope_selector_json}) || document.body;
  const sels = {element_selectors};
  const els = [...scope.querySelectorAll(sels)].filter(el => {{
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) return false;
    const s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden';
  }}).slice(0, {limit});
  let id = 1;
  const results = els.map(el => {{
    const ref = 'e' + id++;
    el.setAttribute('data-wb-ref', ref);
    return {{
      ref, tag: el.tagName.toLowerCase(),
      type: el.type || null,
      role: el.getAttribute('role'),
      text: (el.textContent||'').trim().slice(0,80),
      name: el.getAttribute('name') || el.getAttribute('aria-label'),
      href: el.href || null,
      value: el.type === 'password' ? null : (el.value || null),
      placeholder: el.placeholder || null,
      checked: el.checked === true ? true : null,
      disabled: el.disabled === true ? true : null
    }};
  }});
  return JSON.stringify({{ title: document.title, url: location.href, elements: results }});
}})()"#
    );

    let (tx, rx) = oneshot::channel();
    let cmd = if let Some(ref frame) = request.frame {
        AppCommand::ExecuteInFrame {
            id: handle.id.clone(),
            script,
            frame: frame.clone(),
            resp_tx: tx,
        }
    } else {
        AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        }
    };

    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }

    match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
        Ok(Ok(Ok(result))) => {
            // Result is a JSON string from JS — parse it
            let snap: serde_json::Value = serde_json::from_str(&result)
                .unwrap_or(serde_json::Value::String(result));
            let elem_count = snap.get("elements")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            (StatusCode::OK, Json(json!({
                "success": true,
                "session": request.session,
                "snapshot": snap,
                "element_count": elem_count,
                "elapsed_ms": start.elapsed().as_millis() as u64
            }))).into_response()
        }
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost."),
        Err(_) => error_response(Wbp2Error::InternalError, "Snapshot timed out."),
    }
}

/// Query params for GET /frames
#[derive(serde::Deserialize)]
struct FramesQuery {
    session: String,
}

/// GET /frames - List all frames (iframes) in the page via CDP
async fn frames_list(
    State(state): State<V2AppState>,
    axum::extract::Query(query): axum::extract::Query<FramesQuery>,
) -> impl IntoResponse {
    let manager = get_session_manager_v2();

    let handle = match manager.get_handle(&query.session) {
        Some(h) => h,
        None => return error_response(Wbp2Error::SessionNotFound, &format!("Session '{}' not found", query.session)),
    };

    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::GetFrames {
        id: handle.id.clone(),
        resp_tx: tx,
    };

    if state.cmd_tx.send(cmd).is_err() {
        return error_response(Wbp2Error::InternalError, "Failed to send command");
    }

    match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
        Ok(Ok(Ok(result))) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result));
            (StatusCode::OK, Json(parsed)).into_response()
        }
        Ok(Ok(Err(e))) => error_response(Wbp2Error::InternalError, &e),
        Ok(Err(_)) => error_response(Wbp2Error::InternalError, "Session communication lost. The session may have crashed. Try: POST /session/acquire to re-acquire, or 'wb session acquire <name>'."),
        Err(_) => error_response(Wbp2Error::InternalError, "Frame enumeration timed out. The page may still be loading. Try: wait for page load first."),
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
        frame: request.frame.clone(),
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
                    "message": "Wait condition not met within timeout. The element may not exist on this page. Try: check the selector, increase timeout_ms, or take a screenshot to verify page state."
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
                        "message": format!("Unknown device preset '{}'. Available: iPhone 15, Pixel 8, iPad, desktop, etc. See GET /screenshot/devices for full list.", device_name)
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

    // Prepare file save path (auto-save all screenshots)
    let screenshot_dir = crate::core::config::AppConfig::profile_screenshots_dir(&request.session);
    let _ = std::fs::create_dir_all(&screenshot_dir);
    let fmt_ext = format!("{:?}", request.format).to_lowercase();
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let save_filename = format!("cap_{}.{}", timestamp, fmt_ext);
    let save_path = screenshot_dir.join(&save_filename);

    // If frame is specified, use CDP screenshot with clip
    if request.frame.is_some() {
        let (tx, rx) = oneshot::channel();
        let cmd = AppCommand::ScreenshotCdp {
            id: session_id,
            full_page: false,
            format: format!("{:?}", request.format).to_lowercase(),
            quality: match request.format { crate::core::screenshot_v2::ImageFormat::Png => None, _ => Some(request.quality as u32) },
            frame: request.frame.clone(),
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
        
        return match tokio::time::timeout(
            std::time::Duration::from_millis(request.timeout_ms),
            rx
        ).await {
            Ok(Ok(Ok(bytes))) => {
                use base64::Engine;
                let base64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                // Auto-save to file
                let saved = std::fs::write(&save_path, &bytes).is_ok();
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "session": request.session,
                        "mode": "frame",
                        "frame": request.frame,
                        "format": format!("{:?}", request.format).to_lowercase(),
                        "data": base64_data,
                        "saved": saved,
                        "filename": save_filename,
                    })),
                )
            }
            Ok(Ok(Err(e))) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": {
                        "code": "WBP2_099",
                        "name": "SCREENSHOT_ERROR",
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
                        "message": "Screenshot channel closed"
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
                        "message": "Screenshot timed out. The page may be very large or still rendering. Try: wait for page load, or capture a smaller viewport."
                    }
                })),
            ),
        };
    }
    
    // Use Screenshot command (main frame)
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
        Ok(Ok(Ok(base64_data))) => {
            // Auto-save to file (decode base64 → write)
            use base64::Engine;
            let saved = base64::engine::general_purpose::STANDARD.decode(&base64_data)
                .ok()
                .and_then(|bytes| std::fs::write(&save_path, &bytes).ok())
                .is_some();
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "session": request.session,
                    "mode": mode_info,
                    "format": format!("{:?}", request.format).to_lowercase(),
                    "quality": request.quality,
                    "device": device_info,
                    "image": base64_data,
                    "saved": saved,
                    "filename": save_filename,
                })),
            )
        },
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
                    "message": "Screenshot timed out. The page may be very large or still rendering. Try: wait for page load, or capture a smaller viewport."
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
                            "message": "Navigation timed out. The page may be slow or unreachable. Try: check URL, increase timeout_ms, or use wait_for='load' instead of 'stable'."
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
                            "message": "Extraction timed out. The selector may not match any elements. Try: take a snapshot to verify page content, check the CSS selector, or increase timeout."
                        }
                    })),
                ),
            }
        }
        
        GoalType::Wait => {
            // Use WaitForSelector command
            let (tx, rx) = oneshot::channel();
            let frame = request.params.get("frame").and_then(|v| v.as_str()).map(|s| s.to_string());
            let cmd = AppCommand::WaitForSelector {
                id: session_id.clone(),
                selector: request.target.clone(),
                timeout_ms: request.timeout_ms,
                frame,
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
                            "message": "Wait condition not met within timeout. The element may not exist on this page. Try: check the selector, increase timeout_ms, or take a screenshot to verify page state."
                        }
                    })),
                ),
            }
        }
        
        GoalType::Screenshot => {
            let frame = request.params.get("frame").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Prepare file save path (auto-save)
            let goal_screenshot_dir = crate::core::config::AppConfig::profile_screenshots_dir(&request.session);
            let _ = std::fs::create_dir_all(&goal_screenshot_dir);
            let goal_timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let goal_save_filename = format!("cap_{}.png", goal_timestamp);
            let goal_save_path = goal_screenshot_dir.join(&goal_save_filename);

            // If frame is specified, use CDP screenshot
            if frame.is_some() {
                let (tx, rx) = oneshot::channel();
                let cmd = AppCommand::ScreenshotCdp {
                    id: session_id.clone(),
                    full_page: false,
                    format: "png".to_string(),
                    quality: None,
                    frame,
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
                
                return match tokio::time::timeout(
                    std::time::Duration::from_millis(request.timeout_ms),
                    rx
                ).await {
                    Ok(Ok(Ok(bytes))) => {
                        use base64::Engine;
                        let base64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        let _ = std::fs::write(&goal_save_path, &bytes);
                        (
                            StatusCode::OK,
                            Json(json!({
                                "success": true,
                                "goal_type": goal_type_str,
                                "session": request.session,
                                "image": base64_data,
                                "filename": goal_save_filename,
                            })),
                        )
                    }
                    Ok(Ok(Err(e))) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": {
                                "code": "WBP2_099",
                                "name": "SCREENSHOT_ERROR",
                                "message": e
                            }
                        })),
                    ),
                    Ok(Err(_)) | Err(_) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": {
                                "code": "WBP2_099",
                                "name": "TIMEOUT",
                                "message": "Screenshot timed out. The page may be very large or still rendering. Try: wait for page load, or capture a smaller viewport."
                            }
                        })),
                    ),
                };
            }
            
            // Use Screenshot command (main frame)
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
                Ok(Ok(Ok(base64_data))) => {
                    // Auto-save to file
                    use base64::Engine;
                    let _ = base64::engine::general_purpose::STANDARD.decode(&base64_data)
                        .ok()
                        .and_then(|bytes| std::fs::write(&goal_save_path, &bytes).ok());
                    (
                        StatusCode::OK,
                        Json(json!({
                            "success": true,
                            "goal_type": goal_type_str,
                            "session": request.session,
                            "image": base64_data,
                            "filename": goal_save_filename,
                        })),
                    )
                },
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
                            "message": "Screenshot timed out. The page may be very large or still rendering. Try: wait for page load, or capture a smaller viewport."
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
                            "message": "JavaScript execution timed out. The script may have an infinite loop or be waiting for a resource. Try: simplify the script or increase timeout_ms."
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

    let mut files: Vec<(u64, serde_json::Value)> = Vec::new();
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
                                        let modified_ms = file_meta.modified().ok()
                                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                            .map(|d| d.as_millis() as u64)
                                            .unwrap_or(0);
                                        files.push((modified_ms, serde_json::json!({
                                            "filename": name,
                                            "session": session_name,
                                            "size_bytes": file_meta.len(),
                                            "modified_ms": modified_ms,
                                            "url": format!("/media/screenshots/{}/{}", session_name, name),
                                            "uri": format!("browser://screenshots/{}/{}", session_name, name)
                                        })));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by modification time, newest first
    files.sort_by(|a, b| b.0.cmp(&a.0));
    let sorted: Vec<serde_json::Value> = files.into_iter().map(|(_, v)| v).collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "screenshots": sorted,
            "count": sorted.len()
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

// ============================================================================
// Upload API
// ============================================================================

/// POST /upload - Multipart file upload
async fn upload_file(
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    use crate::core::upload;

    let mut session = String::new();
    let mut filename_override: Option<String> = None;
    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "session" => { session = field.text().await.unwrap_or_default(); }
            "filename" => { filename_override = Some(field.text().await.unwrap_or_default()); }
            "file" => {
                let fname = field.file_name().unwrap_or("upload").to_string();
                match field.bytes().await {
                    Ok(bytes) => { file_data = Some((fname, bytes.to_vec())); }
                    Err(e) => {
                        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                            "success": false, "error": format!("Failed to read file: {e}")
                        })));
                    }
                }
            }
            _ => {}
        }
    }

    // Validate
    if session.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": "Missing 'session' field"
        })));
    }
    if let Err(e) = upload::validate_session_name(&session) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
    }

    let (original_name, data) = match file_data {
        Some(d) => d,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "success": false, "error": "Missing 'file' field"
            })));
        }
    };

    if data.len() as u64 > upload::MAX_UPLOAD_SIZE {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": format!("File too large: {} bytes (max {})", data.len(), upload::MAX_UPLOAD_SIZE)
        })));
    }

    let filename = filename_override.as_deref().unwrap_or(&original_name);
    save_upload_and_respond(&session, filename, &data)
}

/// POST /upload/base64 - Base64 JSON upload (for CLI/MCP)
async fn upload_base64(
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::core::upload;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let session = request.get("session").and_then(|v| v.as_str()).unwrap_or("");
    let filename = request.get("filename").and_then(|v| v.as_str()).unwrap_or("upload");
    let data_b64 = request.get("data").and_then(|v| v.as_str()).unwrap_or("");

    if session.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": "Missing 'session' field"
        })));
    }
    if let Err(e) = upload::validate_session_name(session) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
    }
    if data_b64.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": "Missing 'data' field (base64-encoded)"
        })));
    }

    let data = match STANDARD.decode(data_b64.trim()) {
        Ok(d) => d,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "success": false, "error": format!("Base64 decode error: {e}")
            })));
        }
    };

    if data.len() as u64 > upload::MAX_UPLOAD_SIZE {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": format!("File too large: {} bytes (max {})", data.len(), upload::MAX_UPLOAD_SIZE)
        })));
    }

    save_upload_and_respond(session, filename, &data)
}

/// Common upload save logic
fn save_upload_and_respond(session: &str, filename: &str, data: &[u8]) -> (StatusCode, Json<serde_json::Value>) {
    use crate::core::upload;

    let dir = upload::uploads_dir(session);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false, "error": format!("Failed to create upload dir: {e}")
        })));
    }

    let stored_filename = upload::sanitize_and_store_filename(filename);
    let file_path = dir.join(&stored_filename);
    let mime_type = upload::detect_mime_type(filename);
    let upload_id = stored_filename.split('_').next().unwrap_or("unknown").to_string();

    if let Err(e) = std::fs::write(&file_path, data) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false, "error": format!("File write error: {e}")
        })));
    }

    let url = format!("/uploads/{}/{}", session, stored_filename);

    tracing::info!("[upload] Saved {} ({} bytes) → {}", stored_filename, data.len(), file_path.display());

    (StatusCode::OK, Json(serde_json::json!({
        "success": true,
        "upload_id": upload_id,
        "filename": filename,
        "stored_filename": stored_filename,
        "size": data.len(),
        "mime_type": mime_type,
        "session": session,
        "url": url,
        "file_path": file_path.to_string_lossy(),
    })))
}

/// GET /uploads/:session - List uploaded files for a session
async fn upload_list(
    Path(session): Path<String>,
) -> impl IntoResponse {
    use crate::core::upload;

    if let Err(e) = upload::validate_session_name(&session) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
    }

    let dir = upload::uploads_dir(&session);
    let mut files = Vec::new();

    if dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        let uploaded_at = meta.modified().ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_default())
                            .unwrap_or_default();

                        files.push(serde_json::json!({
                            "filename": fname,
                            "size": meta.len(),
                            "mime_type": upload::detect_mime_type(&fname),
                            "url": format!("/uploads/{}/{}", session, fname),
                            "uploaded_at": uploaded_at,
                        }));
                    }
                }
            }
        }
    }

    // Sort by uploaded_at descending
    files.sort_by(|a, b| {
        let at_a = a["uploaded_at"].as_str().unwrap_or("");
        let at_b = b["uploaded_at"].as_str().unwrap_or("");
        at_b.cmp(at_a)
    });

    (StatusCode::OK, Json(serde_json::json!({
        "success": true,
        "session": session,
        "files": files,
        "count": files.len(),
    })))
}

/// GET /uploads/:session/:filename - Serve an uploaded file
async fn upload_serve(
    Path((session, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    use axum::body::Body;
    use axum::http::header;
    use crate::core::upload;

    // Security: prevent path traversal
    if let Err(_) = upload::validate_session_name(&session) {
        return (StatusCode::BAD_REQUEST, [(header::CONTENT_TYPE, "application/json")],
            Body::from(r#"{"error": "Invalid session"}"#)).into_response();
    }
    if let Err(_) = upload::validate_filename(&filename) {
        return (StatusCode::BAD_REQUEST, [(header::CONTENT_TYPE, "application/json")],
            Body::from(r#"{"error": "Invalid filename"}"#)).into_response();
    }

    let filepath = upload::uploads_dir(&session).join(&filename);

    match std::fs::read(&filepath) {
        Ok(data) => {
            let content_type = upload::detect_mime_type(&filename);
            (StatusCode::OK, [(header::CONTENT_TYPE, content_type.as_str())],
                Body::from(data)).into_response()
        }
        Err(e) => {
            (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "application/json")],
                Body::from(format!(r#"{{"error": "File not found: {e}"}}"#))).into_response()
        }
    }
}

/// DELETE /uploads/:session/:filename - Delete an uploaded file
async fn upload_delete(
    Path((session, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    use crate::core::upload;

    if let Err(e) = upload::validate_session_name(&session) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
    }
    if let Err(e) = upload::validate_filename(&filename) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
    }

    let filepath = upload::uploads_dir(&session).join(&filename);

    if !filepath.exists() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false, "error": "File not found"
        })));
    }

    match std::fs::remove_file(&filepath) {
        Ok(()) => {
            tracing::info!("[upload] Deleted: {}", filepath.display());
            (StatusCode::OK, Json(serde_json::json!({
                "success": true, "deleted": filename
            })))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "success": false, "error": format!("Delete failed: {e}")
            })))
        }
    }
}

// ============================================================================
// Form Injection API
// ============================================================================

/// POST /form/inject-file - Inject uploaded file into a WebView file input
async fn form_inject_file(
    State(state): State<V2AppState>,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::core::upload;

    let session = request.get("session").and_then(|v| v.as_str()).unwrap_or("default");
    let selector = request.get("selector").and_then(|v| v.as_str()).unwrap_or("input[type=file]");
    let file_ref = request.get("file").and_then(|v| v.as_str()).unwrap_or("");
    let frame = request.get("frame").and_then(|v| v.as_str());

    if file_ref.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false, "error": "Missing 'file' field (upload URL or absolute path)"
        })));
    }

    // Resolve file path: if starts with /uploads/, resolve to filesystem path
    let file_path = if file_ref.starts_with("/uploads/") {
        let parts: Vec<&str> = file_ref.trim_start_matches("/uploads/").splitn(2, '/').collect();
        if parts.len() != 2 {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "success": false, "error": "Invalid upload URL format"
            })));
        }
        let upload_session = parts[0];
        let upload_filename = parts[1];
        if let Err(e) = upload::validate_session_name(upload_session) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
        }
        if let Err(e) = upload::validate_filename(upload_filename) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e })));
        }
        let path = upload::uploads_dir(upload_session).join(upload_filename);
        if !path.exists() {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "success": false, "error": "Uploaded file not found"
            })));
        }
        path.to_string_lossy().to_string()
    } else {
        // Assume absolute filesystem path
        let path = std::path::PathBuf::from(file_ref);
        if !path.exists() {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "success": false, "error": "File not found at specified path"
            })));
        }
        file_ref.to_string()
    };

    // Use Windows-style path for CDP (backslashes)
    let win_path = file_path.replace('/', "\\");

    // Send command via AppCommand → session thread → CDP
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let cmd = crate::core::AppCommand::FormInjectFile {
        id: session.to_string(),
        selector: selector.to_string(),
        file_paths: vec![win_path.clone()],
        frame: frame.map(|f| f.to_string()),
        resp_tx,
    };

    if state.cmd_tx.send(cmd).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false, "error": "Failed to send command to session"
        })));
    }

    match resp_rx.await {
        Ok(Ok(result)) => {
            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "selector": selector,
                "file": file_path,
                "method": "cdp_dom_set_file_input_files",
                "result": result,
            })))
        }
        Ok(Err(e)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "success": false, "error": e,
            })))
        }
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "success": false, "error": "Session command channel closed",
            })))
        }
    }
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
