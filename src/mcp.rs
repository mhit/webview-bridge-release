// MCP (Model Context Protocol) Server Implementation
// Reference: https://modelcontextprotocol.io/

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::{AppCommand, SessionOptions};

// ============================================================================
// MCP Types
// ============================================================================

#[derive(Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
}

#[derive(Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Deserialize)]
pub struct McpToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize)]
pub struct McpToolCallResponse {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

#[derive(Serialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
    pub data: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Deserialize)]
pub struct McpResourceRequest {
    pub uri: String,
}

#[derive(Serialize)]
pub struct McpResourceResponse {
    pub contents: Vec<McpResourceContent>,
}

#[derive(Serialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub text: Option<String>,
    pub blob: Option<String>,
}

// ============================================================================
// MCP State
// ============================================================================

pub struct McpState {
    pub tx: mpsc::Sender<AppCommand>,
    pub current_session: tokio::sync::RwLock<Option<String>>,
}

// ============================================================================
// MCP Router
// ============================================================================

pub fn mcp_router(tx: mpsc::Sender<AppCommand>) -> Router {
    let state = Arc::new(McpState {
        tx,
        current_session: tokio::sync::RwLock::new(None),
    });

    Router::new()
        .route("/mcp", get(get_server_info))
        .route("/mcp/info", get(get_server_info))
        .route("/mcp/tools", get(list_tools))
        .route("/mcp/tools/call", post(call_tool))
        .route("/mcp/resources", get(list_resources))
        .route("/mcp/resources/read", post(read_resource))
        .with_state(state)
}

// ============================================================================
// Handler Implementations
// ============================================================================

async fn get_server_info() -> Json<McpServerInfo> {
    Json(McpServerInfo {
        name: "webview-bridge".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: "2024-11-05".to_string(),
    })
}

async fn list_tools() -> Json<Vec<McpTool>> {
    Json(vec![
        McpTool {
            name: "browse".to_string(),
            description: "Navigate to a URL and load the page".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to navigate to" }
                },
                "required": ["url"]
            }),
        },
        McpTool {
            name: "screenshot".to_string(),
            description: "Take a screenshot of the current page".to_string(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        McpTool {
            name: "extract".to_string(),
            description: "Extract text content from elements matching a selector".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector for elements to extract" }
                },
                "required": ["selector"]
            }),
        },
        McpTool {
            name: "wait".to_string(),
            description: "Wait for an element to appear on the page".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector to wait for" },
                    "timeout": { "type": "integer", "description": "Timeout in milliseconds (default: 10000)" }
                },
                "required": ["selector"]
            }),
        },
        McpTool {
            name: "execute".to_string(),
            description: "Execute JavaScript code in the page context".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "script": { "type": "string", "description": "JavaScript code to execute" }
                },
                "required": ["script"]
            }),
        },
        McpTool {
            name: "get_html".to_string(),
            description: "Get the HTML source of the current page".to_string(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        McpTool {
            name: "get_text".to_string(),
            description: "Get the text content of the current page".to_string(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
    ])
}

async fn call_tool(
    State(state): State<Arc<McpState>>,
    Json(request): Json<McpToolCallRequest>,
) -> Json<McpToolCallResponse> {
    let session_id = match ensure_session(&state).await {
        Ok(id) => id,
        Err(e) => {
            return Json(McpToolCallResponse {
                content: vec![McpContent {
                    content_type: "text".to_string(),
                    text: Some(format!("Session error: {}", e)),
                    data: None,
                    mime_type: None,
                }],
                is_error: true,
            });
        }
    };

    let result = execute_tool(&state.tx, &session_id, &request.name, &request.arguments).await;
    match result {
        Ok(content) => Json(McpToolCallResponse {
            content,
            is_error: false,
        }),
        Err(e) => Json(McpToolCallResponse {
            content: vec![McpContent {
                content_type: "text".to_string(),
                text: Some(format!("Error: {}", e)),
                data: None,
                mime_type: None,
            }],
            is_error: true,
        }),
    }
}

async fn ensure_session(state: &McpState) -> Result<String, String> {
    let session = state.current_session.read().await;
    if let Some(id) = session.as_ref() {
        return Ok(id.clone());
    }
    drop(session);
    
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    state.tx.send(AppCommand::CreateSession {
        options: SessionOptions {
            profile: "mcp".to_string(),
            headless: false,
            user_agent: None,
        },
        resp_tx,
    })
    .await
    .map_err(|e| e.to_string())?;

    match resp_rx.await {
        Ok(Ok(id)) => {
            let mut session = state.current_session.write().await;
            *session = Some(id.clone());
            Ok(id)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Channel closed".to_string()),
    }
}

async fn execute_tool(
    tx: &mpsc::Sender<AppCommand>,
    session_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<Vec<McpContent>, String> {
    match tool_name {
        "browse" => {
            let url = args["url"].as_str().ok_or("Missing url")?;
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::Navigate {
                id: session_id.to_string(),
                url: url.to_string(),
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(format!("Navigated to {}", url)),
                data: None,
                mime_type: None,
            }])
        }
        
        "screenshot" => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::Screenshot {
                id: session_id.to_string(),
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let base64_image = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "image".to_string(),
                text: None,
                data: Some(base64_image),
                mime_type: Some("image/png".to_string()),
            }])
        }
        
        "extract" => {
            let selector = args["selector"].as_str().ok_or("Missing selector")?;
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::Extract {
                id: session_id.to_string(),
                selector: selector.to_string(),
                attribute: "text".to_string(),
                extract_all: true,
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let result = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(result),
                data: None,
                mime_type: None,
            }])
        }
        
        "wait" => {
            let selector = args["selector"].as_str().ok_or("Missing selector")?;
            let timeout = args["timeout"].as_u64().unwrap_or(10000);
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::WaitForSelector {
                id: session_id.to_string(),
                selector: selector.to_string(),
                timeout_ms: timeout,
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let found = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(if found { format!("Element '{}' found", selector) } else { format!("Element '{}' not found", selector) }),
                data: None,
                mime_type: None,
            }])
        }
        
        "execute" => {
            let script = args["script"].as_str().ok_or("Missing script")?;
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let request_id = uuid::Uuid::new_v4().to_string();
            tx.send(AppCommand::ExecuteScript {
                id: session_id.to_string(),
                script: script.to_string(),
                request_id,
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let result = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(result.to_string()),
                data: None,
                mime_type: None,
            }])
        }
        
        "get_html" => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::Snapshot {
                id: session_id.to_string(),
                format: "html".to_string(),
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let html = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(html),
                data: None,
                mime_type: Some("text/html".to_string()),
            }])
        }
        
        "get_text" => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            tx.send(AppCommand::Snapshot {
                id: session_id.to_string(),
                format: "text".to_string(),
                resp_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
            
            let text = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
            
            Ok(vec![McpContent {
                content_type: "text".to_string(),
                text: Some(text),
                data: None,
                mime_type: Some("text/plain".to_string()),
            }])
        }
        
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}

async fn list_resources() -> Json<Vec<McpResource>> {
    Json(vec![
        McpResource {
            uri: "browser://current/html".to_string(),
            name: "Current Page HTML".to_string(),
            description: "The HTML source of the current browser page".to_string(),
            mime_type: "text/html".to_string(),
        },
        McpResource {
            uri: "browser://current/text".to_string(),
            name: "Current Page Text".to_string(),
            description: "The text content of the current browser page".to_string(),
            mime_type: "text/plain".to_string(),
        },
        McpResource {
            uri: "browser://current/screenshot".to_string(),
            name: "Current Page Screenshot".to_string(),
            description: "A screenshot of the current browser page".to_string(),
            mime_type: "image/png".to_string(),
        },
    ])
}

async fn read_resource(
    State(state): State<Arc<McpState>>,
    Json(request): Json<McpResourceRequest>,
) -> Json<McpResourceResponse> {
    let session_result = ensure_session(&state).await;
    
    match session_result {
        Ok(session_id) => {
            let content = match request.uri.as_str() {
                "browser://current/html" => get_snapshot(&state.tx, &session_id, "html").await,
                "browser://current/text" => get_snapshot(&state.tx, &session_id, "text").await,
                "browser://current/screenshot" => get_screenshot(&state.tx, &session_id).await,
                _ => Err(format!("Unknown resource: {}", request.uri)),
            };
            
            match content {
                Ok((text, blob, mime_type)) => Json(McpResourceResponse {
                    contents: vec![McpResourceContent {
                        uri: request.uri,
                        mime_type,
                        text,
                        blob,
                    }],
                }),
                Err(e) => Json(McpResourceResponse {
                    contents: vec![McpResourceContent {
                        uri: request.uri,
                        mime_type: "text/plain".to_string(),
                        text: Some(format!("Error: {}", e)),
                        blob: None,
                    }],
                }),
            }
        }
        Err(e) => Json(McpResourceResponse {
            contents: vec![McpResourceContent {
                uri: request.uri,
                mime_type: "text/plain".to_string(),
                text: Some(format!("Session error: {}", e)),
                blob: None,
            }],
        }),
    }
}

async fn get_snapshot(
    tx: &mpsc::Sender<AppCommand>,
    session_id: &str,
    format: &str,
) -> Result<(Option<String>, Option<String>, String), String> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    tx.send(AppCommand::Snapshot {
        id: session_id.to_string(),
        format: format.to_string(),
        resp_tx,
    })
    .await
    .map_err(|e| e.to_string())?;
    
    let result = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
    
    let mime_type = if format == "html" { "text/html" } else { "text/plain" };
    Ok((Some(result), None, mime_type.to_string()))
}

async fn get_screenshot(
    tx: &mpsc::Sender<AppCommand>,
    session_id: &str,
) -> Result<(Option<String>, Option<String>, String), String> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    tx.send(AppCommand::Screenshot {
        id: session_id.to_string(),
        resp_tx,
    })
    .await
    .map_err(|e| e.to_string())?;
    
    let result = resp_rx.await.map_err(|_| "Channel closed")?.map_err(|e| e)?;
    Ok((None, Some(result), "image/png".to_string()))
}
