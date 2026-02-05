// CDP (Chrome DevTools Protocol) Compatibility Layer
// Reference: https://chromedevtools.github.io/devtools-protocol/

use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::{AppCommand, SessionOptions};

// ============================================================================
// CDP Response Types
// ============================================================================

#[derive(Serialize)]
pub struct CdpBrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
    #[serde(rename = "V8-Version")]
    pub v8_version: String,
    #[serde(rename = "WebKit-Version")]
    pub webkit_version: String,
}

#[derive(Serialize)]
pub struct CdpTarget {
    pub description: String,
    #[serde(rename = "devtoolsFrontendUrl")]
    pub devtools_frontend_url: String,
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: String,
}

#[derive(Deserialize)]
pub struct CdpCommandRequest {
    pub id: u32,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct CdpCommandResponse {
    pub id: u32,
    pub result: serde_json::Value,
}

#[derive(Serialize)]
pub struct CdpErrorResponse {
    pub id: u32,
    pub error: CdpError,
}

#[derive(Serialize)]
pub struct CdpError {
    pub code: i32,
    pub message: String,
}

// ============================================================================
// CDP State
// ============================================================================

pub struct CdpState {
    pub tx: mpsc::Sender<AppCommand>,
    pub sessions: tokio::sync::RwLock<HashMap<String, CdpSessionInfo>>,
}

pub struct CdpSessionInfo {
    pub id: String,
    pub bridge_session_id: String,
    pub url: String,
    pub title: String,
}

// ============================================================================
// CDP Router
// ============================================================================

pub fn cdp_router(tx: mpsc::Sender<AppCommand>) -> Router {
    let state = Arc::new(CdpState {
        tx,
        sessions: tokio::sync::RwLock::new(HashMap::new()),
    });

    Router::new()
        // Browser-level endpoints (Puppeteer/Playwright discovery)
        .route("/json/version", get(get_version))
        .route("/json", get(list_targets))
        .route("/json/list", get(list_targets))
        .route("/json/new", get(new_target))
        .route("/json/close/:target_id", get(close_target))
        .route("/json/protocol", get(get_protocol))
        // CDP command endpoints (simplified HTTP fallback)
        .route("/cdp/command/:target_id", post(execute_command))
        .with_state(state)
}

// ============================================================================
// Handler Implementations
// ============================================================================

async fn get_version() -> Json<CdpBrowserVersion> {
    Json(CdpBrowserVersion {
        browser: "WebView2/1.0 WebViewBridge/0.1.0".to_string(),
        protocol_version: "1.3".to_string(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0".to_string(),
        v8_version: "12.0.0".to_string(),
        webkit_version: "537.36".to_string(),
    })
}

async fn list_targets(State(state): State<Arc<CdpState>>) -> Json<Vec<CdpTarget>> {
    let sessions = state.sessions.read().await;
    let targets: Vec<CdpTarget> = sessions
        .values()
        .map(|s| CdpTarget {
            description: String::new(),
            devtools_frontend_url: format!("devtools://devtools/bundled/inspector.html?ws=127.0.0.1:9400/cdp/ws/{}", s.id),
            id: s.id.clone(),
            title: s.title.clone(),
            target_type: "page".to_string(),
            url: s.url.clone(),
            websocket_debugger_url: format!("ws://127.0.0.1:9400/cdp/ws/{}", s.id),
        })
        .collect();
    Json(targets)
}

#[derive(Deserialize)]
pub struct NewTargetParams {
    pub url: Option<String>,
}

async fn new_target(
    State(state): State<Arc<CdpState>>,
    Query(params): Query<NewTargetParams>,
) -> Result<Json<CdpTarget>, (StatusCode, String)> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    
    state.tx.send(AppCommand::CreateSession {
        options: SessionOptions {
            profile: "cdp".to_string(),
            headless: false,
            user_agent: None,
        },
        resp_tx,
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let bridge_session_id = resp_rx.await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Channel closed".to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let cdp_id = uuid::Uuid::new_v4().to_string();
    let url = params.url.unwrap_or_else(|| "about:blank".to_string());
    
    // Navigate if URL provided
    if url != "about:blank" {
        let (nav_tx, nav_rx) = tokio::sync::oneshot::channel();
        let _ = state.tx.send(AppCommand::Navigate {
            id: bridge_session_id.clone(),
            url: url.clone(),
            resp_tx: nav_tx,
        }).await;
        let _ = nav_rx.await;
    }
    
    let session_info = CdpSessionInfo {
        id: cdp_id.clone(),
        bridge_session_id,
        url: url.clone(),
        title: "New Tab".to_string(),
    };
    
    state.sessions.write().await.insert(cdp_id.clone(), session_info);
    
    Ok(Json(CdpTarget {
        description: String::new(),
        devtools_frontend_url: format!("devtools://devtools/bundled/inspector.html?ws=127.0.0.1:9400/cdp/ws/{}", cdp_id),
        id: cdp_id.clone(),
        title: "New Tab".to_string(),
        target_type: "page".to_string(),
        url,
        websocket_debugger_url: format!("ws://127.0.0.1:9400/cdp/ws/{}", cdp_id),
    }))
}

async fn close_target(
    State(state): State<Arc<CdpState>>,
    Path(target_id): Path<String>,
) -> Result<&'static str, (StatusCode, String)> {
    let session_info = {
        let sessions = state.sessions.read().await;
        sessions.get(&target_id).map(|s| s.bridge_session_id.clone())
    };
    
    if let Some(bridge_id) = session_info {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = state.tx.send(AppCommand::CloseSession {
            id: bridge_id,
            resp_tx,
        }).await;
        let _ = resp_rx.await;
        
        state.sessions.write().await.remove(&target_id);
        Ok("Target is closing")
    } else {
        Err((StatusCode::NOT_FOUND, "No such target".to_string()))
    }
}

async fn get_protocol() -> Json<serde_json::Value> {
    // Minimal protocol definition for compatibility
    Json(serde_json::json!({
        "version": {
            "major": "1",
            "minor": "3"
        },
        "domains": [
            {
                "domain": "Page",
                "commands": [
                    {"name": "navigate", "parameters": [{"name": "url", "type": "string"}]},
                    {"name": "captureScreenshot", "returns": [{"name": "data", "type": "string"}]}
                ]
            },
            {
                "domain": "Runtime",
                "commands": [
                    {"name": "evaluate", "parameters": [{"name": "expression", "type": "string"}]}
                ]
            },
            {
                "domain": "DOM",
                "commands": [
                    {"name": "getDocument", "returns": [{"name": "root", "type": "object"}]}
                ]
            }
        ]
    }))
}

async fn execute_command(
    State(state): State<Arc<CdpState>>,
    Path(target_id): Path<String>,
    Json(request): Json<CdpCommandRequest>,
) -> Result<Json<CdpCommandResponse>, (StatusCode, Json<CdpErrorResponse>)> {
    let bridge_session_id = {
        let sessions = state.sessions.read().await;
        sessions.get(&target_id)
            .map(|s| s.bridge_session_id.clone())
            .ok_or_else(|| (StatusCode::NOT_FOUND, Json(CdpErrorResponse {
                id: request.id,
                error: CdpError {
                    code: -32000,
                    message: "No such target".to_string(),
                },
            })))?
    };

    let result = match request.method.as_str() {
        // Page domain
        "Page.navigate" => {
            let url = request.params
                .as_ref()
                .and_then(|p| p.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("about:blank");
            
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            state.tx.send(AppCommand::Navigate {
                id: bridge_session_id,
                url: url.to_string(),
                resp_tx,
            }).await.map_err(|_| cdp_error(request.id, -32603, "Internal error"))?;
            
            resp_rx.await
                .map_err(|_| cdp_error(request.id, -32603, "Channel closed"))?
                .map_err(|e| cdp_error(request.id, -32000, &e))?;
            
            serde_json::json!({"frameId": "main", "loaderId": uuid::Uuid::new_v4().to_string()})
        }
        
        "Page.captureScreenshot" => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            state.tx.send(AppCommand::Screenshot {
                id: bridge_session_id,
                resp_tx,
            }).await.map_err(|_| cdp_error(request.id, -32603, "Internal error"))?;
            
            let data = resp_rx.await
                .map_err(|_| cdp_error(request.id, -32603, "Channel closed"))?
                .map_err(|e| cdp_error(request.id, -32000, &e))?;
            
            serde_json::json!({"data": data})
        }
        
        // Runtime domain
        "Runtime.evaluate" => {
            let expression = request.params
                .as_ref()
                .and_then(|p| p.get("expression"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let request_id = uuid::Uuid::new_v4().to_string();
            state.tx.send(AppCommand::ExecuteScript {
                id: bridge_session_id,
                script: expression.to_string(),
                request_id,
                resp_tx,
            }).await.map_err(|_| cdp_error(request.id, -32603, "Internal error"))?;
            
            let result = resp_rx.await
                .map_err(|_| cdp_error(request.id, -32603, "Channel closed"))?
                .map_err(|e| cdp_error(request.id, -32000, &e))?;
            
            serde_json::json!({
                "result": {
                    "type": "string",
                    "value": result
                }
            })
        }
        
        // DOM domain
        "DOM.getDocument" => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            state.tx.send(AppCommand::Snapshot {
                id: bridge_session_id,
                format: "html".to_string(),
                resp_tx,
            }).await.map_err(|_| cdp_error(request.id, -32603, "Internal error"))?;
            
            let _ = resp_rx.await;
            
            serde_json::json!({
                "root": {
                    "nodeId": 1,
                    "backendNodeId": 1,
                    "nodeType": 9,
                    "nodeName": "#document",
                    "localName": "",
                    "nodeValue": ""
                }
            })
        }
        
        // Unknown method
        _ => {
            return Err(cdp_error(request.id, -32601, &format!("Method not found: {}", request.method)));
        }
    };

    Ok(Json(CdpCommandResponse {
        id: request.id,
        result,
    }))
}

fn cdp_error(id: u32, code: i32, message: &str) -> (StatusCode, Json<CdpErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(CdpErrorResponse {
            id,
            error: CdpError {
                code,
                message: message.to_string(),
            },
        }),
    )
}
