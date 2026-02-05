// WebDriver W3C Protocol Compatible API
// Reference: https://www.w3.org/TR/webdriver2/

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::{AppCommand, SessionOptions};

// ============================================================================
// WebDriver Response Types
// ============================================================================

#[derive(Serialize)]
pub struct WebDriverValue<T> {
    pub value: T,
}

#[derive(Serialize)]
pub struct WebDriverError {
    pub error: String,
    pub message: String,
    pub stacktrace: String,
}

#[derive(Serialize)]
pub struct SessionResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub capabilities: SessionCapabilities,
}

#[derive(Serialize, Deserialize, Default)]
pub struct SessionCapabilities {
    #[serde(rename = "browserName")]
    pub browser_name: String,
    #[serde(rename = "browserVersion")]
    pub browser_version: String,
    #[serde(rename = "platformName")]
    pub platform_name: String,
    #[serde(rename = "acceptInsecureCerts")]
    pub accept_insecure_certs: bool,
}

#[derive(Deserialize)]
pub struct NewSessionRequest {
    pub capabilities: Option<CapabilitiesRequest>,
}

#[derive(Deserialize)]
pub struct CapabilitiesRequest {
    #[serde(rename = "alwaysMatch")]
    pub always_match: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct NavigateRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct FindElementRequest {
    pub using: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct ElementResponse {
    #[serde(rename = "element-6066-11e4-a52e-4f735466cecf")]
    pub element_id: String,
}

#[derive(Deserialize)]
pub struct ExecuteScriptRequest {
    pub script: String,
    pub args: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize)]
pub struct CookieData {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "httpOnly")]
    pub http_only: Option<bool>,
}

#[derive(Deserialize)]
pub struct CookieRequest {
    pub cookie: CookieData,
}

#[derive(Deserialize)]
pub struct ElementValueRequest {
    pub text: String,
}

// ============================================================================
// WebDriver Router
// ============================================================================

pub fn webdriver_router(tx: mpsc::Sender<AppCommand>) -> Router {
    let state = Arc::new(tx);

    Router::new()
        .route("/wd/hub/session", post(new_session))
        .route("/wd/hub/session/:session_id", delete(delete_session))
        .route("/wd/hub/status", get(get_status))
        .route("/wd/hub/session/:session_id/url", post(navigate_to))
        .route("/wd/hub/session/:session_id/url", get(get_current_url))
        .route("/wd/hub/session/:session_id/title", get(get_title))
        .route("/wd/hub/session/:session_id/element", post(find_element))
        .route("/wd/hub/session/:session_id/elements", post(find_elements))
        .route("/wd/hub/session/:session_id/execute/sync", post(execute_script))
        .route("/wd/hub/session/:session_id/screenshot", get(take_screenshot))
        .route("/wd/hub/session/:session_id/cookie", get(get_all_cookies))
        .route("/wd/hub/session/:session_id/cookie", post(add_cookie))
        .route("/wd/hub/session/:session_id/source", get(get_page_source))
        .route("/wd/hub/session/:session_id/window", get(get_window_handle))
        .with_state(state)
}

// ============================================================================
// Handler Implementations
// ============================================================================

async fn get_status() -> Json<WebDriverValue<serde_json::Value>> {
    Json(WebDriverValue {
        value: serde_json::json!({
            "ready": true,
            "message": "WebView Bridge WebDriver Ready"
        }),
    })
}

async fn new_session(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Json(_payload): Json<NewSessionRequest>,
) -> Result<Json<WebDriverValue<SessionResponse>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::CreateSession {
        options: SessionOptions {
            profile: "webdriver".to_string(),
            headless: false,
            user_agent: None,
        },
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(session_id)) => Ok(Json(WebDriverValue {
            value: SessionResponse {
                session_id,
                capabilities: SessionCapabilities {
                    browser_name: "webview2".to_string(),
                    browser_version: "1.0".to_string(),
                    platform_name: "windows".to_string(),
                    accept_insecure_certs: true,
                },
            },
        })),
        Ok(Err(e)) => Err(webdriver_error("session not created", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn delete_session(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<()>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::CloseSession {
        id: session_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(())) => Ok(Json(WebDriverValue { value: () })),
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn navigate_to(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
    Json(payload): Json<NavigateRequest>,
) -> Result<Json<WebDriverValue<()>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::Navigate {
        id: session_id,
        url: payload.url,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(())) => Ok(Json(WebDriverValue { value: () })),
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn get_current_url(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<String>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tx.send(AppCommand::ExecuteScript {
        id: session_id,
        script: "return window.location.href".to_string(),
        request_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(result)) => {
            let url = result.as_str().unwrap_or("").to_string();
            Ok(Json(WebDriverValue { value: url }))
        }
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn get_title(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<String>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tx.send(AppCommand::ExecuteScript {
        id: session_id,
        script: "return document.title".to_string(),
        request_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(result)) => {
            let title = result.as_str().unwrap_or("").to_string();
            Ok(Json(WebDriverValue { value: title }))
        }
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn find_element(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
    Json(payload): Json<FindElementRequest>,
) -> Result<Json<WebDriverValue<ElementResponse>>, (StatusCode, Json<WebDriverError>)> {
    let script = format!(
        r#"(function() {{
            var el = document.querySelector('{}');
            if (!el) return null;
            if (!el.__webdriver_id) el.__webdriver_id = Math.random().toString(36).substr(2, 9);
            return el.__webdriver_id;
        }})()"#,
        payload.value.replace("'", "\\'")
    );

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tx.send(AppCommand::ExecuteScript {
        id: session_id,
        script,
        request_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(result)) => {
            if result.is_null() {
                Err(webdriver_error("no such element", "Element not found"))
            } else {
                let id = result.as_str().unwrap_or("").to_string();
                Ok(Json(WebDriverValue {
                    value: ElementResponse { element_id: id },
                }))
            }
        }
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn find_elements(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
    Json(payload): Json<FindElementRequest>,
) -> Result<Json<WebDriverValue<Vec<ElementResponse>>>, (StatusCode, Json<WebDriverError>)> {
    let script = format!(
        r#"(function() {{
            var els = document.querySelectorAll('{}');
            return Array.from(els).map(function(el) {{
                if (!el.__webdriver_id) el.__webdriver_id = Math.random().toString(36).substr(2, 9);
                return el.__webdriver_id;
            }});
        }})()"#,
        payload.value.replace("'", "\\'")
    );

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tx.send(AppCommand::ExecuteScript {
        id: session_id,
        script,
        request_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(result)) => {
            let ids: Vec<String> = serde_json::from_value(result).unwrap_or_default();
            let elements: Vec<ElementResponse> = ids
                .into_iter()
                .map(|id| ElementResponse { element_id: id })
                .collect();
            Ok(Json(WebDriverValue { value: elements }))
        }
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn execute_script(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
    Json(payload): Json<ExecuteScriptRequest>,
) -> Result<Json<WebDriverValue<serde_json::Value>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tx.send(AppCommand::ExecuteScript {
        id: session_id,
        script: payload.script,
        request_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(value)) => Ok(Json(WebDriverValue { value })),
        Ok(Err(e)) => Err(webdriver_error("javascript error", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn take_screenshot(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<String>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::Screenshot {
        id: session_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(base64_image)) => Ok(Json(WebDriverValue { value: base64_image })),
        Ok(Err(e)) => Err(webdriver_error("unable to capture screen", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn get_all_cookies(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<Vec<CookieData>>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::GetCookies {
        id: session_id,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(cookies_json)) => {
            let cookies: Vec<CookieData> = serde_json::from_str(&cookies_json).unwrap_or_default();
            Ok(Json(WebDriverValue { value: cookies }))
        }
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn add_cookie(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
    Json(payload): Json<CookieRequest>,
) -> Result<Json<WebDriverValue<()>>, (StatusCode, Json<WebDriverError>)> {
    let cookie_json = serde_json::to_string(&vec![payload.cookie]).unwrap_or_default();
    
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::SetCookies {
        id: session_id,
        cookies: cookie_json,
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(())) => Ok(Json(WebDriverValue { value: () })),
        Ok(Err(e)) => Err(webdriver_error("unable to set cookie", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn get_page_source(
    State(tx): State<Arc<mpsc::Sender<AppCommand>>>,
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<String>>, (StatusCode, Json<WebDriverError>)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    tx.send(AppCommand::Snapshot {
        id: session_id,
        format: "html".to_string(),
        resp_tx,
    })
    .await
    .map_err(|_| webdriver_error("unknown error", "Failed to send command"))?;

    match resp_rx.await {
        Ok(Ok(html)) => Ok(Json(WebDriverValue { value: html })),
        Ok(Err(e)) => Err(webdriver_error("no such session", &e)),
        Err(_) => Err(webdriver_error("unknown error", "Response channel closed")),
    }
}

async fn get_window_handle(
    Path(session_id): Path<String>,
) -> Result<Json<WebDriverValue<String>>, (StatusCode, Json<WebDriverError>)> {
    Ok(Json(WebDriverValue { value: session_id }))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn webdriver_error(error: &str, message: &str) -> (StatusCode, Json<WebDriverError>) {
    let status = match error {
        "no such session" => StatusCode::NOT_FOUND,
        "no such element" => StatusCode::NOT_FOUND,
        "session not created" => StatusCode::INTERNAL_SERVER_ERROR,
        "javascript error" => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    
    (
        status,
        Json(WebDriverError {
            error: error.to_string(),
            message: message.to_string(),
            stacktrace: String::new(),
        }),
    )
}
