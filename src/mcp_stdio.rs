// MCP Stdio Server - HTTP Proxy Mode
// Connects to the HTTP server and proxies MCP requests
// This allows stdio-based MCP clients to use the WebView Bridge

use std::io::{BufRead, BufReader, Write};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SERVER_URL: &str = "http://127.0.0.1:9400";

#[derive(Deserialize, Debug)]
struct JsonRpcRequest {
    #[serde(default)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// Log to stderr (won't interfere with MCP protocol on stdout)
macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("[webview-mcp] {}", format!($($arg)*));
    };
}

fn get_tools() -> Value {
    json!([
        {
            "name": "browse",
            "description": "Navigate to a URL and wait for the page to load. Returns success status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to navigate to" },
                    "session": { "type": "string", "description": "Session name (default: 'default')" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "screenshot", 
            "description": "Take a screenshot of the current page. By default saves to file and returns filename (use download_file to retrieve). Set save_to_file=false to return base64 directly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session name" },
                    "save_to_file": { "type": "boolean", "description": "Save to file and return filename (default: true). Set false to return base64." },
                    "load_lazy_images": { "type": "boolean", "description": "Force load lazy images before screenshot (default: false)" },
                    "timeout_ms": { "type": "integer", "description": "Timeout for lazy image loading in ms (default: 10000)" }
                }
            }
        },
        {
            "name": "extract",
            "description": "Extract text from elements matching a CSS selector.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector" },
                    "session": { "type": "string", "description": "Session name" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "click",
            "description": "Click on an element.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector" },
                    "session": { "type": "string", "description": "Session name" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "type_text",
            "description": "Type text into an input element.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector" },
                    "text": { "type": "string", "description": "Text to type" },
                    "session": { "type": "string", "description": "Session name" }
                },
                "required": ["selector", "text"]
            }
        },
        {
            "name": "execute",
            "description": "Execute JavaScript and return the result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "script": { "type": "string", "description": "JavaScript code" },
                    "session": { "type": "string", "description": "Session name" }
                },
                "required": ["script"]
            }
        },
        {
            "name": "get_text",
            "description": "Get all text content from the page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session name" }
                }
            }
        },
        {
            "name": "youtube_subtitles",
            "description": "Extract subtitles/captions from a YouTube video using yt-dlp.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "YouTube video URL" },
                    "language": { "type": "string", "description": "Subtitle language code (e.g., 'ja', 'en'). Default: 'ja'" },
                    "format": { "type": "string", "description": "Output format: 'text', 'srt', 'vtt'. Default: 'text'" },
                    "auto_generated": { "type": "boolean", "description": "Include auto-generated subtitles. Default: true" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "youtube_download",
            "description": "Download a YouTube video or audio using yt-dlp.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "YouTube video URL" },
                    "quality": { "type": "string", "description": "Quality: 'best', 'hd', 'sd', 'low'. Default: 'best'" },
                    "audio_only": { "type": "boolean", "description": "Extract audio only as MP3. Default: false" },
                    "output_dir": { "type": "string", "description": "Output directory. Default: 'downloads'" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "collect_images",
            "description": "Collect all images from the current page, optionally downloading them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session name" },
                    "selector": { "type": "string", "description": "CSS selector for container. Default: 'body'" },
                    "min_width": { "type": "integer", "description": "Minimum image width filter" },
                    "min_height": { "type": "integer", "description": "Minimum image height filter" },
                    "download": { "type": "boolean", "description": "Download images to files. Default: false" },
                    "max_images": { "type": "integer", "description": "Maximum images. Default: 100" }
                }
            }
        }
    ])
}

fn get_resources() -> Value {
    json!([
        {
            "uri": "browser://current/text",
            "name": "Page Text",
            "description": "Text content of current page",
            "mimeType": "text/plain"
        },
        {
            "uri": "browser://current/screenshot", 
            "name": "Page Screenshot",
            "description": "Screenshot of current page",
            "mimeType": "image/png"
        },
        {
            "uri": "browser://screenshots/list",
            "name": "Saved Screenshots List",
            "description": "List of saved screenshot files",
            "mimeType": "application/json"
        },
        {
            "uri": "browser://screenshots/{filename}",
            "name": "Saved Screenshot",
            "description": "Get a saved screenshot by filename (e.g., browser://screenshots/default_1738831886.png)",
            "mimeType": "image/png"
        }
    ])
}

pub fn run_mcp_stdio_proxy() {
    log!("Starting MCP stdio proxy...");
    
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);
    
    let mut current_session = "default".to_string();
    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .build();
    
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log!("Read error: {}", e);
                break;
            }
        };
        
        // Accept empty lines silently
        if line.trim().is_empty() {
            continue;
        }
        
        log!("REQUEST: {}", &line);
        
        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                log!("Parse error: {}", e);
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError { code: -32700, message: format!("Parse error: {}", e) }),
                };
                output_response(&mut stdout, &response);
                continue;
            }
        };
        
        let response = handle_request(&request, &client, &mut current_session);
        output_response(&mut stdout, &response);
    }
    
    log!("MCP stdio proxy stopping.");
}

fn output_response(stdout: &mut std::io::Stdout, response: &JsonRpcResponse) {
    let json_str = serde_json::to_string(response).unwrap();
    log!("RESPONSE: {}", &json_str);
    let _ = writeln!(stdout, "{}", json_str);
    let _ = stdout.flush();
}

fn handle_request(
    request: &JsonRpcRequest,
    client: &ureq::Agent,
    current_session: &mut String,
) -> JsonRpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);
    
    log!("Method: {}", request.method);
    
    match request.method.as_str() {
        "initialize" => {
            log!("Handling initialize");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "listChanged": false, "subscribe": false }
                    },
                    "serverInfo": {
                        "name": "webview-bridge",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })),
                error: None,
            }
        }
        
        "initialized" | "notifications/initialized" => {
            log!("Handling initialized notification");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({})),
                error: None,
            }
        }
        
        "tools/list" => {
            log!("Handling tools/list");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "tools": get_tools()
                })),
                error: None,
            }
        }
        
        "tools/call" => {
            let params = request.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));
            
            log!("Calling tool: {} with args: {}", tool_name, arguments);
            
            match execute_tool(tool_name, &arguments, client, current_session) {
                Ok(content) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": content,
                        "isError": false
                    })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{"type": "text", "text": e}],
                        "isError": true
                    })),
                    error: None,
                },
            }
        }
        
        "resources/list" => {
            log!("Handling resources/list");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "resources": get_resources()
                })),
                error: None,
            }
        }
        
        "resources/read" => {
            let uri = request.params
                .as_ref()
                .and_then(|p| p.get("uri"))
                .and_then(|u| u.as_str())
                .unwrap_or("");
            
            log!("Reading resource: {}", uri);
            
            match read_resource(uri, client, current_session) {
                Ok(contents) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({ "contents": contents })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError { code: -32000, message: e }),
                },
            }
        }
        
        "ping" => {
            log!("Handling ping");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({})),
                error: None,
            }
        }
        
        _ => {
            log!("Unknown method: {}", request.method);
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
            }
        }
    }
}

fn execute_tool(
    name: &str,
    args: &Value,
    client: &ureq::Agent,
    current_session: &mut String,
) -> Result<Vec<Value>, String> {
    let session = args.get("session")
        .and_then(|s| s.as_str())
        .unwrap_or(current_session.as_str());
    
    // Ensure session exists
    let _ = client.post(&format!("{}/session/acquire", SERVER_URL))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "name": session,
            "create_if_missing": true,
            "headless": false
        }));
    
    match name {
        "browse" => {
            let url = args.get("url").and_then(|u| u.as_str()).ok_or("Missing 'url'")?;
            let url_str = url.to_string();
            let session_name = session.to_string();
            
            let resp = client.post(&format!("{}/navigate", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": &session_name, "url": &url_str }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            *current_session = session_name.clone();
            
            // Save URL to session state for restore feature
            let _ = client.post(&format!("{}/session/state/url", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "name": &session_name, "url": &url_str }));
            
            Ok(vec![json!({
                "type": "text",
                "text": format!("Navigated to: {} ({}ms)", url_str, body.get("load_time_ms").unwrap_or(&json!(0)))
            })])
        }
        
        "screenshot" => {
            let load_lazy = args.get("load_lazy_images").and_then(|v| v.as_bool()).unwrap_or(false);
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(10000);
            let save_to_file = args.get("save_to_file").and_then(|v| v.as_bool()).unwrap_or(true);
            
            // If load_lazy_images is enabled, force load lazy images first
            if load_lazy {
                let lazy_script = crate::core::screenshot_v2::generate_force_load_lazy_images_script(timeout_ms);
                let _ = client.post(&format!("{}/execute", SERVER_URL))
                    .set("Content-Type", "application/json")
                    .send_json(json!({ "session": session, "script": lazy_script, "timeout_ms": timeout_ms + 5000 }));
            }
            
            let resp = client.post(&format!("{}/screenshot", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let image_base64 = body.get("image").and_then(|i| i.as_str()).unwrap_or("");
            
            if save_to_file {
                // Save to file and return filename only (AI context efficient)
                use std::io::Write;
                use base64::Engine;
                
                // Create screenshots directory in session folder
                let screenshots_dir = crate::core::config::AppConfig::get_session_screenshots_dir(session);
                
                if let Err(e) = std::fs::create_dir_all(&screenshots_dir) {
                    return Err(format!("Failed to create screenshots directory: {}", e));
                }
                
                // Generate filename: {session}_{timestamp}.png
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let filename = format!("{}_{}.png", session, timestamp);
                let filepath = screenshots_dir.join(&filename);
                
                // Decode base64 and save
                let image_data = base64::engine::general_purpose::STANDARD
                    .decode(image_base64)
                    .map_err(|e| format!("Failed to decode base64: {}", e))?;
                
                let mut file = std::fs::File::create(&filepath)
                    .map_err(|e| format!("Failed to create file: {}", e))?;
                file.write_all(&image_data)
                    .map_err(|e| format!("Failed to write file: {}", e))?;
                
                let size_kb = image_data.len() / 1024;
                
                Ok(vec![json!({
                    "type": "text",
                    "text": format!("Screenshot saved: {} ({}KB). Use download_file tool to retrieve.", filename, size_kb)
                })])
            } else {
                // Return base64 directly (legacy mode)
                Ok(vec![json!({
                    "type": "image",
                    "data": image_base64,
                    "mimeType": "image/png"
                })])
            }
        }
        
        "extract" => {
            let selector = args.get("selector").and_then(|s| s.as_str()).ok_or("Missing 'selector'")?;
            
            let script = format!(r#"
                (function() {{
                    var els = document.querySelectorAll("{}");
                    var results = [];
                    els.forEach(function(el) {{ results.push(el.textContent.trim()); }});
                    return JSON.stringify(results);
                }})()
            "#, selector.replace('"', r#"\""#));
            
            let resp = client.post(&format!("{}/execute", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session, "script": script }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let result = body.get("result").and_then(|r| r.as_str()).unwrap_or("[]");
            
            Ok(vec![json!({ "type": "text", "text": result })])
        }
        
        "click" => {
            let selector = args.get("selector").and_then(|s| s.as_str()).ok_or("Missing 'selector'")?;
            
            let resp = client.post(&format!("{}/click", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session, "selector": selector }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let success = body.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
            
            Ok(vec![json!({ "type": "text", "text": if success { "clicked" } else { "click failed" } })])
        }
        
        "type_text" => {
            let selector = args.get("selector").and_then(|s| s.as_str()).ok_or("Missing 'selector'")?;
            let text = args.get("text").and_then(|t| t.as_str()).ok_or("Missing 'text'")?;
            
            let resp = client.post(&format!("{}/type", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session, "selector": selector, "text": text }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let success = body.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
            
            Ok(vec![json!({ "type": "text", "text": if success { "typed" } else { "type failed" } })])
        }
        
        "execute" => {
            let script = args.get("script").and_then(|s| s.as_str()).ok_or("Missing 'script'")?;
            
            let resp = client.post(&format!("{}/execute", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session, "script": script }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let result = body.get("result").and_then(|r| r.as_str()).unwrap_or("");
            
            Ok(vec![json!({ "type": "text", "text": result })])
        }
        
        "get_text" => {
            let resp = client.post(&format!("{}/execute", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": session, "script": "document.body.innerText" }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let text = body.get("result").and_then(|r| r.as_str()).unwrap_or("");
            
            Ok(vec![json!({ "type": "text", "text": text })])
        }
        
        "youtube_subtitles" => {
            let url = args.get("url").and_then(|u| u.as_str()).ok_or("Missing 'url'")?;
            let language = args.get("language").and_then(|l| l.as_str()).unwrap_or("ja");
            let format = args.get("format").and_then(|f| f.as_str()).unwrap_or("text");
            let auto_generated = args.get("auto_generated").and_then(|a| a.as_bool()).unwrap_or(true);
            
            // Proxy to v2/mcp endpoint
            let resp = client.post(&format!("{}/v2/mcp", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "youtube_subtitles",
                        "arguments": {
                            "url": url,
                            "language": language,
                            "format": format,
                            "auto_generated": auto_generated
                        }
                    }
                }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let content = body.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("{}");
            
            Ok(vec![json!({ "type": "text", "text": content })])
        }
        
        "youtube_download" => {
            let url = args.get("url").and_then(|u| u.as_str()).ok_or("Missing 'url'")?;
            let quality = args.get("quality").and_then(|q| q.as_str()).unwrap_or("best");
            let audio_only = args.get("audio_only").and_then(|a| a.as_bool()).unwrap_or(false);
            
            // Use session-specific media directory by default
            let session_media_dir = crate::core::config::AppConfig::get_session_media_dir(session);
            let default_output_dir = session_media_dir.to_string_lossy().to_string();
            let output_dir = args.get("output_dir")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string())
                .unwrap_or(default_output_dir);
            
            // Proxy to v2/mcp endpoint
            let resp = client.post(&format!("{}/v2/mcp", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "youtube_download",
                        "arguments": {
                            "url": url,
                            "quality": quality,
                            "audio_only": audio_only,
                            "output_dir": output_dir
                        }
                    }
                }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let content = body.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("{}");
            
            Ok(vec![json!({ "type": "text", "text": content })])
        }
        
        "collect_images" => {
            let selector = args.get("selector").and_then(|s| s.as_str()).unwrap_or("body");
            let min_width = args.get("min_width").and_then(|w| w.as_u64()).unwrap_or(0);
            let min_height = args.get("min_height").and_then(|h| h.as_u64()).unwrap_or(0);
            let max_images = args.get("max_images").and_then(|m| m.as_u64()).unwrap_or(100);
            let download = args.get("download").and_then(|d| d.as_bool()).unwrap_or(false);
            
            // Proxy to v2/mcp endpoint
            let resp = client.post(&format!("{}/v2/mcp", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "collect_images",
                        "arguments": {
                            "session": session,
                            "selector": selector,
                            "min_width": min_width,
                            "min_height": min_height,
                            "max_images": max_images,
                            "download": download
                        }
                    }
                }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let content = body.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("{}");
            
            Ok(vec![json!({ "type": "text", "text": content })])
        }
        
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn read_resource(
    uri: &str,
    client: &ureq::Agent,
    current_session: &str,
) -> Result<Vec<Value>, String> {
    match uri {
        "browser://current/text" => {
            let resp = client.post(&format!("{}/execute", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": current_session, "script": "document.body.innerText" }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let text = body.get("result").and_then(|r| r.as_str()).unwrap_or("");
            
            Ok(vec![json!({ "uri": uri, "mimeType": "text/plain", "text": text })])
        }
        
        "browser://current/screenshot" => {
            let resp = client.post(&format!("{}/screenshot", SERVER_URL))
                .set("Content-Type", "application/json")
                .send_json(json!({ "session": current_session }))
                .map_err(|e| format!("HTTP error: {}", e))?;
            
            let body: Value = resp.into_json().map_err(|e| format!("JSON error: {}", e))?;
            let image = body.get("image").and_then(|i| i.as_str()).unwrap_or("");
            
            Ok(vec![json!({ "uri": uri, "mimeType": "image/png", "blob": image })])
        }
        
        "browser://screenshots/list" => {
            // List saved screenshots
            use base64::Engine;
            
            let screenshots_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("webview-bridge")
                .join("media")
                .join("screenshots");
            
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&screenshots_dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            if let Some(name) = entry.file_name().to_str() {
                                files.push(json!({
                                    "filename": name,
                                    "size_bytes": metadata.len(),
                                    "uri": format!("browser://screenshots/{}", name)
                                }));
                            }
                        }
                    }
                }
            }
            
            Ok(vec![json!({
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&json!({ "screenshots": files, "count": files.len() })).unwrap_or_default()
            })])
        }
        
        _ if uri.starts_with("browser://screenshots/") => {
            // Get specific screenshot file
            use base64::Engine;
            
            let filename = uri.strip_prefix("browser://screenshots/").unwrap_or("");
            if filename.is_empty() || filename.contains("..") || filename.contains('/') || filename.contains('\\') {
                return Err("Invalid filename".to_string());
            }
            
            let screenshots_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("webview-bridge")
                .join("media")
                .join("screenshots");
            
            let filepath = screenshots_dir.join(filename);
            
            let data = std::fs::read(&filepath)
                .map_err(|e| format!("Failed to read screenshot: {}", e))?;
            
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);
            
            Ok(vec![json!({
                "uri": uri,
                "mimeType": "image/png",
                "blob": base64_data
            })])
        }
        
        _ => Err(format!("Unknown resource: {}", uri)),
    }
}
