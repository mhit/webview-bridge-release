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
use serde::Deserialize;
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

// ============================================================================
// Wait v2 Endpoints
// ============================================================================

use crate::core::wait_v2::{WaitRequest, WaitResponse, generate_wait_script};

/// POST /v2/wait - Smart wait with multiple condition types
async fn wait_v2(
    State(_state): State<V2AppState>,
    Json(request): Json<WaitRequest>,
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
    
    // Generate the wait script
    let _script = generate_wait_script(&request);
    
    // Execute via session (this is simplified - full impl would use oneshot channel)
    // For now, return a placeholder indicating the script was generated
    // In production, this would execute the script and wait for the Promise to resolve
    
    // TODO: Execute script through session handle and wait for result
    // For now, return success with the generated script info
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Wait request queued",
            "session": request.session,
            "selector": request.selector,
            "condition": format!("{:?}", request.condition),
            "timeout_ms": request.timeout_ms,
            "_note": "Full async execution pending - script generated"
        })),
    )
}

// ============================================================================
// Screenshot v2 Endpoints
// ============================================================================

use crate::core::screenshot_v2::{
    ScreenshotRequest, CaptureMode, get_device_presets, find_device_preset,
    generate_wait_for_images_script, generate_element_screenshot_script,
    generate_full_page_dimensions_script, generate_hide_elements_script,
};

/// POST /v2/screenshot - Advanced screenshot with modes and device emulation
async fn screenshot_v2(
    State(_state): State<V2AppState>,
    Json(request): Json<ScreenshotRequest>,
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
    
    // Generate appropriate scripts based on mode
    let scripts: Vec<String> = {
        let mut s = Vec::new();
        
        // Wait for images if requested
        if request.wait_for_images {
            s.push(generate_wait_for_images_script(request.timeout_ms));
        }
        
        // Hide elements if requested
        if let Some(ref selectors) = request.hide_selectors {
            s.push(generate_hide_elements_script(selectors));
        }
        
        // Mode-specific scripts
        match request.mode {
            CaptureMode::Element => {
                if let Some(ref selector) = request.selector {
                    s.push(generate_element_screenshot_script(selector, request.padding.unwrap_or(0)));
                }
            }
            CaptureMode::FullPage => {
                s.push(generate_full_page_dimensions_script());
            }
            CaptureMode::Viewport => {
                // No additional scripts needed
            }
        }
        
        s
    };
    
    // TODO: Execute scripts through session handle and capture screenshot
    // For now, return success with the request info
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Screenshot request queued",
            "session": request.session,
            "mode": mode_info,
            "format": format!("{:?}", request.format).to_lowercase(),
            "quality": request.quality,
            "device": device_info,
            "scripts_count": scripts.len(),
            "_note": "Full implementation pending - scripts generated"
        })),
    )
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
    State(_state): State<V2AppState>,
    Json(request): Json<GoalRequest>,
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
    
    // TODO: Execute script through session handle with retry logic
    // For now, return success with the request info
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Goal execution queued",
            "session": request.session,
            "goal_type": goal_type_str,
            "target": request.target,
            "retry_config": {
                "max_retries": request.retry.max_retries,
                "initial_delay_ms": request.retry.initial_delay_ms
            },
            "script_length": script.len(),
            "_note": "Full execution pending - script generated"
        })),
    )
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
