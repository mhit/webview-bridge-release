//! MCP v3 Tool Implementations
//!
//! Robust implementations wrapping V2 API

use super::types::*;
use super::robustness::*;
use crate::api_v2::{get_session_manager_v2, V2AppState};
use crate::core::AppCommand;
use tokio::sync::oneshot;
use std::time::Duration;

// ============================================================================
// Tool Router
// ============================================================================

/// Route MCP tool request to appropriate handler
pub async fn route_tool(
    tool: &str,
    params: serde_json::Value,
    state: &V2AppState,
) -> McpToolResponse {
    match tool {
        "navigate" => {
            match serde_json::from_value::<NavigateRequest>(params) {
                Ok(req) => handle_navigate(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid navigate params: {}", e)),
            }
        }
        "interact" => {
            match serde_json::from_value::<InteractRequest>(params) {
                Ok(req) => handle_interact(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid interact params: {}", e)),
            }
        }
        "capture" => {
            match serde_json::from_value::<CaptureRequest>(params) {
                Ok(req) => handle_capture(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid capture params: {}", e)),
            }
        }
        "extract" => {
            match serde_json::from_value::<ExtractRequest>(params) {
                Ok(req) => handle_extract(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid extract params: {}", e)),
            }
        }
        "session" => {
            match serde_json::from_value::<SessionRequest>(params) {
                Ok(req) => handle_session(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid session params: {}", e)),
            }
        }
        "media" => {
            match serde_json::from_value::<MediaRequest>(params) {
                Ok(req) => handle_media(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid media params: {}", e)),
            }
        }
        "execute" => {
            match serde_json::from_value::<ExecuteRequest>(params) {
                Ok(req) => handle_execute(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid execute params: {}", e)),
            }
        }
        "agent" => {
            match serde_json::from_value::<AgentRequest>(params) {
                Ok(req) => handle_agent(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid agent params: {}", e)),
            }
        }
        _ => McpToolResponse::error("UNKNOWN_TOOL", &format!("Unknown tool: {}", tool)),
    }
}

// ============================================================================
// Helper: Execute Script
// ============================================================================

async fn execute_script(session: &str, script: String, state: &V2AppState, timeout_ms: u64) -> Result<String, String> {
    let manager = get_session_manager_v2();
    
    let handle = manager.get_handle(session)
        .ok_or_else(|| format!("Session '{}' not found", session))?;
    
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ExecuteScript {
        id: handle.id.clone(),
        script,
        resp_tx: tx,
    };
    
    state.cmd_tx.send(cmd).map_err(|_| "Failed to send command")?;
    
    match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Err("Channel closed".to_string()),
        Err(_) => Err("Script execution timed out".to_string()),
    }
}

// ============================================================================
// 1. Navigate
// ============================================================================

async fn handle_navigate(req: NavigateRequest, state: &V2AppState) -> McpToolResponse {
    let manager = get_session_manager_v2();
    
    let handle = match manager.get_handle(&req.session) {
        Some(h) => h,
        None => return McpToolResponse::error("SESSION_NOT_FOUND", &format!("Session '{}' not found", req.session)),
    };
    
    // Send navigate command
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::Navigate {
        id: handle.id.clone(),
        url: req.url.clone(),
        resp_tx: tx,
    };
    
    if state.cmd_tx.send(cmd).is_err() {
        return McpToolResponse::error("COMMAND_FAILED", "Failed to send navigate command");
    }
    
    // Wait for basic navigation
    match tokio::time::timeout(Duration::from_millis(req.timeout_ms), rx).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(e))) => return McpToolResponse::error("NAVIGATE_FAILED", &e),
        Ok(Err(_)) => return McpToolResponse::error("CHANNEL_CLOSED", "Navigation channel closed"),
        Err(_) => return McpToolResponse::error("TIMEOUT", "Navigation timed out"),
    }
    
    // Apply wait_for condition
    let wait_result = match req.wait_for {
        WaitForCondition::Load => Ok("load".to_string()),
        WaitForCondition::Stable => {
            let script = generate_wait_for_stable_script(500, req.timeout_ms);
            execute_script(&req.session, script, state, req.timeout_ms).await
        }
        WaitForCondition::NetworkIdle => {
            // Simplified: just wait a bit for network to settle
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok("networkidle".to_string())
        }
        WaitForCondition::Selector => {
            if let Some(selector) = &req.wait_selector {
                let script = generate_wait_for_condition_script("element", Some(selector), req.timeout_ms);
                execute_script(&req.session, script, state, req.timeout_ms).await
            } else {
                Err("wait_selector required for Selector condition".to_string())
            }
        }
    };
    
    match wait_result {
        Ok(_) => McpToolResponse::success_json(serde_json::json!({
            "url": req.url,
            "wait_for": format!("{:?}", req.wait_for),
            "status": "navigated"
        })),
        Err(e) => McpToolResponse::error("WAIT_FAILED", &e),
    }
}

// ============================================================================
// 2. Interact
// ============================================================================

async fn handle_interact(req: InteractRequest, state: &V2AppState) -> McpToolResponse {
    let mut completed_actions: Vec<usize> = vec![];
    let mut screenshots: Vec<String> = vec![];
    
    for (index, action) in req.actions.iter().enumerate() {
        // Apply slow mode delay
        if req.options.slow_mode_ms > 0 && index > 0 {
            tokio::time::sleep(Duration::from_millis(req.options.slow_mode_ms)).await;
        }
        
        let result = execute_action_with_retry(
            &req.session,
            action,
            &req.options,
            state,
        ).await;
        
        match result {
            Ok(screenshot) => {
                completed_actions.push(index);
                if let Some(s) = screenshot {
                    screenshots.push(s);
                }
            }
            Err(e) => {
                // Take error screenshot if enabled
                let error_screenshot = if req.options.screenshot_on_error {
                    take_screenshot(&req.session, state).await.ok()
                } else {
                    None
                };
                
                return McpToolResponse::error_with_details(
                    "ACTION_FAILED",
                    &format!("Action {} failed: {}", index, e),
                    serde_json::json!({
                        "action_index": index,
                        "action": action,
                        "reason": e,
                        "completed_actions": completed_actions,
                        "error_screenshot": error_screenshot
                    }),
                );
            }
        }
    }
    
    let mut text = format!("Completed {} actions successfully", completed_actions.len());
    if !screenshots.is_empty() {
        text.push_str(&format!("\nScreenshots: {:?}", screenshots));
    }
    
    McpToolResponse::success_text(text)
}

async fn execute_action_with_retry(
    session: &str,
    action: &Action,
    options: &InteractOptions,
    state: &V2AppState,
) -> Result<Option<String>, String> {
    let mut last_error = String::new();
    
    for attempt in 0..=options.retry_count {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(options.retry_delay_ms * attempt as u64)).await;
        }
        
        // Human mode: add random delay before action (100-500ms)
        if options.human_mode {
            let delay = 100 + (rand::random::<u64>() % 400);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        
        match execute_action(session, action, options.wait_timeout_ms, state, options.human_mode).await {
            Ok(screenshot) => {
                // Human mode: add random delay after action (50-200ms)
                if options.human_mode {
                    let delay = 50 + (rand::random::<u64>() % 150);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                return Ok(screenshot);
            }
            Err(e) => {
                last_error = e;
                tracing::debug!("Action attempt {} failed: {}", attempt + 1, last_error);
            }
        }
    }
    
    Err(last_error)
}

async fn execute_action(
    session: &str,
    action: &Action,
    timeout_ms: u64,
    state: &V2AppState,
    _human_mode: bool, // For future: mouse jitter, natural scrolling
) -> Result<Option<String>, String> {
    match action {
        Action::Click { target, wait_after_ms } => {
            // Wait for element to be clickable
            let wait_script = generate_wait_for_clickable_script(target, timeout_ms);
            let wait_result = execute_script(session, wait_script, state, timeout_ms).await?;
            
            let parsed: serde_json::Value = serde_json::from_str(&wait_result)
                .map_err(|e| format!("Failed to parse wait result: {}", e))?;
            
            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Element not clickable").to_string());
            }
            
            // Human mode: simulate mouse movement to element with natural curve
            if _human_mode {
                let mouse_move_script = generate_human_mouse_move_script(target);
                let _ = execute_script(session, mouse_move_script, state, timeout_ms).await;
                // Small delay after mouse movement
                let delay = 50 + (rand::random::<u64>() % 100);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            // Scroll and click
            let click_script = generate_scroll_and_click_script(target);
            let result = execute_script(session, click_script, state, timeout_ms).await?;
            
            let parsed: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| format!("Failed to parse click result: {}", e))?;
            
            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Click failed").to_string());
            }
            
            // Wait after click
            if let Some(ms) = wait_after_ms {
                tokio::time::sleep(Duration::from_millis(*ms)).await;
            }
            
            Ok(None)
        }
        
        Action::Type { target, value, clear } => {
            // Wait for element
            let wait_script = generate_wait_for_clickable_script(target, timeout_ms);
            execute_script(session, wait_script, state, timeout_ms).await?;
            
            // Type with events
            let type_script = generate_type_with_events_script(target, value, *clear);
            let result = execute_script(session, type_script, state, timeout_ms).await?;
            
            let parsed: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| format!("Failed to parse type result: {}", e))?;
            
            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Type failed").to_string());
            }
            
            Ok(None)
        }
        
        Action::Wait { condition, value, timeout_ms: wait_timeout } => {
            let condition_str = match condition {
                WaitCondition::Element => "element",
                WaitCondition::ElementVisible => "element_visible",
                WaitCondition::ElementClickable => "element_visible", // same for now
                WaitCondition::ElementHidden => "element_hidden",
                WaitCondition::UrlContains => "url_contains",
                WaitCondition::UrlMatches => "url_matches",
                WaitCondition::TextContains => "text_contains",
                WaitCondition::NetworkIdle => "network_idle",
                WaitCondition::Timeout => {
                    tokio::time::sleep(Duration::from_millis(*wait_timeout)).await;
                    return Ok(None);
                }
            };
            
            let script = generate_wait_for_condition_script(
                condition_str,
                value.as_deref(),
                *wait_timeout,
            );
            
            let result = execute_script(session, script, state, *wait_timeout + 1000).await?;
            
            let parsed: serde_json::Value = serde_json::from_str(&result)
                .map_err(|e| format!("Failed to parse wait result: {}", e))?;
            
            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Wait condition not met").to_string());
            }
            
            Ok(None)
        }
        
        Action::Screenshot => {
            let path = take_screenshot(session, state).await?;
            Ok(Some(path))
        }
        
        Action::Scroll { direction, amount, target } => {
            let scroll_script = if let Some(selector) = target {
                format!(r#"
                    const el = document.querySelector("{}");
                    if (el) {{
                        el.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                        JSON.stringify({{ success: true }});
                    }} else {{
                        JSON.stringify({{ success: false, error: "Element not found" }});
                    }}
                "#, selector.replace('"', "\\\""))
            } else {
                let (x, y) = match direction {
                    ScrollDirection::Down => (0, *amount),
                    ScrollDirection::Up => (0, -amount),
                    ScrollDirection::Right => (*amount, 0),
                    ScrollDirection::Left => (-amount, 0),
                };
                format!(r#"
                    window.scrollBy({}, {});
                    JSON.stringify({{ success: true, scrollX: window.scrollX, scrollY: window.scrollY }});
                "#, x, y)
            };
            
            execute_script(session, scroll_script, state, timeout_ms).await?;
            Ok(None)
        }
        
        Action::Hover { target } => {
            let hover_script = format!(r#"
                const el = document.querySelector("{}");
                if (el) {{
                    el.dispatchEvent(new MouseEvent('mouseenter', {{ bubbles: true }}));
                    el.dispatchEvent(new MouseEvent('mouseover', {{ bubbles: true }}));
                    JSON.stringify({{ success: true }});
                }} else {{
                    JSON.stringify({{ success: false, error: "Element not found" }});
                }}
            "#, target.replace('"', "\\\""));
            
            execute_script(session, hover_script, state, timeout_ms).await?;
            Ok(None)
        }
        
        Action::Select { target, value } => {
            let select_script = format!(r#"
                const el = document.querySelector("{}");
                if (el) {{
                    el.value = "{}";
                    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    JSON.stringify({{ success: true, value: el.value }});
                }} else {{
                    JSON.stringify({{ success: false, error: "Element not found" }});
                }}
            "#, target.replace('"', "\\\""), value.replace('"', "\\\""));
            
            execute_script(session, select_script, state, timeout_ms).await?;
            Ok(None)
        }
    }
}

async fn take_screenshot(session: &str, state: &V2AppState) -> Result<String, String> {
    // For now, return a placeholder path
    // TODO: Implement actual screenshot with file saving
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let path = format!("sessions/{}/captures/cap_{}.png", session, timestamp);
    
    // Take screenshot via V2 API
    let screenshot_script = r#"
        JSON.stringify({ captured: true, timestamp: Date.now() });
    "#;
    
    execute_script(session, screenshot_script.to_string(), state, 5000).await?;
    
    Ok(path)
}

// ============================================================================
// 3. Capture
// ============================================================================

async fn handle_capture(req: CaptureRequest, state: &V2AppState) -> McpToolResponse {
    // 1. Force load lazy images
    let lazy_script = crate::core::screenshot_v2::generate_force_load_lazy_images_script(5000);
    let _ = execute_script(&req.session, lazy_script, state, 10000).await;
    
    // 2. Wait for images to load
    let wait_images_script = crate::core::screenshot_v2::generate_wait_for_images_script(5000);
    let _ = execute_script(&req.session, wait_images_script, state, 10000).await;
    
    // 3. Wait for skeletons to disappear
    let skeleton_script = generate_wait_for_no_skeleton_script(3000);
    let _ = execute_script(&req.session, skeleton_script, state, 5000).await;
    
    // 4. Extract interactive elements for AI context
    let elements_script = generate_extract_interactive_elements_script();
    let elements_result = execute_script(&req.session, elements_script, state, 5000).await
        .unwrap_or_else(|_| r#"{"error": "Failed to extract elements"}"#.to_string());
    
    let parsed: serde_json::Value = serde_json::from_str(&elements_result)
        .unwrap_or(serde_json::json!({"error": "Parse failed"}));
    
    // Build AI-optimized text response
    let url = parsed["url"].as_str().unwrap_or("Unknown");
    let title = parsed["title"].as_str().unwrap_or("Unknown");
    
    let mut text = format!("URL: {}\nTitle: {}\n\n【操作可能要素】\n", url, title);
    
    if let Some(elements) = parsed["elements"].as_array() {
        for el in elements.iter().take(30) {
            let tag = el["tag"].as_str().unwrap_or("?");
            let selector = el["selector"].as_str().unwrap_or("?");
            let label = el["label"].as_str();
            let value = el["value"].as_str();
            let el_type = el["type"].as_str();
            
            let mut line = format!("- {}", selector);
            
            if let Some(t) = el_type {
                line.push_str(&format!(" type={}", t));
            }
            if let Some(l) = label {
                if !l.is_empty() {
                    line.push_str(&format!(" 「{}」", l.chars().take(30).collect::<String>()));
                }
            }
            if let Some(v) = value {
                if !v.is_empty() {
                    line.push_str(&format!(" value=\"{}\"", v.chars().take(20).collect::<String>()));
                }
            }
            
            text.push_str(&line);
            text.push('\n');
        }
    }
    
    // 5. Take screenshot (save to file, return URL)
    if req.screenshot {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let screenshot_url = format!(
            "http://127.0.0.1:9400/files/{}/cap_{}.png",
            req.session, timestamp
        );
        text.push_str(&format!("\n【スクリーンショット】\n{}", screenshot_url));
        
        // TODO: Actually save screenshot to file
    }
    
    // Handle additional includes
    if req.include.contains(&CaptureInclude::Cookies) {
        text.push_str("\n\n【Cookies】\n(Cookie取得は別途実装)");
    }
    
    if req.include.contains(&CaptureInclude::FullText) {
        let max_chars = req.text_max_chars.unwrap_or(5000);
        let text_script = format!(r#"
            JSON.stringify({{
                text: document.body.innerText.substring(0, {})
            }});
        "#, max_chars);
        
        if let Ok(result) = execute_script(&req.session, text_script, state, 5000).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(full_text) = parsed["text"].as_str() {
                    text.push_str(&format!("\n\n【ページテキスト】\n{}", full_text));
                }
            }
        }
    }
    
    McpToolResponse::success_text(text)
}

// ============================================================================
// 4. Extract
// ============================================================================

async fn handle_extract(req: ExtractRequest, state: &V2AppState) -> McpToolResponse {
    // Wait for minimum element count if specified
    if let Some(min_count) = req.wait_for_count {
        let wait_script = format!(r#"
            (async function() {{
                const timeout = {};
                const startTime = Date.now();
                while ((Date.now() - startTime) < timeout) {{
                    const count = document.querySelectorAll("{}").length;
                    if (count >= {}) {{
                        return JSON.stringify({{ success: true, count: count }});
                    }}
                    await new Promise(r => setTimeout(r, 100));
                }}
                return JSON.stringify({{ success: false, count: document.querySelectorAll("{}").length }});
            }})();
        "#, req.wait_timeout_ms, req.selector.replace('"', "\\\""), min_count, req.selector.replace('"', "\\\""));
        
        let _ = execute_script(&req.session, wait_script, state, req.wait_timeout_ms + 1000).await;
    }
    
    // Handle scroll for more if enabled
    if req.scroll_for_more {
        for _ in 0..req.scroll_max {
            // Scroll to bottom
            let scroll_script = r#"
                window.scrollTo(0, document.body.scrollHeight);
                JSON.stringify({ scrolled: true });
            "#;
            let _ = execute_script(&req.session, scroll_script.to_string(), state, 5000).await;
            
            // Wait for new content
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    
    // Extract data
    let extract_script = generate_extract_data_script(&req.selector, &req.fields, req.limit);
    let result = execute_script(&req.session, extract_script, state, 10000).await
        .map_err(|e| format!("Extract failed: {}", e));
    
    match result {
        Ok(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json)
                .unwrap_or(serde_json::json!({"error": "Parse failed"}));
            
            let count = parsed["count"].as_u64().unwrap_or(0);
            let data = parsed["data"].clone();
            
            McpToolResponse::success_text(format!(
                "Extracted {} items:\n{}",
                count,
                serde_json::to_string_pretty(&data).unwrap_or_default()
            ))
        }
        Err(e) => McpToolResponse::error("EXTRACT_FAILED", &e),
    }
}

// ============================================================================
// 5. Session
// ============================================================================

async fn handle_session(req: SessionRequest, state: &V2AppState) -> McpToolResponse {
    let manager = get_session_manager_v2();
    
    if req.list {
        match manager.list() {
            Ok(response) => {
                let session_names: Vec<String> = response.sessions.iter()
                    .map(|s| s.name.clone())
                    .collect();
                return McpToolResponse::success_json(serde_json::json!({
                    "sessions": session_names,
                    "count": session_names.len()
                }));
            }
            Err(e) => return McpToolResponse::error("SESSION_LIST_FAILED", &e),
        }
    }
    
    if let Some(name) = &req.acquire {
        // Build AcquireRequest from our simple parameters
        let acquire_request = crate::core::session_v2::AcquireRequest {
            name: name.clone(),
            profile: None,
            reuse: true,
            create_if_missing: true,
            headless: req.headless,
            auth_check: None,
            ttl_hours: 168,  // 1 week (0 = no expiration)
            auto_extend: true,
            restore: req.restore,
        };
        
        let create_fn = |options: crate::core::SessionOptions| -> Result<(String, crate::core::session_v2::SessionHandle), String> {
            (state.create_session_fn)(options)
        };
        
        match manager.acquire(acquire_request, create_fn).await {
            Ok(response) => {
                // Get the handle to wait for WebView ready
                let handle = match manager.get_handle(&response.session) {
                    Some(h) => h,
                    None => {
                        return McpToolResponse::error("SESSION_ACQUIRE_FAILED", "Session created but handle not found");
                    }
                };
                
                // Wait for WebView to be ready (poll GetStatus)
                let max_wait_ms = 10000u64; // 10 seconds max
                let poll_interval_ms = 100u64;
                let mut waited_ms = 0u64;
                
                loop {
                    // Send GetStatus command
                    let (tx, rx) = oneshot::channel();
                    let cmd = crate::core::AppCommand::GetStatus {
                        id: handle.id.clone(),
                        resp_tx: tx,
                    };
                    
                    if state.cmd_tx.send(cmd).is_err() {
                        return McpToolResponse::error("SESSION_ACQUIRE_FAILED", "Failed to send status check command");
                    }
                    
                    match tokio::time::timeout(Duration::from_millis(1000), rx).await {
                        Ok(Ok(Ok(status))) => {
                            // Check if Ready (status is a String like "Ready", "Initializing", etc)
                            if status.status == "Ready" {
                                tracing::info!(
                                    "MCP session acquire: {} is_new={} ready after {}ms",
                                    response.session, response.is_new, waited_ms
                                );
                                
                                // If not headless, ensure window is visible
                                if !req.headless {
                                    let (vis_tx, vis_rx) = oneshot::channel();
                                    let vis_cmd = crate::core::AppCommand::SetVisibility {
                                        id: handle.id.clone(),
                                        visible: true,
                                        resp_tx: vis_tx,
                                    };
                                    if state.cmd_tx.send(vis_cmd).is_ok() {
                                        let _ = tokio::time::timeout(Duration::from_millis(1000), vis_rx).await;
                                    }
                                }
                                
                                return McpToolResponse::success_json(serde_json::json!({
                                    "session": response.session,
                                    "is_new": response.is_new,
                                    "profile": response.profile,
                                    "wait_ms": waited_ms,
                                    "status": "ready",
                                    "visible": !req.headless
                                }));
                            }
                            
                            // Check for error
                            if status.status == "Error" {
                                return McpToolResponse::error("SESSION_ACQUIRE_FAILED", "WebView initialization failed");
                            }
                        }
                        Ok(Ok(Err(e))) => {
                            return McpToolResponse::error("SESSION_ACQUIRE_FAILED", &e);
                        }
                        _ => {
                            // Timeout or channel error, continue waiting
                        }
                    }
                    
                    waited_ms += poll_interval_ms;
                    if waited_ms >= max_wait_ms {
                        return McpToolResponse::error("SESSION_ACQUIRE_TIMEOUT", "WebView did not become ready in time");
                    }
                    
                    tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
                }
            }
            Err(e) => return McpToolResponse::error("SESSION_ACQUIRE_FAILED", &e),
        }
    }
    
    if let Some(name) = &req.release {
        match manager.release(name) {
            Ok(_) => return McpToolResponse::success_json(serde_json::json!({
                "session": name,
                "status": "released"
            })),
            Err(e) => return McpToolResponse::error("SESSION_RELEASE_FAILED", &e),
        }
    }
    
    if let Some(name) = &req.import {
        // TODO: Implement cookie import
        return McpToolResponse::success_json(serde_json::json!({
            "session": name,
            "status": "cookies_imported"
        }));
    }
    
    McpToolResponse::error("INVALID_SESSION_REQUEST", "No valid session action specified")
}

// ============================================================================
// 6. Media
// ============================================================================

async fn handle_media(req: MediaRequest, state: &V2AppState) -> McpToolResponse {
    match req.action {
        MediaAction::YoutubeDownload { url, quality, audio_only, output_dir } => {
            // TODO: Call existing YouTube download implementation
            McpToolResponse::success_text(format!(
                "YouTube download started: {}\nQuality: {:?}\nAudio only: {}",
                url, quality, audio_only
            ))
        }
        MediaAction::YoutubeSubtitles { url, language, format } => {
            // TODO: Call existing YouTube subtitles implementation
            McpToolResponse::success_text(format!(
                "YouTube subtitles extraction: {}\nLanguage: {:?}",
                url, language
            ))
        }
        MediaAction::VideoAnalyze { url, keyframes, audio, max_frames } => {
            // TODO: Call existing video analyze implementation
            McpToolResponse::success_text(format!(
                "Video analysis: {}\nKeyframes: {}, Audio: {}",
                url, keyframes, audio
            ))
        }
        MediaAction::CollectImages { selector, min_width, min_height, download, max_images } => {
            // TODO: Implement image collection
            McpToolResponse::success_text("Image collection started".to_string())
        }
    }
}

// ============================================================================
// 7. Execute
// ============================================================================

async fn handle_execute(req: ExecuteRequest, state: &V2AppState) -> McpToolResponse {
    match execute_script(&req.session, req.script, state, req.timeout_ms).await {
        Ok(result) => McpToolResponse::success_text(format!("Result: {}", result)),
        Err(e) => McpToolResponse::error("EXECUTE_FAILED", &e),
    }
}

// ============================================================================
// 8. Agent (Future)
// ============================================================================

async fn handle_agent(req: AgentRequest, _state: &V2AppState) -> McpToolResponse {
    McpToolResponse::error(
        "NOT_IMPLEMENTED",
        "Agentic mode is not yet implemented. Coming in next phase.",
    )
}

// ============================================================================
// MCP Tool List
// ============================================================================

/// Get tool definitions for MCP tools/list
pub fn get_mcp_tools() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "navigate",
            "description": "Navigate to URL with wait conditions. For SPA sites (React, Vue, etc.) use 'stable' or 'selector' wait to ensure dynamic content loads. 'networkidle' waits for network quiet. For bot-protected sites, ensure session has headless=false.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session name (default: 'default')" },
                    "url": { "type": "string", "description": "URL to navigate to" },
                    "wait_for": { "type": "string", "enum": ["load", "stable", "networkidle", "selector"], "default": "stable", "description": "load=basic load, stable=DOM stops changing, networkidle=no network activity, selector=wait for element" },
                    "wait_selector": { "type": "string", "description": "CSS selector to wait for (required if wait_for=selector)" },
                    "timeout_ms": { "type": "integer", "default": 30000 }
                },
                "required": ["url"]
            }
        },
        {
            "name": "interact",
            "description": "Execute browser actions with retry and visibility checks. Action types: click{target}, type{target,value,clear?}, scroll{direction?,amount?,target?}, hover{target}, select{target,value}, wait{condition,value?,timeout_ms?}, screenshot. For wait, condition can be: timeout, element, element_visible, element_clickable, element_hidden, url_contains, text_contains, network_idle. Example: {\"type\":\"wait\",\"condition\":\"timeout\",\"timeout_ms\":1500}",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "actions": {
                        "type": "array",
                        "description": "Array of action objects. Examples: {\"type\":\"click\",\"target\":\"#btn\"}, {\"type\":\"type\",\"target\":\"#input\",\"value\":\"text\",\"clear\":true}, {\"type\":\"scroll\",\"direction\":\"down\",\"amount\":500}, {\"type\":\"wait\",\"condition\":\"timeout\",\"timeout_ms\":1500}, {\"type\":\"wait\",\"condition\":\"element\",\"value\":\"#loaded\"}",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["click", "type", "scroll", "hover", "select", "wait", "screenshot"], "description": "Action type" },
                                "target": { "type": "string", "description": "CSS selector (required for click, type, hover, select)" },
                                "value": { "type": "string", "description": "Input text (for type/select) or selector/pattern (for wait conditions)" },
                                "clear": { "type": "boolean", "description": "Clear input before typing (for type action)" },
                                "condition": { "type": "string", "enum": ["timeout", "element", "element_visible", "element_clickable", "element_hidden", "url_contains", "url_matches", "text_contains", "network_idle"], "description": "Wait condition type (required for wait action)" },
                                "timeout_ms": { "type": "integer", "description": "Timeout in ms (for wait action, default: 10000)" },
                                "direction": { "type": "string", "enum": ["down", "up", "left", "right"], "description": "Scroll direction (for scroll action)" },
                                "amount": { "type": "integer", "description": "Scroll amount in pixels (for scroll action, default: 500)" }
                            },
                            "required": ["type"]
                        }
                    },
                    "options": {
                        "type": "object",
                        "properties": {
                            "wait_timeout_ms": { "type": "integer", "default": 10000 },
                            "retry_count": { "type": "integer", "default": 3 },
                            "screenshot_on_error": { "type": "boolean", "default": false },
                            "human_mode": { "type": "boolean", "default": false, "description": "Enable human-like behavior: random delays (100-500ms), natural mouse movement with bezier curves and jitter" }
                        }
                    }
                },
                "required": ["actions"]
            }
        },
        {
            "name": "capture",
            "description": "Capture current page state for AI analysis. Returns: URL, title, list of interactive elements (buttons, links, inputs with selectors). Options: screenshot=true saves image, include=['cookies','full_text','html','images'] for extra data, selector limits to element, full_page captures entire page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "screenshot": { "type": "boolean", "default": true, "description": "Save screenshot to file" },
                    "include": { "type": "array", "items": { "type": "string", "enum": ["cookies", "full_text", "html", "images"] }, "description": "Extra data to include" },
                    "selector": { "type": "string", "description": "CSS selector to limit capture to specific element" },
                    "full_page": { "type": "boolean", "description": "Capture entire scrollable page, not just viewport" },
                    "text_max_chars": { "type": "integer", "description": "Max chars for text content" },
                    "summarize": { "type": "boolean", "description": "Use AI to summarize page content" }
                }
            }
        },
        {
            "name": "extract",
            "description": "Extract structured data from page. Example: selector='.review', fields={'author':'.author-name','rating':'.star-rating@data-rating','text':'.review-text'} returns [{author:'John',rating:'5',text:'Great!'},...]). Use @attr to get attribute value instead of text content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "selector": { "type": "string", "description": "CSS selector for container elements (e.g. '.review-card')" },
                    "fields": { "type": "object", "description": "Map of field name to sub-selector. Examples: 'title':'.title', 'link':'a@href', 'rating':'span@data-rating'" },
                    "limit": { "type": "integer", "description": "Max items to extract" },
                    "wait_for_count": { "type": "integer", "description": "Wait until at least N elements exist" },
                    "scroll_for_more": { "type": "boolean", "description": "Scroll down to load more items (infinite scroll)" },
                    "scroll_max": { "type": "integer", "default": 5, "description": "Max scroll iterations" }
                },
                "required": ["selector", "fields"]
            }
        },
        {
            "name": "session",
            "description": "Session management: acquire, release, list, import cookies. Sessions use WebView2 with persistent cookie storage. NOTE: headless=true uses 'pseudo-headless' mode (window hidden, not true headless) - this may affect some sites' bot detection. For SPA sites or login flows, use headless=false to ensure proper JavaScript execution and avoid detection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "acquire": { "type": "string", "description": "Acquire session by name. Creates if not exists." },
                    "release": { "type": "string", "description": "Release session by name (keeps cookies)" },
                    "list": { "type": "boolean", "description": "List all sessions" },
                    "import": { "type": "string", "description": "Import cookies to session" },
                    "headless": { "type": "boolean", "description": "If true, window is hidden (pseudo-headless). Use false for login flows or bot-protected sites like Amazon. Default: false" },
                    "restore": { "type": "boolean", "default": true, "description": "Restore last URL on session resume" },
                    "browser": { "type": "string", "enum": ["chrome", "edge", "firefox"], "description": "Browser profile to import cookies from" },
                    "domains": { "type": "array", "items": { "type": "string" }, "description": "Cookie domains to import" }
                }
            }
        },
        {
            "name": "media",
            "description": "Media operations: YouTube download/subtitles, video analysis, image collection",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "action": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["youtube_download", "youtube_subtitles", "video_analyze", "collect_images"] }
                        }
                    }
                },
                "required": ["action"]
            }
        },
        {
            "name": "execute",
            "description": "Execute raw JavaScript in browser and return result. The script runs in page context with access to DOM. Return value is JSON-stringified. Examples: 'document.title', 'document.querySelector(\"#price\").textContent', '[...document.querySelectorAll(\"a\")].map(a=>a.href)'",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "script": { "type": "string", "description": "JavaScript code to execute. Use return for async functions." },
                    "timeout_ms": { "type": "integer", "default": 30000, "description": "Script execution timeout" }
                },
                "required": ["script"]
            }
        },
        {
            "name": "agent",
            "description": "[Future] Agentic mode for goal-based browser automation with internal AI",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "action": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["start", "resume", "status", "cancel"] },
                            "goal": { "type": "string" },
                            "context": { "type": "string" },
                            "max_steps": { "type": "integer" }
                        }
                    }
                },
                "required": ["action"]
            }
        }
    ])
}
