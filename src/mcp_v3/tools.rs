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
    let result = route_tool_inner(tool, params.clone(), state).await;

    // Auto-reacquire: if a session-dependent tool gets SESSION_NOT_FOUND,
    // automatically re-acquire the session and retry the operation.
    // This eliminates the common "expired → manual re-acquire → retry" cycle.
    if let Some(ref err) = result.error {
        if err.code == "SESSION_NOT_FOUND" && tool != "session" {
            if let Some(session_name) = params.get("session").and_then(|s| s.as_str()) {
                if !session_name.is_empty() {
                    tracing::info!("Auto-reacquiring session '{}' for tool '{}'", session_name, tool);

                    // Build a minimal acquire request
                    let acquire_req = SessionRequest {
                        session: None,
                        acquire: Some(session_name.to_string()),
                        release: None,
                        list: false,
                        import: None,
                        clone_to: None,
                        headless: false,
                        restore: true,
                        ttl_hours: 168,
                        browser: None,
                        domains: None,
                        ai_status: false,
                        ai_models: false,
                        ai_config: None,
                        device: None,
                        viewport_width: None,
                        viewport_height: None,
                        user_agent: None,
                    };

                    let acquire_result = handle_session(acquire_req, state).await;
                    if acquire_result.success {
                        tracing::info!("Session '{}' re-acquired, retrying '{}'", session_name, tool);
                        let retry_result = route_tool_inner(tool, params, state).await;
                        // Return retry result with a note about auto-reacquire
                        if retry_result.success {
                            return retry_result;
                        }
                        // Retry also failed — return the retry error, not the original
                        return retry_result;
                    }
                    // Acquire failed — return original error with hint
                    return McpToolResponse::error("SESSION_NOT_FOUND",
                        &format!("Session '{}' not found. Auto-reacquire failed — the session may have been deleted. Create a new session: use the session tool with {{\"acquire\": \"{}\"}}", session_name, session_name));
                }
            }
        }
    }

    result
}

/// Inner routing — dispatches to tool handlers without auto-reacquire logic
async fn route_tool_inner(
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
        "network" => {
            match serde_json::from_value::<NetworkRequest>(params) {
                Ok(req) => handle_network(req, state).await,
                Err(e) => McpToolResponse::error("INVALID_PARAMS", &format!("Invalid network params: {}", e)),
            }
        }
        _ => McpToolResponse::error("UNKNOWN_TOOL", &format!("Unknown tool: '{}'. Available tools: navigate, interact, capture, extract, session, media, execute, agent, network", tool)),
    }
}

// ============================================================================
// Helper: Execute Script
// ============================================================================

async fn execute_script(session: &str, script: String, state: &V2AppState, timeout_ms: u64) -> Result<String, String> {
    execute_script_in(session, script, state, timeout_ms, None).await
}

async fn execute_script_in(session: &str, script: String, state: &V2AppState, timeout_ms: u64, frame: Option<&str>) -> Result<String, String> {
    let manager = get_session_manager_v2();

    let handle = manager.get_handle(session)
        .ok_or_else(|| format!("Session '{}' not found", session))?;

    let (tx, rx) = oneshot::channel();
    let cmd = if let Some(frame_spec) = frame {
        AppCommand::ExecuteInFrame {
            id: handle.id.clone(),
            script,
            frame: frame_spec.to_string(),
            resp_tx: tx,
        }
    } else {
        AppCommand::ExecuteScript {
            id: handle.id.clone(),
            script,
            resp_tx: tx,
        }
    };

    state.cmd_tx.send(cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;

    match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
        Err(_) => Err("Script execution timed out. The script may have an infinite loop or be waiting for a resource.".to_string()),
    }
}

// ============================================================================
// Helper: Get Cookies (CDP via AppCommand)
// ============================================================================

async fn get_cookies_cdp(session: &str, state: &V2AppState) -> Result<String, String> {
    let manager = get_session_manager_v2();
    
    let handle = manager.get_handle(session)
        .ok_or_else(|| format!("Session '{}' not found", session))?;
    
    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::GetCookies {
        id: handle.id.clone(),
        resp_tx: tx,
    };
    
    state.cmd_tx.send(cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;
    
    match tokio::time::timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
        Err(_) => Err("GetCookies timed out. The session may be unresponsive. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
    }
}

// ============================================================================
// Helper: Set Cookies (CDP via AppCommand)
// ============================================================================

async fn set_cookies_cdp(session: &str, cookies_json: String, state: &V2AppState) -> Result<(), String> {
    let manager = get_session_manager_v2();

    let handle = manager.get_handle(session)
        .ok_or_else(|| format!("Session '{}' not found", session))?;

    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::SetCookies {
        id: handle.id.clone(),
        cookies: cookies_json,
        resp_tx: tx,
    };

    state.cmd_tx.send(cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;

    match tokio::time::timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
        Err(_) => Err("SetCookies timed out. The session may be unresponsive. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
    }
}

/// Convert ImportedCookie list to CDP CookieInfo JSON array
fn imported_cookies_to_cdp_json(cookies: &[crate::core::cookie_import::ImportedCookie]) -> String {
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
    serde_json::to_string(&cdp_cookies).unwrap_or_else(|_| "[]".to_string())
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
            // Wait for network to truly settle (no pending XHR/fetch for 500ms)
            let script = generate_wait_for_condition_script("network_idle", None, req.timeout_ms);
            execute_script(&req.session, script, state, req.timeout_ms).await
        }
        WaitForCondition::Selector => {
            if let Some(selector) = &req.wait_selector {
                let script = generate_wait_for_condition_script("element", Some(selector), req.timeout_ms);
                execute_script(&req.session, script, state, req.timeout_ms).await
            } else {
                Err("wait_selector is required when using 'Selector' condition. Example: {\"condition\": \"Selector\", \"wait_selector\": \".my-element\"}".to_string())
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
            req.frame.as_deref(),
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
                let error_screenshot_url = error_screenshot.as_ref().and_then(|uri| {
                    let stripped = uri.strip_prefix("browser://screenshots/")?;
                    let (s, f) = stripped.split_once('/')?;
                    Some(to_screenshot_http_url(s, f))
                });

                return McpToolResponse::error_with_details(
                    "ACTION_FAILED",
                    &format!("Action {} failed: {}", index, e),
                    serde_json::json!({
                        "action_index": index,
                        "action": action,
                        "reason": e,
                        "completed_actions": completed_actions,
                        "error_screenshot": error_screenshot,
                        "error_screenshot_url": error_screenshot_url
                    }),
                );
            }
        }
    }
    
    let mut text = format!("Completed {} actions successfully", completed_actions.len());
    if !screenshots.is_empty() {
        // screenshots contain browser:// URIs - also provide HTTP URLs
        let http_urls: Vec<String> = screenshots.iter().filter_map(|uri| {
            // Parse browser://screenshots/{session}/{filename}
            let stripped = uri.strip_prefix("browser://screenshots/")?;
            let (session, filename) = stripped.split_once('/')?;
            Some(to_screenshot_http_url(session, filename))
        }).collect();
        text.push_str(&format!("\nScreenshots: {:?}", screenshots));
        if !http_urls.is_empty() {
            text.push_str(&format!("\nView at: {:?}", http_urls));
        }
    }

    McpToolResponse::success_text(text)
}

async fn execute_action_with_retry(
    session: &str,
    action: &Action,
    options: &InteractOptions,
    state: &V2AppState,
    frame: Option<&str>,
) -> Result<Option<String>, String> {
    let mut last_error = String::new();
    
    for attempt in 0..=options.retry_count {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(options.retry_delay_ms * attempt as u64)).await;
        }
        
        // Human mode: add random delay before action (100-500ms)
        if options.human_mode {
            let delay = 100 + (rand::random::<u64>() % 400);
            tracing::info!("[human_mode] Pre-action delay: {}ms", delay);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        
        match execute_action(session, action, options.wait_timeout_ms, state, options.human_mode, frame).await {
            Ok(screenshot) => {
                // Human mode: add random delay after action (50-200ms)
                if options.human_mode {
                    let delay = 50 + (rand::random::<u64>() % 150);
                    tracing::info!("[human_mode] Post-action delay: {}ms", delay);
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
    human_mode: bool,
    frame: Option<&str>,
) -> Result<Option<String>, String> {
    match action {
        Action::Click { target, wait_after_ms } => {
            // Quick element check (sync, no polling loop)
            let check_script = format!(r#"
                (function() {{
                    const el = document.querySelector("{}");
                    if (!el) return JSON.stringify({{ success: false, error: "Element not found" }});
                    const rect = el.getBoundingClientRect();
                    if (rect.width === 0 || rect.height === 0) return JSON.stringify({{ success: false, error: "Element hidden" }});
                    return JSON.stringify({{ success: true }});
                }})()
            "#, target.replace('"', "\\\""));
            let check_result = execute_script_in(session, check_script, state, 5000, frame).await?;

            let parsed: serde_json::Value = serde_json::from_str(&check_result)
                .map_err(|e| format!("Failed to parse check result: {}", e))?;

            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Element not clickable. It may be hidden, disabled, or covered by another element. Try: scroll to the element, wait for it to be visible, or use a more specific selector.").to_string());
            }

            // Human mode: add delay before click
            if human_mode {
                let delay = 50 + (rand::random::<u64>() % 100);
                tracing::info!("[human_mode] Pre-click delay: {}ms", delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            // CDP click
            tracing::info!("[cdp] Using CDP Input.dispatchMouseEvent for click: {}, human={}", target, human_mode);
            let manager = get_session_manager_v2();
            if let Some(handle) = manager.get_handle(session) {
                let (tx, rx) = oneshot::channel();
                let cmd = crate::core::AppCommand::ClickCdp {
                    id: handle.id.clone(),
                    selector: target.clone(),
                    human_mode,
                    resp_tx: tx,
                };
                state.cmd_tx.send(cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;
                match tokio::time::timeout(Duration::from_secs(10), rx).await {
                    Ok(Ok(Ok(()))) => tracing::info!("[cdp] Click succeeded"),
                    Ok(Ok(Err(e))) => return Err(format!("CDP click failed: {}", e)),
                    Ok(Err(_)) => return Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
                    Err(_) => return Err("Click timed out. The element may not be interactable. Try: wait for it to be visible, or use a different selector.".to_string()),
                }
            } else {
                return Err(format!("Session '{}' not found", session));
            }
            
            // Wait after click
            if let Some(ms) = wait_after_ms {
                tokio::time::sleep(Duration::from_millis(*ms)).await;
            }
            
            Ok(None)
        }
        
        Action::Type { target, value, clear, instant } => {
            // Quick element check
            let check_script = format!(r#"
                (function() {{
                    const el = document.querySelector("{}");
                    if (!el) return JSON.stringify({{ success: false, error: "Element not found" }});
                    return JSON.stringify({{ success: true }});
                }})()
            "#, target.replace('"', "\\\""));
            let check_result = execute_script_in(session, check_script, state, 5000, frame).await?;

            let parsed: serde_json::Value = serde_json::from_str(&check_result)
                .map_err(|e| format!("Failed to parse check result: {}", e))?;

            if !parsed["success"].as_bool().unwrap_or(false) {
                return Err(parsed["error"].as_str().unwrap_or("Element not found for typing. The element may not exist yet. Try: wait for it to appear, or check the selector.").to_string());
            }

            // CDP type (instant=true uses 0 delay, normal uses 20ms, human_mode uses random)
            let char_delay = if *instant { 0 } else if human_mode { 50 + (rand::random::<u64>() % 100) } else { 20 };
            tracing::info!("[cdp] Type via CDP: {} chars, delay={}ms, instant={}", value.len(), char_delay, instant);
            
            let manager = get_session_manager_v2();
            if let Some(handle) = manager.get_handle(session) {
                // Click to focus
                let (click_tx, click_rx) = oneshot::channel();
                let click_cmd = crate::core::AppCommand::ClickCdp {
                    id: handle.id.clone(),
                    selector: target.clone(),
                    human_mode: false, // Focus click doesn't need human movements
                    resp_tx: click_tx,
                };
                state.cmd_tx.send(click_cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;
                match tokio::time::timeout(Duration::from_secs(10), click_rx).await {
                    Ok(Ok(Ok(()))) => tracing::info!("[cdp] Focus click succeeded"),
                    Ok(Ok(Err(e))) => return Err(format!("CDP focus click failed: {}", e)),
                    Ok(Err(_)) => return Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
                    Err(_) => return Err("Focus click timed out. The input element may not be visible. Try: scroll to the element first.".to_string()),
                }
                
                if !*instant {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                
                // Clear if requested
                if *clear {
                    let clear_script = format!(r#"
                        (function() {{
                            const el = document.querySelector("{}");
                            if (el) {{ el.value = ""; el.dispatchEvent(new Event('input', {{bubbles: true}})); }}
                        }})()
                    "#, target.replace('"', "\\\""));
                    let _ = execute_script_in(session, clear_script, state, 2000, frame).await;
                }
                
                // Type via CDP
                let (type_tx, type_rx) = oneshot::channel();
                let type_cmd = crate::core::AppCommand::TypeCdp {
                    id: handle.id.clone(),
                    text: value.clone(),
                    char_delay_ms: char_delay,
                    human_mode,
                    resp_tx: type_tx,
                };
                state.cmd_tx.send(type_cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;
                
                let type_timeout_secs = if *instant { 10 } else { (value.len() as u64 * char_delay / 1000) + 10 };
                match tokio::time::timeout(Duration::from_secs(type_timeout_secs), type_rx).await {
                    Ok(Ok(Ok(()))) => tracing::info!("[cdp] Type succeeded: {} chars", value.len()),
                    Ok(Ok(Err(e))) => return Err(format!("CDP type failed: {}", e)),
                    Ok(Err(_)) => return Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
                    Err(_) => return Err("Type timed out. The input may not be focused. Try: click the input element first, then type.".to_string()),
                }
            } else {
                return Err(format!("Session '{}' not found", session));
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
            
            let result = execute_script_in(session, script, state, *wait_timeout + 1000, frame).await?;
            
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
            } else if human_mode {
                // Human-like inertia scroll: starts fast, gradually slows down
                let (dx, dy) = match direction {
                    ScrollDirection::Down => (0, *amount),
                    ScrollDirection::Up => (0, -amount),
                    ScrollDirection::Right => (*amount, 0),
                    ScrollDirection::Left => (-amount, 0),
                };
                format!(r#"
(async function() {{
    const totalX = {};
    const totalY = {};
    const steps = 15 + Math.floor(Math.random() * 10); // 15-25 steps
    
    // Ease-out function: fast start, slow end (deceleration)
    const easeOut = (t) => 1 - Math.pow(1 - t, 3);
    
    let scrolledX = 0;
    let scrolledY = 0;
    
    for (let i = 1; i <= steps; i++) {{
        const progress = easeOut(i / steps);
        const targetX = Math.round(totalX * progress);
        const targetY = Math.round(totalY * progress);
        
        const stepX = targetX - scrolledX;
        const stepY = targetY - scrolledY;
        
        // Add slight randomness to simulate hand wheel movement
        const jitterX = (Math.random() - 0.5) * 3;
        const jitterY = (Math.random() - 0.5) * 3;
        
        window.scrollBy(stepX + jitterX, stepY + jitterY);
        scrolledX = targetX;
        scrolledY = targetY;
        
        // Variable delay: faster at start, slower at end
        const delay = 10 + (i / steps) * 25 + Math.random() * 10;
        await new Promise(r => setTimeout(r, delay));
    }}
    
    return JSON.stringify({{ 
        success: true, 
        scrollX: window.scrollX, 
        scrollY: window.scrollY,
        human_mode: true,
        steps: steps
    }});
}})();
"#, dx, dy)
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
            
            let scroll_result = execute_script_in(session, scroll_script, state, timeout_ms, frame).await?;

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&scroll_result) {
                if parsed["success"].as_bool() == Some(false) {
                    return Err(parsed["error"].as_str().unwrap_or("Scroll target element not found. Try: check the selector or scroll the window instead.").to_string());
                }
            }
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

            let hover_result = execute_script_in(session, hover_script, state, timeout_ms, frame).await?;

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&hover_result) {
                if parsed["success"].as_bool() == Some(false) {
                    return Err(parsed["error"].as_str().unwrap_or("Hover target element not found. Try: check the selector or wait for the element to appear.").to_string());
                }
            }
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

            let select_result = execute_script_in(session, select_script, state, timeout_ms, frame).await?;

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&select_result) {
                if parsed["success"].as_bool() == Some(false) {
                    return Err(parsed["error"].as_str().unwrap_or("Select target element not found. Try: check the selector or wait for the dropdown to appear.").to_string());
                }
            }
            Ok(None)
        }
    }
}

/// Convert a screenshot path to a compact browser:// URI for MCP responses
fn to_screenshot_uri(session: &str, filename: &str) -> String {
    format!("browser://screenshots/{}/{}", session, filename)
}

/// Build the HTTP URL to retrieve a screenshot via the REST API
fn to_screenshot_http_url(session: &str, filename: &str) -> String {
    let cfg = crate::core::config::get_config();
    let port = cfg.server.port;
    format!("http://127.0.0.1:{}/media/screenshots/{}/{}", port, session, filename)
}

async fn take_screenshot(session: &str, state: &V2AppState) -> Result<String, String> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    
    // Create screenshot directory
    let screenshot_dir = crate::core::config::AppConfig::profile_screenshots_dir(session);
    let _ = std::fs::create_dir_all(&screenshot_dir);
    
    let filename = format!("cap_{}.png", timestamp);
    let screenshot_path = screenshot_dir.join(&filename);
    
    // Take screenshot via CDP
    let manager = get_session_manager_v2();
    let handle = manager.get_handle(session)
        .ok_or_else(|| format!("Session '{}' not found", session))?;
    
    let (tx, rx) = oneshot::channel();
    let cdp_cmd = crate::core::AppCommand::ScreenshotCdp {
        id: handle.id.clone(),
        full_page: false,
        format: "png".to_string(),
        quality: None,
        frame: None,
        resp_tx: tx,
    };
    
    state.cmd_tx.send(cdp_cmd).map_err(|_| "Failed to send command to session. The session may not be active. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.")?;
    
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(bytes))) => {
            // Save to file
            std::fs::write(&screenshot_path, &bytes)
                .map_err(|e| format!("Failed to save screenshot: {}", e))?;
            tracing::info!("[take_screenshot] Saved {} bytes to {:?}", bytes.len(), screenshot_path);
            Ok(to_screenshot_uri(session, &filename))
        }
        Ok(Ok(Err(e))) => Err(format!("Screenshot failed: {}", e)),
        Ok(Err(_)) => Err("Session communication lost. The browser session may have crashed. Try: use the session tool with {\"acquire\": \"<name>\"} to re-acquire.".to_string()),
        Err(_) => Err("Screenshot timed out. The page may be very large or still rendering. Try: wait for page load first.".to_string()),
    }
}

// ============================================================================
// 3. Capture
// ============================================================================

async fn handle_capture(req: CaptureRequest, state: &V2AppState) -> McpToolResponse {
    // 1. Force load lazy images
    let frame = req.frame.as_deref();
    let lazy_script = crate::core::screenshot_v2::generate_force_load_lazy_images_script(5000);
    let _ = execute_script_in(&req.session, lazy_script, state, 10000, frame).await;

    // 2. Wait for images to load
    let wait_images_script = crate::core::screenshot_v2::generate_wait_for_images_script(5000);
    let _ = execute_script_in(&req.session, wait_images_script, state, 10000, frame).await;

    // 3. Wait for skeletons to disappear
    let skeleton_script = generate_wait_for_no_skeleton_script(3000);
    let _ = execute_script_in(&req.session, skeleton_script, state, 5000, frame).await;

    // 4. Extract interactive elements for AI context
    let elements_script = generate_extract_interactive_elements_script();
    let elements_result = execute_script_in(&req.session, elements_script, state, 5000, frame).await
        .unwrap_or_else(|_| r#"{"error": "Failed to extract elements"}"#.to_string());
    
    let parsed: serde_json::Value = serde_json::from_str(&elements_result)
        .unwrap_or(serde_json::json!({"error": "Parse failed"}));
    
    // 4.5 Optional: LLM analysis for mid-range scores
    let mut analyzed_result = if req.analyze_interactivity {
        let ai_config = crate::core::ai::AiConfig::default();
        if ai_config.is_available() {
            match crate::mcp_v3::visual_interactivity::analyze_with_llm(&elements_result, &ai_config) {
                Ok(analyzed) => {
                    serde_json::from_str(&analyzed).unwrap_or(parsed.clone())
                }
                Err(e) => {
                    eprintln!("[Visual Interactivity] LLM analysis failed: {}", e);
                    parsed.clone()
                }
            }
        } else {
            parsed.clone()
        }
    } else {
        parsed.clone()
    };
    
    // Re-sort elements by score after LLM analysis
    if req.analyze_interactivity {
        if let Some(elements) = analyzed_result.get_mut("elements").and_then(|e| e.as_array_mut()) {
            elements.sort_by(|a, b| {
                let score_a = a.get("interactivity")
                    .and_then(|i| i.get("score"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0);
                let score_b = b.get("interactivity")
                    .and_then(|i| i.get("score"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0);
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
    
    // 4.6 Optional: Vision LLM analysis for images without alt text
    let final_result = if req.analyze_vision {
        let ai_config = crate::core::ai::AiConfig::default();
        if ai_config.is_available() {
            // Future: Capture viewport screenshot and pass to Vision LLM
            
            // Get elements that need vision analysis
            let needs_vision_elements: Vec<_> = analyzed_result.get("elements")
                .and_then(|e| e.as_array())
                .map(|elements| {
                    elements.iter()
                        .enumerate()
                        .filter(|(_, el)| {
                            el.get("interactivity")
                                .and_then(|i| i.get("needs_vision"))
                                .and_then(|n| n.as_bool())
                                .unwrap_or(false)
                        })
                        .take(5)
                        .map(|(idx, el)| {
                            let selector = el.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                            let tag = el.get("tag").and_then(|t| t.as_str()).unwrap_or("");
                            let visual = el.get("visual");
                            let size = visual.and_then(|v| v.get("size"));
                            let width = size.and_then(|s| s.get("width")).and_then(|w| w.as_i64()).unwrap_or(0);
                            let height = size.and_then(|s| s.get("height")).and_then(|h| h.as_i64()).unwrap_or(0);
                            (idx, selector.to_string(), tag.to_string(), width, height)
                        })
                        .collect()
                })
                .unwrap_or_default();
            
            if !needs_vision_elements.is_empty() {
                // Build prompt with element positions for Vision LLM
                let mut prompt = String::from(
                    "このページのスクリーンショットを分析してください。\n\n" 
                );
                prompt.push_str("以下の画像要素のクリック可能性と内容を判断してください:\n");
                
                for (idx, selector, tag, width, height) in &needs_vision_elements {
                    prompt.push_str(&format!(
                        "- 要素{}: {} ({}x{}px) セレクタ: {}\n",
                        idx, tag, width, height, selector
                    ));
                }
                
                prompt.push_str("\nJSON形式で回答:\n");
                prompt.push_str(r#"[{"index": 0, "description": "商品画像", "action": "click_product"}]"#);
                
                // Call Vision LLM
                let vision_result = match ai_config.provider.to_lowercase().as_str() {
                    "ollama" => {
                        let client = crate::core::ai::OllamaClient::new(&ai_config);
                        // Note: For now call without image, future: pass screenshot
                        client.call(&prompt, None)
                    }
                    "gemini" => {
                        if let Some(client) = crate::core::ai::GeminiClient::new(&ai_config) {
                            client.call(&prompt, None)
                        } else {
                            Err("Gemini not available".to_string())
                        }
                    }
                    _ => Err("Unsupported provider".to_string())
                };
                
                // Apply vision results if successful
                if let Ok(response) = vision_result {
                    let mut result = analyzed_result.clone();
                    // Parse response and update elements
                    if let Some(elements) = result.get_mut("elements").and_then(|e| e.as_array_mut()) {
                        // Try to extract JSON array from response
                        if let Some(start) = response.find('[') {
                            if let Some(end) = response.rfind(']') {
                                let json_str = &response[start..=end];
                                if let Ok(vision_data) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                                    for item in vision_data {
                                        if let Some(idx) = item.get("index").and_then(|i| i.as_u64()) {
                                            if let Some(el) = elements.get_mut(idx as usize) {
                                                if let Some(desc) = item.get("description").and_then(|d| d.as_str()) {
                                                    el["vision_description"] = serde_json::json!(desc);
                                                    // Update label if empty
                                                    if el.get("label").and_then(|l| l.as_str()).unwrap_or("").is_empty() {
                                                        el["label"] = serde_json::json!(desc.chars().take(30).collect::<String>());
                                                    }
                                                }
                                                if let Some(_action) = item.get("action").and_then(|a| a.as_str()) {
                                                    if let Some(interactivity) = el.get_mut("interactivity") {
                                                        interactivity["analyzed_by"] = serde_json::json!("vision");
                                                        interactivity["needs_vision"] = serde_json::json!(false);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    result
                } else {
                    analyzed_result.clone()
                }
            } else {
                analyzed_result.clone()
            }
        } else {
            analyzed_result.clone()
        }
    } else {
        analyzed_result.clone()
    };
    
    // Build AI-optimized text response
    let url = final_result["url"].as_str().unwrap_or("Unknown");
    let title = final_result["title"].as_str().unwrap_or("Unknown");
    
    let mut text = format!("URL: {}\nTitle: {}\n\n【操作可能要素】\n", url, title);
    
    if let Some(elements) = final_result["elements"].as_array() {
        // Filter: score >= 0.4, limit to top 20 for context efficiency
        // (threshold raised due to position bonuses)
        let filtered: Vec<_> = elements.iter()
            .filter(|el| {
                el.get("interactivity")
                    .and_then(|i| i.get("score"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) >= 0.4
            })
            .take(20)
            .collect();
        
        // Show count info
        text.push_str(&format!("({}要素中 上位{}件)\n", elements.len(), filtered.len()));
        
        for el in filtered {
            // Truncate selector for AI context efficiency
            let selector = el["selector"].as_str().unwrap_or("?");
            let short_selector: String = if selector.len() > 40 {
                format!("{}...", selector.chars().take(37).collect::<String>())
            } else {
                selector.to_string()
            };
            let label = el["label"].as_str();
            let value = el["value"].as_str();
            let el_type = el["type"].as_str();
            
            // Include interactivity score if available
            let score = el["interactivity"]["score"].as_f64();
            let analyzed_by = el["interactivity"]["analyzed_by"].as_str();
            
            // Get size and position from visual properties
            let width = el["visual"]["size"]["width"].as_u64().unwrap_or(0);
            let height = el["visual"]["size"]["height"].as_u64().unwrap_or(0);
            let in_viewport = el["inViewport"].as_bool().unwrap_or(true);
            
            let mut line = format!("- {}", short_selector);
            
            if let Some(s) = score {
                line.push_str(&format!(" [score:{:.2}]", s));
            }
            
            // Add size info (compact format)
            if width > 0 && height > 0 {
                line.push_str(&format!(" {}×{}", width, height));
            }
            
            if !in_viewport {
                line.push_str(" (画面外)");
            }
            
            if let Some(a) = analyzed_by {
                if a == "llm" {
                    line.push_str(" (LLM)");
                }
            }
            if let Some(t) = el_type {
                line.push_str(&format!(" type={}", t));
            }
            if let Some(l) = label {
                if !l.is_empty() {
                    line.push_str(&format!(" 「{}」", l.chars().take(20).collect::<String>()));
                }
            }
            if let Some(v) = value {
                if !v.is_empty() {
                    line.push_str(&format!(" value=\"{}\"", v.chars().take(20).collect::<String>()));
                }
            }
            
            // Show first predicted action
            if let Some(actions) = el["interactivity"]["predicted_actions"].as_array() {
                if let Some(first) = actions.first() {
                    if let Some(action) = first["action"].as_str() {
                        line.push_str(&format!(" → {}", action));
                    }
                }
            }
            
            text.push_str(&line);
            text.push('\n');
        }
    }
    
    // Add challenge detection results with AI-actionable strategies
    if let Some(challenges) = final_result.get("challenges").and_then(|c| c.as_array()) {
        if !challenges.is_empty() {
            text.push_str("\n【チャレンジ検出】\n");
            for challenge in challenges {
                let challenge_type = challenge.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                let auto_strategy = challenge.get("auto_strategy");
                
                // Get recommended action
                let action = auto_strategy
                    .and_then(|s| s.get("action"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("unknown");
                
                let message = auto_strategy
                    .and_then(|s| s.get("message"))
                    .and_then(|m| m.as_str());
                
                text.push_str(&format!("- {}: ", challenge_type));
                
                match action {
                    "proceed" => {
                        text.push_str("自動処理可能");
                        if let Some(msg) = message {
                            text.push_str(&format!(" ({})", msg));
                        }
                    }
                    "wait_and_retry" => {
                        let timeout = auto_strategy
                            .and_then(|s| s.get("timeout_ms"))
                            .and_then(|t| t.as_u64())
                            .unwrap_or(10000);
                        text.push_str(&format!("待機推奨 ({}ms後リトライ)", timeout));
                    }
                    "click_checkbox" => {
                        let selector = auto_strategy
                            .and_then(|s| s.get("selector"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        text.push_str(&format!("→ click {} で解決試行可", selector));
                    }
                    _ => {
                        text.push_str("対応方法を検討中");
                    }
                }
                text.push('\n');
            }
        }
    }

    
    // 5. Take screenshot (save to file, return URL)
    if req.screenshot {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("cap_{}.png", timestamp);
        
        // Get screenshot output directory (inside session profile folder)
        let screenshot_dir = crate::core::config::AppConfig::profile_screenshots_dir(&req.session);
        let _ = std::fs::create_dir_all(&screenshot_dir);
        let screenshot_path = screenshot_dir.join(&filename);
        
        // Use CDP screenshot for full_page or when explicitly requested
        if req.use_cdp || req.full_page {
            // Get session manager
            let manager = get_session_manager_v2();
            if let Some(handle) = manager.get_handle(&req.session) {
                let (tx, rx) = oneshot::channel();
                let cdp_cmd = crate::core::AppCommand::ScreenshotCdp {
                    id: handle.id.clone(),
                    full_page: req.full_page,
                    format: "png".to_string(),
                    quality: None,
                    frame: None,
                    resp_tx: tx,
                };
                
                if state.cmd_tx.send(cdp_cmd).is_ok() {
                    match tokio::time::timeout(Duration::from_secs(30), rx).await {
                        Ok(Ok(Ok(bytes))) => {
                            // Save to file
                            match std::fs::write(&screenshot_path, &bytes) {
                                Ok(_) => {
                                    text.push_str(&format!("\n【スクリーンショット】(CDP)\n{}　({} bytes)\nView: {}",
                                        to_screenshot_uri(&req.session, &filename), bytes.len(),
                                        to_screenshot_http_url(&req.session, &filename)));
                                }
                                Err(e) => {
                                    text.push_str(&format!("\n【スクリーンショット保存失敗】{}", e));
                                }
                            }
                        }
                        Ok(Ok(Err(e))) => {
                            text.push_str(&format!("\n【CDPスクリーンショット失敗】{}", e));
                        }
                        _ => {
                            text.push_str("\n【CDPスクリーンショットタイムアウト】");
                        }
                    }
                } else {
                    text.push_str("\n【コマンド送信失敗】");
                }
            } else {
                text.push_str(&format!("\n【セッション未検出】{}", req.session));
            }
        } else {
            // Fallback: just report page dimensions (existing behavior)
            let screenshot_script = r#"
                (async function() {
                    const canvas = document.createElement('canvas');
                    const ctx = canvas.getContext('2d');
                    canvas.width = window.innerWidth;
                    canvas.height = window.innerHeight;
                    
                    try {
                        return JSON.stringify({
                            success: true,
                            width: window.innerWidth,
                            height: window.innerHeight,
                            scroll: { x: window.scrollX, y: window.scrollY }
                        });
                    } catch(e) {
                        return JSON.stringify({ success: false, error: e.message });
                    }
                })();
            "#;
            
            match execute_script_in(&req.session, screenshot_script.to_string(), state, 5000, frame).await {
                Ok(result) => {
                    text.push_str(&format!("\n【ページ情報】\n{}", result));
                }
                Err(e) => {
                    text.push_str(&format!("\n【スクリーンショット失敗】{}", e));
                }
            }
        }
    }
    
    // Handle additional includes
    if req.include.contains(&CaptureInclude::Cookies) {
        // Use CDP Network.getCookies for HttpOnly cookies
        match get_cookies_cdp(&req.session, state).await {
            Ok(cookies_json) => {
                if let Ok(cookies) = serde_json::from_str::<Vec<serde_json::Value>>(&cookies_json) {
                    text.push_str(&format!("\n\n【Cookies】({}件)\n", cookies.len()));
                    for cookie in cookies.iter().take(20) {
                        let name = cookie["name"].as_str().unwrap_or("?");
                        let value = cookie["value"].as_str().unwrap_or("").chars().take(30).collect::<String>();
                        let http_only = cookie["httpOnly"].as_bool().unwrap_or(false);
                        let suffix = if http_only { " [HttpOnly]" } else { "" };
                        text.push_str(&format!("- {}={}{}\n", name, value, suffix));
                    }
                    if cookies.len() > 20 {
                        text.push_str(&format!("... 他{}件\n", cookies.len() - 20));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("[capture] CDP cookie fetch failed: {}", e);
                text.push_str("\n\n【Cookies】取得失敗");
            }
        }
    }
    
    if req.include.contains(&CaptureInclude::FullText) {
        let max_chars = req.text_max_chars.unwrap_or(5000);
        let text_script = format!(r#"
            JSON.stringify({{
                text: document.body.innerText.substring(0, {})
            }});
        "#, max_chars);
        
        if let Ok(result) = execute_script_in(&req.session, text_script, state, 5000, frame).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(full_text) = parsed["text"].as_str() {
                    text.push_str(&format!("\n\n【ページテキスト】\n{}", full_text));
                }
            }
        }
    }
    
    if req.include.contains(&CaptureInclude::Html) {
        let max_chars = req.text_max_chars.unwrap_or(50000);
        let html_script = format!(r#"
            JSON.stringify({{
                html: document.documentElement.outerHTML.substring(0, {})
            }});
        "#, max_chars);
        
        if let Ok(result) = execute_script_in(&req.session, html_script, state, 5000, frame).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(html) = parsed["html"].as_str() {
                    text.push_str(&format!("\n\n【HTML】({}文字)\n{}", html.len(), html));
                }
            }
        }
    }
    
    if req.include.contains(&CaptureInclude::Images) {
        let images_script = generate_collect_images_script(None, 50, 50, 30);
        
        if let Ok(result) = execute_script_in(&req.session, images_script, state, 5000, frame).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(images) = parsed["images"].as_array() {
                    text.push_str(&format!("\n\n【画像】({}件)\n", images.len()));
                    for img in images.iter().take(10) {
                        let src = img["src"].as_str().unwrap_or("?");
                        let alt = img["alt"].as_str().unwrap_or("");
                        let w = img["width"].as_u64().unwrap_or(0);
                        let h = img["height"].as_u64().unwrap_or(0);
                        text.push_str(&format!("- {}x{} {} {}\n", w, h, alt, src.chars().take(60).collect::<String>()));
                    }
                    if images.len() > 10 {
                        text.push_str(&format!("... 他{}件\n", images.len() - 10));
                    }
                }
            }
        }
    }
    
    // AI Summarization
    if req.summarize {
        // Get page text for summarization
        let text_script = r#"
            JSON.stringify({
                text: document.body.innerText.substring(0, 8000)
            });
        "#;
        
        if let Ok(result) = execute_script_in(&req.session, text_script.to_string(), state, 5000, frame).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(page_text) = parsed["text"].as_str() {
                    // Create AI client
                    let config = crate::core::config::get_config();
                    tracing::info!("[summarize] provider={}, model={}", config.ai.provider, config.ai.model);
                    
                    let ai_config = crate::core::ai::AiConfig {
                        enabled: config.ai.enabled,
                        provider: config.ai.provider.clone(),
                        model: config.ai.model.clone(),
                        api_key: config.ai.api_key.clone(),
                        timeout_ms: config.ai.timeout_ms,
                        daily_budget_usd: config.ai.daily_budget_usd,
                        daily_usage_usd: 0.0,
                    };
                    
                    // Debug: check Ollama availability
                    if config.ai.provider.to_lowercase() == "ollama" {
                        let ollama = crate::core::ai::OllamaClient::new(&ai_config);
                        tracing::info!("[summarize] Ollama available: {}, base_url: {}", ollama.is_available(), ollama.base_url);
                        
                        // Use Ollama directly (skip AiClient fallback logic)
                        let truncated_text: String = page_text.chars().take(4000).collect();
                        let prompt = format!(
                            "以下のウェブページの内容を200文字以内で簡潔に要約してください。\n\n---\n{}",
                            truncated_text
                        );
                        
                        match ollama.call(&prompt, None) {
                            Ok(summary) => {
                                text.push_str(&format!("\n\n【AI要約】(ollama/{})\n{}", 
                                    config.ai.model,
                                    summary.trim()
                                ));
                            }
                            Err(e) => {
                                text.push_str(&format!("\n\n【AI要約】Ollamaエラー: {}", e));
                            }
                        }
                    } else if let Some(client) = crate::core::ai::AiClient::new(&ai_config) {
                        let truncated_text: String = page_text.chars().take(4000).collect();
                        let prompt = format!(
                            "以下のウェブページの内容を200文字以内で簡潔に要約してください。\n\n---\n{}",
                            truncated_text
                        );
                        
                        match client.call(&prompt, None) {
                            Ok(summary) => {
                                text.push_str(&format!("\n\n【AI要約】({})\n{}", 
                                    client.provider_name(),
                                    summary.trim()
                                ));
                            }
                            Err(e) => {
                                text.push_str(&format!("\n\n【AI要約】エラー: {}", e));
                            }
                        }
                    } else {
                        text.push_str("\n\n【AI要約】AI未設定（provider/api_keyを確認）");
                    }
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
    let frame = req.frame.as_deref();
    // Smart wait: always ensure elements exist before extraction
    // If wait_for_count is specified, use that; otherwise auto-wait for at least 1 element
    let min_count = req.wait_for_count.unwrap_or(1);
    let auto_wait_timeout = if req.wait_for_count.is_some() {
        req.wait_timeout_ms
    } else {
        5000 // Default 5s auto-wait for elements to appear
    };
    
    let wait_script = format!(r#"
        (async function() {{
            const timeout = {};
            const startTime = Date.now();
            while ((Date.now() - startTime) < timeout) {{
                const count = document.querySelectorAll("{}").length;
                if (count >= {}) {{
                    return JSON.stringify({{ success: true, count: count }});
                }}
                await new Promise(r => setTimeout(r, 200));
            }}
            return JSON.stringify({{ success: false, count: document.querySelectorAll("{}").length }});
        }})();
    "#, auto_wait_timeout, req.selector.replace('"', "\\\""), min_count, req.selector.replace('"', "\\\""));
    
    let wait_result = execute_script_in(&req.session, wait_script, state, auto_wait_timeout + 1000, frame).await;
    
    // Log wait result for diagnostics
    if let Ok(ref wr) = wait_result {
        tracing::info!("[extract] Element wait result: {}", wr);
    }
    
    // Handle scroll-and-collect mode: extract → scroll → repeat with dedup
    if req.scroll_for_more {
        use std::collections::HashSet;
        use std::hash::{Hash, Hasher};

        let mut accumulated: Vec<serde_json::Value> = Vec::new();
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut no_new_streak: u32 = 0;
        let scroll_amount = req.scroll_amount;
        let scroll_delay = req.scroll_delay_ms;
        let dedup_key = req.scroll_dedup_key.as_deref();
        let limit = req.limit;
        let mut total_duplicates: usize = 0;

        for i in 0..req.scroll_max {
            // Extract current visible items
            let extract_script = generate_extract_data_script(&req.selector, &req.fields, None);
            let batch_result = execute_script_in(&req.session, extract_script, state, 10000, frame).await;

            let mut new_count = 0usize;
            if let Ok(json) = batch_result {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                    if let Some(items) = parsed["data"].as_array() {
                        for item in items {
                            // Compute dedup key
                            let key = if let Some(dk) = dedup_key {
                                item.get(dk)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                // Hash entire JSON object
                                let s = serde_json::to_string(item).unwrap_or_default();
                                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                s.hash(&mut hasher);
                                format!("{:x}", hasher.finish())
                            };

                            if key.is_empty() {
                                continue;
                            }

                            if seen_keys.insert(key) {
                                accumulated.push(item.clone());
                                new_count += 1;
                            } else {
                                total_duplicates += 1;
                            }
                        }
                    }
                }
            }

            tracing::info!("[extract scroll] iteration {}/{}: +{} new, {} total, {} dupes",
                i + 1, req.scroll_max, new_count, accumulated.len(), total_duplicates);

            // Early exit: 3 consecutive rounds with 0 new items
            if new_count == 0 {
                no_new_streak += 1;
                if no_new_streak >= 3 {
                    tracing::info!("[extract scroll] stopping: 3 consecutive rounds with no new items");
                    break;
                }
            } else {
                no_new_streak = 0;
            }

            // Check limit
            if let Some(lim) = limit {
                if accumulated.len() >= lim {
                    accumulated.truncate(lim);
                    break;
                }
            }

            // Don't scroll after last iteration
            if i + 1 < req.scroll_max {
                let scroll_script = format!(
                    "window.scrollBy(0, {}); JSON.stringify({{ scrolled: true }});",
                    scroll_amount
                );
                let _ = execute_script_in(&req.session, scroll_script, state, 5000, frame).await;
                tokio::time::sleep(Duration::from_millis(scroll_delay)).await;
            }
        }

        // Return accumulated results
        let count = accumulated.len();
        let data = serde_json::Value::Array(accumulated);
        let summary = format!("Extracted {} unique items ({} scrolls, {} duplicates removed)",
            count, req.scroll_max, total_duplicates);

        return McpToolResponse::success_text(
            serde_json::to_string(&serde_json::json!({
                "count": count,
                "data": data,
                "scroll_mode": true,
                "duplicates_removed": total_duplicates,
                "summary": summary,
            })).unwrap_or_else(|_| "{}".to_string())
        );
    }

    // Non-scroll mode: single extraction
    let extract_script = generate_extract_data_script(&req.selector, &req.fields, req.limit);
    let result = execute_script_in(&req.session, extract_script, state, 10000, frame).await
        .map_err(|e| format!("Extract failed: {}", e));

    match result {
        Ok(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json)
                .unwrap_or(serde_json::json!({"error": "Parse failed"}));

            let count = parsed["count"].as_u64().unwrap_or(0);
            let data = parsed["data"].clone();
            
            // If still 0 items, provide diagnostic info
            if count == 0 {
                // Try to get page info for debugging
                let diag_script = format!(r#"
                    JSON.stringify({{
                        readyState: document.readyState,
                        bodyChildCount: document.body ? document.body.children.length : 0,
                        totalElements: document.querySelectorAll('*').length,
                        matchingSelector: document.querySelectorAll('{}').length,
                        url: window.location.href
                    }})
                "#, req.selector.replace('"', "\\\""));
                let diag = execute_script_in(&req.session, diag_script, state, 3000, frame).await
                    .unwrap_or_else(|_| "diagnostic unavailable".to_string());
                
                tracing::warn!("[extract] 0 items extracted. Diagnostics: {}", diag);
                
                McpToolResponse::success_text(format!(
                    "Extracted 0 items (selector: '{}')\nDiagnostics: {}\nHint: The page may need more time to load dynamic content. Try using interact with wait{{condition:'element',value:'{}'}} before extract.",
                    req.selector,
                    diag,
                    req.selector
                ))
            } else {
                // Check if all fields are null (selector mismatch diagnostic)
                let diag = &parsed["_diagnostic"];
                if !diag.is_null() {
                    let diag_pretty = serde_json::to_string_pretty(diag).unwrap_or_default();
                    McpToolResponse::success_text(format!(
                        "Extracted {} items but ALL fields are null — the field selectors don't match any child elements inside '{}'.\n\nDiagnostic (first container):\n{}\n\nHint: Check the child_structure and inner_html_sample above to find the correct CSS selectors for your fields.",
                        count,
                        req.selector,
                        diag_pretty
                    ))
                } else {
                    McpToolResponse::success_text(format!(
                        "Extracted {} items:\n{}",
                        count,
                        serde_json::to_string_pretty(&data).unwrap_or_default()
                    ))
                }
            }
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
                let sessions: Vec<serde_json::Value> = response.sessions.iter()
                    .map(|s| serde_json::json!({
                        "name": s.name,
                        "profile": s.profile,
                        "status": format!("{:?}", s.auth_status),
                        "last_accessed": s.last_accessed,
                        "active": s.active,
                        "acquired": s.acquired,
                        "expired": s.expired,
                        "ttl_hours": s.ttl_hours,
                        "expires_at": s.expires_at
                    }))
                    .collect();
                return McpToolResponse::success_json(serde_json::json!({
                    "sessions": sessions,
                    "count": sessions.len()
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
            ttl_hours: req.ttl_hours,  // 168 = 1 week (default), 0 = no expiration
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
                                
                                // Apply device simulation if requested
                                let mut device_info: Option<String> = None;
                                
                                // Option 1: Device preset (e.g., "iPhone 14")
                                if let Some(device_name) = &req.device {
                                    let (dev_tx, dev_rx) = oneshot::channel();
                                    let dev_cmd = crate::core::AppCommand::SimulateDevice {
                                        id: handle.id.clone(),
                                        device_name: device_name.clone(),
                                        resp_tx: dev_tx,
                                    };
                                    if state.cmd_tx.send(dev_cmd).is_ok() {
                                        match tokio::time::timeout(Duration::from_millis(2000), dev_rx).await {
                                            Ok(Ok(Ok(msg))) => {
                                                device_info = Some(msg);
                                            }
                                            Ok(Ok(Err(e))) => {
                                                tracing::warn!("Device simulation failed: {}", e);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                // Option 2: Custom viewport dimensions
                                else if let (Some(w), Some(h)) = (req.viewport_width, req.viewport_height) {
                                    let (vp_tx, vp_rx) = oneshot::channel();
                                    let vp_cmd = crate::core::AppCommand::SetViewport {
                                        id: handle.id.clone(),
                                        width: w,
                                        height: h,
                                        resp_tx: vp_tx,
                                    };
                                    if state.cmd_tx.send(vp_cmd).is_ok() {
                                        match tokio::time::timeout(Duration::from_millis(2000), vp_rx).await {
                                            Ok(Ok(Ok(_))) => {
                                                device_info = Some(format!("Viewport set to {}x{}", w, h));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                
                                // Option 3: Custom user agent
                                if let Some(ua) = &req.user_agent {
                                    let (ua_tx, ua_rx) = oneshot::channel();
                                    let ua_cmd = crate::core::AppCommand::SetUserAgent {
                                        id: handle.id.clone(),
                                        user_agent: ua.clone(),
                                        resp_tx: ua_tx,
                                    };
                                    if state.cmd_tx.send(ua_cmd).is_ok() {
                                        let _ = tokio::time::timeout(Duration::from_millis(1000), ua_rx).await;
                                    }
                                }
                                
                                return McpToolResponse::success_json(serde_json::json!({
                                    "session": response.session,
                                    "is_new": response.is_new,
                                    "profile": response.profile,
                                    "wait_ms": waited_ms,
                                    "status": "ready",
                                    "visible": !req.headless,
                                    "device": device_info
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

    // Clone session (copy profile/cookies to a new session name)
    if let Some(new_name) = &req.clone_to {
        let source = req.session.as_deref()
            .or(req.acquire.as_deref())
            .unwrap_or("default");
        match manager.clone_session(source, new_name) {
            Ok(()) => return McpToolResponse::success_json(serde_json::json!({
                "source": source,
                "new_session": new_name,
                "status": "cloned",
                "message": format!("Session '{}' cloned to '{}'. The new session has the same cookies and profile. Use acquire to start using it.", source, new_name)
            })),
            Err(e) => return McpToolResponse::error("SESSION_CLONE_FAILED", &e),
        }
    }

    if let Some(name) = &req.import {
        use crate::core::cookie_import::{BrowserType, get_cookie_db_path, read_firefox_cookies, summarize_cookies};

        // Determine browser type
        let browser_str = req.browser.as_deref().unwrap_or("firefox");
        let browser = match browser_str.to_lowercase().as_str() {
            "chrome" => BrowserType::Chrome,
            "edge" => BrowserType::Edge,
            "firefox" => BrowserType::Firefox,
            _ => return McpToolResponse::error("INVALID_BROWSER", &format!("Unknown browser: {}. Use 'chrome', 'edge', or 'firefox'", browser_str)),
        };

        // Chrome/Edge cookies are DPAPI-encrypted — not supported yet
        if matches!(browser, BrowserType::Chrome | BrowserType::Edge) {
            return McpToolResponse::error("UNSUPPORTED_BROWSER",
                "Chrome/Edge cookie import requires DPAPI decryption (not yet implemented). Use browser='firefox' instead.");
        }

        // Find cookie database
        let db_path = match get_cookie_db_path(browser, "Default") {
            Some(p) if p.exists() => p,
            Some(p) => return McpToolResponse::error("COOKIE_DB_NOT_FOUND", &format!("Cookie database not found at: {}", p.display())),
            None => return McpToolResponse::error("COOKIE_DB_NOT_FOUND", "Could not determine Firefox cookie database path"),
        };

        // Read cookies
        let domains = req.domains.clone().unwrap_or_default();
        let cookies = match read_firefox_cookies(&db_path, &domains) {
            Ok(c) => c,
            Err(e) => return McpToolResponse::error("COOKIE_READ_FAILED", &format!("Failed to read cookies: {}", e)),
        };

        if cookies.is_empty() {
            return McpToolResponse::error("NO_COOKIES_FOUND", &format!(
                "No cookies found for domains: {:?}. Make sure Firefox has cookies for these domains.", domains
            ));
        }

        // Verify session exists
        let manager = get_session_manager_v2();
        if manager.get_handle(name).is_none() {
            return McpToolResponse::error("SESSION_NOT_FOUND", &format!("Session '{}' not found. Acquire it first.", name));
        }

        // Convert and set cookies on the WebView session
        let cdp_json = imported_cookies_to_cdp_json(&cookies);
        match set_cookies_cdp(name, cdp_json, state).await {
            Ok(()) => {
                let domain_counts = summarize_cookies(&cookies);
                return McpToolResponse::success_json(serde_json::json!({
                    "session": name,
                    "status": "cookies_imported",
                    "imported_count": cookies.len(),
                    "browser": browser_str,
                    "domain_counts": domain_counts,
                }));
            }
            Err(e) => return McpToolResponse::error("COOKIE_SET_FAILED", &format!("Cookies read OK but failed to set on session: {}", e)),
        }
    }
    
    // AI Status - show current AI configuration
    if req.ai_status {
        let config = crate::core::config::get_config();
        let ai_config = crate::core::ai::AiConfig::default();
        
        return McpToolResponse::success_json(serde_json::json!({
            "provider": config.ai.provider,
            "model": config.ai.model,
            "enabled": config.ai.enabled,
            "available": ai_config.is_available(),
            "has_api_key": config.ai.api_key.is_some(),
        }));
    }
    
    // AI Models - list available models (Ollama only)
    if req.ai_models {
        let config = crate::core::config::get_config();
        
        if config.ai.provider.to_lowercase() == "ollama" {
            let ollama = crate::core::ai::OllamaClient::new(&crate::core::ai::AiConfig::default());
            
            match ollama.list_models() {
                Ok(models) => {
                    return McpToolResponse::success_json(serde_json::json!({
                        "provider": "ollama",
                        "available": ollama.is_available(),
                        "models": models,
                        "current_model": config.ai.model
                    }));
                }
                Err(e) => {
                    return McpToolResponse::error("OLLAMA_MODELS_FAILED", &e);
                }
            }
        } else {
            // Gemini doesn't have a model list API, return known models
            return McpToolResponse::success_json(serde_json::json!({
                "provider": "gemini",
                "models": [
                    "gemini-2.0-flash",
                    "gemini-1.5-flash",
                    "gemini-1.5-pro",
                    "gemini-pro-vision"
                ],
                "current_model": config.ai.model
            }));
        }
    }
    
    // AI Config update
    if let Some(update) = &req.ai_config {
        let mut config = crate::core::config::get_config().clone();
        
        if let Some(provider) = &update.provider {
            config.ai.provider = provider.clone();
        }
        if let Some(model) = &update.model {
            config.ai.model = model.clone();
        }
        if let Some(api_key) = &update.api_key {
            config.ai.api_key = Some(api_key.clone());
        }
        if let Some(enabled) = update.enabled {
            config.ai.enabled = enabled;
        }
        
        // Save updated config
        match config.save() {
            Ok(_) => {
                return McpToolResponse::success_json(serde_json::json!({
                    "status": "config_updated",
                    "provider": config.ai.provider,
                    "model": config.ai.model,
                    "enabled": config.ai.enabled
                }));
            }
            Err(e) => {
                return McpToolResponse::error("CONFIG_SAVE_FAILED", &e);
            }
        }
    }
    
    // Device switch on existing session (no acquire needed)
    // This allows switching devices on an already active session
    if req.device.is_some() || req.viewport_width.is_some() || req.user_agent.is_some() {
        // Get session name from 'session' or 'acquire' field, or default
        let session_name = req.session.as_deref()
            .or(req.acquire.as_deref())
            .unwrap_or("default");
        
        // Check if session exists
        if let Some(handle) = manager.get_handle(session_name) {
            let mut device_result: Option<String> = None;
            let mut viewport_result: Option<String> = None;
            let mut ua_result: Option<String> = None;
            
            // Apply device preset
            if let Some(device_name) = &req.device {
                let (dev_tx, dev_rx) = oneshot::channel();
                let dev_cmd = crate::core::AppCommand::SimulateDevice {
                    id: handle.id.clone(),
                    device_name: device_name.clone(),
                    resp_tx: dev_tx,
                };
                if state.cmd_tx.send(dev_cmd).is_ok() {
                    match tokio::time::timeout(Duration::from_millis(3000), dev_rx).await {
                        Ok(Ok(Ok(msg))) => {
                            device_result = Some(msg);
                        }
                        Ok(Ok(Err(e))) => {
                            return McpToolResponse::error("DEVICE_SIMULATION_FAILED", &e);
                        }
                        _ => {
                            return McpToolResponse::error("DEVICE_SIMULATION_TIMEOUT", "Device simulation timed out");
                        }
                    }
                }
            }
            // Or apply custom viewport
            else if let (Some(w), Some(h)) = (req.viewport_width, req.viewport_height) {
                let (vp_tx, vp_rx) = oneshot::channel();
                let vp_cmd = crate::core::AppCommand::SetViewport {
                    id: handle.id.clone(),
                    width: w,
                    height: h,
                    resp_tx: vp_tx,
                };
                if state.cmd_tx.send(vp_cmd).is_ok() {
                    match tokio::time::timeout(Duration::from_millis(2000), vp_rx).await {
                        Ok(Ok(Ok(_))) => {
                            viewport_result = Some(format!("{}x{}", w, h));
                        }
                        _ => {}
                    }
                }
            }
            
            // Apply custom user agent
            if let Some(ua) = &req.user_agent {
                let (ua_tx, ua_rx) = oneshot::channel();
                let ua_cmd = crate::core::AppCommand::SetUserAgent {
                    id: handle.id.clone(),
                    user_agent: ua.clone(),
                    resp_tx: ua_tx,
                };
                if state.cmd_tx.send(ua_cmd).is_ok() {
                    if tokio::time::timeout(Duration::from_millis(1000), ua_rx).await.is_ok() {
                        ua_result = Some("User agent updated".to_string());
                    }
                }
            }
            
            return McpToolResponse::success_json(serde_json::json!({
                "session": session_name,
                "status": "device_switched",
                "device": device_result,
                "viewport": viewport_result,
                "user_agent": ua_result
            }));
        } else {
            return McpToolResponse::error("SESSION_NOT_FOUND", &format!("Session '{}' not found. Use acquire to create it first.", session_name));
        }
    }
    
    McpToolResponse::error("INVALID_SESSION_REQUEST", "No valid session action specified")
}

// ============================================================================
// 6. Media
// ============================================================================

async fn handle_media(req: MediaRequest, state: &V2AppState) -> McpToolResponse {
    match req.action {
        MediaAction::YoutubeDownload { url, quality, audio_only, output_dir } => {
            // Generate output directory
            let output_path = output_dir.unwrap_or_else(|| {
                crate::core::config::AppConfig::data_dir()
                    .join("downloads")
                    .to_string_lossy()
                    .to_string()
            });
            
            // Create output directory
            if let Err(e) = std::fs::create_dir_all(&output_path) {
                return McpToolResponse::error("DIR_CREATE_FAILED", &e.to_string());
            }
            
            // Build yt-dlp command
            let quality_str = quality.as_deref().unwrap_or("best");
            let format_arg = match quality_str {
                "best" => "bestvideo+bestaudio/best",
                "hd" | "1080p" => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
                "sd" | "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]",
                _ => "bestvideo+bestaudio/best",
            };
            
            let mut args = vec![
                url.clone(),
                "-f".to_string(), if audio_only { "bestaudio".to_string() } else { format_arg.to_string() },
                "-o".to_string(), format!("{}\\%(title)s.%(ext)s", output_path),
                "--embed-metadata".to_string(),
                "--no-playlist".to_string(),
            ];
            
            if audio_only {
                args.push("-x".to_string());
                args.push("--audio-format".to_string());
                args.push("mp3".to_string());
            }
            
            // Execute yt-dlp
            match std::process::Command::new("yt-dlp")
                .args(&args)
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        McpToolResponse::success_json(serde_json::json!({
                            "success": true,
                            "url": url,
                            "output_dir": output_path,
                            "audio_only": audio_only,
                            "message": String::from_utf8_lossy(&output.stdout).trim()
                        }))
                    } else {
                        McpToolResponse::error("YTDLP_FAILED", &String::from_utf8_lossy(&output.stderr))
                    }
                }
                Err(e) => McpToolResponse::error("YTDLP_NOT_FOUND", &format!("yt-dlp command failed: {}. Make sure yt-dlp is installed.", e))
            }
        }
        MediaAction::YoutubeSubtitles { url, language, format } => {
            let lang = language.as_deref().unwrap_or("ja,en");
            let fmt = format.as_deref().unwrap_or("json3");
            
            // Create temp dir for subtitles
            let output_path = crate::core::config::AppConfig::data_dir()
                .join("subtitles");
            let _ = std::fs::create_dir_all(&output_path);
            
            // Build yt-dlp command for subtitle extraction
            let args = vec![
                url.clone(),
                "--write-sub".to_string(),
                "--write-auto-sub".to_string(),
                "--sub-lang".to_string(), lang.to_string(),
                "--sub-format".to_string(), fmt.to_string(),
                "--skip-download".to_string(),
                "-o".to_string(), format!("{}\\%(title)s", output_path.to_string_lossy()),
                "--print".to_string(), "%(title)s".to_string(),
            ];
            
            // Execute yt-dlp
            match std::process::Command::new("yt-dlp")
                .args(&args)
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        
                        // Try to read the subtitle file
                        let subtitle_file = output_path.join(format!("{}.{}.{}", title, lang.split(',').next().unwrap_or("en"), fmt));
                        let content = std::fs::read_to_string(&subtitle_file).ok();
                        
                        McpToolResponse::success_json(serde_json::json!({
                            "success": true,
                            "url": url,
                            "title": title,
                            "language": lang,
                            "format": fmt,
                            "file": subtitle_file.to_string_lossy(),
                            "content": content
                        }))
                    } else {
                        McpToolResponse::error("YTDLP_SUBTITLES_FAILED", &String::from_utf8_lossy(&output.stderr))
                    }
                }
                Err(e) => McpToolResponse::error("YTDLP_NOT_FOUND", &format!("yt-dlp command failed: {}. Make sure yt-dlp is installed.", e))
            }
        }
        MediaAction::VideoAnalyze { url, keyframes, audio, max_frames } => {
            // Create output directory
            let output_path = crate::core::config::AppConfig::data_dir()
                .join("analysis");
            let _ = std::fs::create_dir_all(&output_path);
            
            let mut results = serde_json::json!({
                "success": true,
                "source": url,
            });
            
            // Get video metadata using ffprobe
            match std::process::Command::new("ffprobe")
                .args(&[
                    "-v", "quiet",
                    "-print_format", "json",
                    "-show_format",
                    "-show_streams",
                    &url
                ])
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        if let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                            results["metadata"] = metadata;
                        }
                    }
                }
                Err(e) => {
                    results["metadata_error"] = serde_json::json!(e.to_string());
                }
            }
            
            // Extract keyframes if requested
            if keyframes {
                let max = max_frames.unwrap_or(10);
                let output_pattern = output_path.join("keyframe_%04d.jpg");
                
                let keyframe_result = std::process::Command::new("ffmpeg")
                    .args(&[
                        "-i", &url,
                        "-vf", &format!("select='eq(pict_type,I)',scale=320:-1"),
                        "-vsync", "vfr",
                        "-frames:v", &max.to_string(),
                        "-q:v", "5",
                        "-y",
                        &output_pattern.to_string_lossy()
                    ])
                    .output();
                
                match keyframe_result {
                    Ok(output) => {
                        if output.status.success() {
                            // List extracted frames
                            let frames: Vec<String> = (1..=max)
                                .map(|i| format!("keyframe_{:04}.jpg", i))
                                .filter(|f| output_path.join(f).exists())
                                .collect();
                            results["keyframes"] = serde_json::json!({
                                "count": frames.len(),
                                "directory": output_path.to_string_lossy(),
                                "files": frames
                            });
                        } else {
                            results["keyframes_error"] = serde_json::json!(String::from_utf8_lossy(&output.stderr));
                        }
                    }
                    Err(e) => {
                        results["keyframes_error"] = serde_json::json!(format!("ffmpeg not found: {}", e));
                    }
                }
            }
            
            // Extract audio if requested
            if audio {
                let audio_output = output_path.join("audio.mp3");
                
                let audio_result = std::process::Command::new("ffmpeg")
                    .args(&[
                        "-i", &url,
                        "-vn",
                        "-acodec", "libmp3lame",
                        "-ab", "192k",
                        "-y",
                        &audio_output.to_string_lossy()
                    ])
                    .output();
                
                match audio_result {
                    Ok(output) => {
                        if output.status.success() {
                            results["audio"] = serde_json::json!({
                                "file": audio_output.to_string_lossy(),
                                "format": "mp3",
                                "bitrate": "192k"
                            });
                        } else {
                            results["audio_error"] = serde_json::json!(String::from_utf8_lossy(&output.stderr));
                        }
                    }
                    Err(e) => {
                        results["audio_error"] = serde_json::json!(format!("ffmpeg not found: {}", e));
                    }
                }
            }
            
            McpToolResponse::success_json(results)
        }
        MediaAction::CollectImages { selector, min_width, min_height, download: _, max_images } => {
            let script = generate_collect_images_script(
                selector.as_deref(),
                min_width.unwrap_or(100),
                min_height.unwrap_or(100),
                max_images.unwrap_or(50)
            );
            
            match execute_script(&req.session, script, state, 30000).await {
                Ok(result) => {
                    match serde_json::from_str::<serde_json::Value>(&result) {
                        Ok(data) => McpToolResponse::success_json(data),
                        Err(_) => McpToolResponse::success_text(result),
                    }
                }
                Err(e) => McpToolResponse::error("COLLECT_IMAGES_FAILED", &e),
            }
        }
        MediaAction::Upload { data, filename, mime_type: _ } => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            use crate::core::upload;

            let bytes = match STANDARD.decode(data.trim()) {
                Ok(b) => b,
                Err(e) => return McpToolResponse::error("UPLOAD_DECODE_ERROR", &format!("Base64 decode error: {e}")),
            };

            if bytes.len() as u64 > upload::MAX_UPLOAD_SIZE {
                return McpToolResponse::error("UPLOAD_TOO_LARGE", &format!("File too large: {} bytes (max {})", bytes.len(), upload::MAX_UPLOAD_SIZE));
            }

            let dir = upload::uploads_dir(&req.session);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return McpToolResponse::error("UPLOAD_DIR_ERROR", &format!("Failed to create upload dir: {e}"));
            }

            let stored = upload::sanitize_and_store_filename(&filename);
            let file_path = dir.join(&stored);
            if let Err(e) = std::fs::write(&file_path, &bytes) {
                return McpToolResponse::error("UPLOAD_WRITE_ERROR", &format!("File write error: {e}"));
            }

            let url = format!("/uploads/{}/{}", req.session, stored);
            McpToolResponse::success_json(serde_json::json!({
                "success": true,
                "filename": filename,
                "stored_filename": stored,
                "size": bytes.len(),
                "url": url,
                "file_path": file_path.to_string_lossy(),
            }))
        }
        MediaAction::InjectFile { selector, file, frame } => {
            use crate::core::upload;

            // Resolve file path
            let file_path = if file.starts_with("/uploads/") {
                let parts: Vec<&str> = file.trim_start_matches("/uploads/").splitn(2, '/').collect();
                if parts.len() != 2 {
                    return McpToolResponse::error("INVALID_FILE_URL", "Invalid upload URL format");
                }
                upload::uploads_dir(parts[0]).join(parts[1]).to_string_lossy().to_string()
            } else {
                file.clone()
            };

            let win_path = file_path.replace('/', "\\");

            // Send CDP command via AppCommand
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let cmd = crate::core::AppCommand::FormInjectFile {
                id: req.session.clone(),
                selector: selector.clone(),
                file_paths: vec![win_path],
                frame,
                resp_tx,
            };

            if state.cmd_tx.send(cmd).is_err() {
                return McpToolResponse::error("SESSION_COMMAND_ERROR", "Failed to send command");
            }

            match tokio::time::timeout(std::time::Duration::from_secs(10), resp_rx).await {
                Ok(Ok(Ok(result))) => McpToolResponse::success_json(serde_json::json!({
                    "success": true,
                    "selector": selector,
                    "file": file_path,
                    "method": "cdp_dom_set_file_input_files",
                    "result": result,
                })),
                Ok(Ok(Err(e))) => McpToolResponse::error("INJECT_FAILED", &e),
                Ok(Err(_)) => McpToolResponse::error("INJECT_TIMEOUT", "Channel closed"),
                Err(_) => McpToolResponse::error("INJECT_TIMEOUT", "Timed out waiting for file injection"),
            }
        }
        MediaAction::ListUploads => {
            use crate::core::upload;

            let dir = upload::uploads_dir(&req.session);
            let mut files = Vec::new();

            if dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if let Ok(meta) = entry.metadata() {
                            if meta.is_file() {
                                let fname = entry.file_name().to_string_lossy().to_string();
                                files.push(serde_json::json!({
                                    "filename": fname,
                                    "size": meta.len(),
                                    "url": format!("/uploads/{}/{}", req.session, fname),
                                }));
                            }
                        }
                    }
                }
            }

            McpToolResponse::success_json(serde_json::json!({
                "session": req.session,
                "files": files,
                "count": files.len(),
            }))
        }
    }
}

// ============================================================================
// 8. Agent (Agentic Mode - Goal-based Browser Automation)
// ============================================================================

async fn handle_agent(req: AgentRequest, state: &V2AppState) -> McpToolResponse {
    use crate::mcp_v3::types::AgentAction;
    
    let manager = get_session_manager_v2();
    if manager.get_handle(&req.session).is_none() {
        return McpToolResponse::error("SESSION_NOT_FOUND", &format!("Session '{}' not found", req.session));
    }
    
    match req.action {
        AgentAction::Start { goal, context, max_steps, system_prompt, human_mode, instant_type } => {
            let max_steps = max_steps.unwrap_or(5);
            let context_text = context.unwrap_or_default();
            let custom_prompt = system_prompt.unwrap_or_default();
            
            tracing::info!("[agent] Starting goal: {}, human_mode: {}, instant_type: {}", 
                goal, human_mode, instant_type);
            
            // Agent loop
            let mut steps = Vec::new();
            let mut completed = false;
            let mut final_result = String::new();
            
            for step_num in 1..=max_steps {
                // 1. Capture current page state
                let capture_script = r#"
                    JSON.stringify({
                        url: window.location.href,
                        title: document.title,
                        text: document.body.innerText.substring(0, 3000),
                        elements: Array.from(document.querySelectorAll('a, button, input, select, textarea'))
                            .slice(0, 30)
                            .map(el => ({
                                tag: el.tagName.toLowerCase(),
                                text: (el.textContent || el.placeholder || el.value || '').substring(0, 50).trim(),
                                type: el.type || null,
                                href: el.href || null,
                                selector: el.id ? '#' + el.id : (el.className ? '.' + el.className.split(' ')[0] : el.tagName.toLowerCase())
                            }))
                            .filter(e => e.text || e.type === 'text')
                    });
                "#;
                
                let page_state = match execute_script(&req.session, capture_script.to_string(), state, 5000).await {
                    Ok(s) => s,
                    Err(e) => {
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "capture",
                            "error": e
                        }));
                        break;
                    }
                };
                
                let page_data: serde_json::Value = serde_json::from_str(&page_state)
                    .unwrap_or(serde_json::json!({"error": "parse failed"}));
                
                // 2. Ask AI for next action
                let config = crate::core::config::get_config();
                let ai_config = crate::core::ai::AiConfig {
                    enabled: config.ai.enabled,
                    provider: config.ai.provider.clone(),
                    model: config.ai.model.clone(),
                    api_key: config.ai.api_key.clone(),
                    timeout_ms: config.ai.timeout_ms,
                    daily_budget_usd: config.ai.daily_budget_usd,
                    daily_usage_usd: 0.0,
                };
                
                // Build history of past actions for this session
                let history_text = if steps.is_empty() {
                    "(first action)".to_string()
                } else {
                    steps.iter().map(|s| {
                        let action = s["action"].as_str().unwrap_or("?");
                        let result = s.get("result").map(|r| r.to_string()).unwrap_or_default();
                        let success = result.contains("success");
                        format!("Step {}: {} - {}", 
                            s["step"].as_u64().unwrap_or(0),
                            action,
                            if success { "OK" } else { "FAIL" }
                        )
                    }).collect::<Vec<_>>().join("\n")
                };
                
                let ai_prompt = format!(r#"You are a browser automation agent.
{}
[GOAL]
{}

[CONTEXT]
{}

[ACTION HISTORY]
{}

[CURRENT PAGE STATE]
URL: {}
Title: {}
Page text (excerpt): {}

[INTERACTIVE ELEMENTS]
{}

[RULES]
- Do NOT repeat the same action
- After typing, use submit:true to press Enter OR click the search button
- If the page changed, evaluate if goal is achieved
- Search results page = goal achieved

[INSTRUCTION]
Decide the next action. Respond with JSON only.

Response formats:
- Goal achieved: {{"done": true, "result": "description"}}
- Click: {{"action": "click", "selector": "CSS_SELECTOR", "reason": "why"}}
- Type: {{"action": "type", "selector": "CSS_SELECTOR", "value": "TEXT", "reason": "why"}}
- Type+Enter: {{"action": "type", "selector": "CSS_SELECTOR", "value": "TEXT", "submit": true, "reason": "why"}}
- Navigate: {{"action": "navigate", "url": "URL", "reason": "why"}}
- Failed: {{"failed": true, "reason": "why"}}

Output JSON only, no explanation."#,
                    if custom_prompt.is_empty() { String::new() } else { format!("\n[CUSTOM INSTRUCTIONS]\n{}\n", custom_prompt) },
                    goal,
                    context_text,
                    history_text,
                    page_data["url"].as_str().unwrap_or("unknown"),
                    page_data["title"].as_str().unwrap_or("unknown"),
                    page_data["text"].as_str().unwrap_or("").chars().take(1000).collect::<String>(),
                    serde_json::to_string_pretty(&page_data["elements"]).unwrap_or_default()
                );
                
                let ai_response = if config.ai.provider.to_lowercase() == "ollama" {
                    let ollama = crate::core::ai::OllamaClient::new(&ai_config);
                    ollama.call(&ai_prompt, None)
                } else {
                    match crate::core::ai::AiClient::new(&ai_config) {
                        Some(client) => client.call(&ai_prompt, None),
                        None => Err("AI not configured".to_string()),
                    }
                };
                
                let ai_text = match ai_response {
                    Ok(t) => t,
                    Err(e) => {
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "ai_decision",
                            "error": e
                        }));
                        break;
                    }
                };
                
                // Extract JSON from AI response
                let ai_json: serde_json::Value = {
                    // Try to find JSON in the response
                    let json_start = ai_text.find('{');
                    let json_end = ai_text.rfind('}');
                    
                    match (json_start, json_end) {
                        (Some(start), Some(end)) if end > start => {
                            serde_json::from_str(&ai_text[start..=end])
                                .unwrap_or(serde_json::json!({"failed": true, "reason": "Invalid JSON from AI"}))
                        }
                        _ => serde_json::json!({"failed": true, "reason": "No JSON found in AI response"})
                    }
                };
                
                // 3. Execute the action
                if ai_json.get("done").and_then(|v| v.as_bool()) == Some(true) {
                    completed = true;
                    final_result = ai_json["result"].as_str().unwrap_or("完了").to_string();
                    steps.push(serde_json::json!({
                        "step": step_num,
                        "action": "done",
                        "result": final_result
                    }));
                    break;
                }
                
                if ai_json.get("failed").and_then(|v| v.as_bool()) == Some(true) {
                    final_result = ai_json["reason"].as_str().unwrap_or("失敗").to_string();
                    steps.push(serde_json::json!({
                        "step": step_num,
                        "action": "failed",
                        "reason": final_result
                    }));
                    break;
                }
                
                let action_name = ai_json["action"].as_str().unwrap_or("unknown");
                match action_name {
                    "click" => {
                        let selector = ai_json["selector"].as_str().unwrap_or("").to_string();
                        tracing::info!("[agent] Executing click on: {}", selector);
                        
                        let click_action = Action::Click { 
                            target: selector.clone(), 
                            wait_after_ms: Some(500) 
                        };
                        
                        let result = execute_action(&req.session, &click_action, 10000, state, human_mode, None).await;
                        
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "click",
                            "selector": selector,
                            "reason": ai_json["reason"],
                            "human_mode": human_mode,
                            "result": result.as_ref().map(|_| "success").unwrap_or("failed"),
                            "error": result.as_ref().err()
                        }));
                        
                        if result.is_err() {
                            tracing::warn!("[agent] Click failed: {:?}", result.err());
                        }
                        
                        // Wait for page update
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                    "type" => {
                        let selector = ai_json["selector"].as_str().unwrap_or("").to_string();
                        let value = ai_json["value"].as_str().unwrap_or("").to_string();
                        let should_submit = ai_json["submit"].as_bool().unwrap_or(false);
                        
                        // Calculate timeout based on mode and text length
                        let type_timeout = if instant_type {
                            10000_u64
                        } else if human_mode {
                            // Human mode: 150ms per char + 3s buffer for typos/pauses
                            10000_u64.max(5000 + (value.len() as u64 * 150) + 3000)
                        } else {
                            10000_u64.max(5000 + (value.len() as u64 * 20))
                        };
                        
                        tracing::info!("[agent] Executing type on: {}, value: {}, submit: {}, instant: {}, timeout: {}ms", 
                            selector, value, should_submit, instant_type, type_timeout);
                        
                        // Use execute_action with instant mode for autocomplete-heavy sites
                        let type_action = Action::Type { 
                            target: selector.clone(), 
                            value: value.clone(),
                            clear: true,
                            instant: instant_type,
                        };
                        
                        let result = execute_action(&req.session, &type_action, type_timeout, state, human_mode, None).await;
                        
                        // If submit requested, press Enter
                        let submit_result = if should_submit && result.is_ok() {
                            let enter_script = format!(r#"
                                (function() {{
                                    const el = document.querySelector('{}');
                                    if (el) {{
                                        el.dispatchEvent(new KeyboardEvent('keydown', {{key: 'Enter', keyCode: 13, bubbles: true}}));
                                        if (el.form) el.form.submit();
                                        return JSON.stringify({{success: true}});
                                    }}
                                    return JSON.stringify({{success: false}});
                                }})();
                            "#, selector.replace('\'', "\\'"));
                            execute_script(&req.session, enter_script, state, 3000).await.ok()
                        } else {
                            None
                        };
                        
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "type",
                            "selector": selector,
                            "value": value,
                            "submit": should_submit,
                            "instant": instant_type,
                            "human_mode": human_mode,
                            "reason": ai_json["reason"],
                            "result": result.as_ref().map(|_| "success").unwrap_or("failed"),
                            "submit_result": submit_result,
                            "error": result.as_ref().err()
                        }));
                        
                        if result.is_err() {
                            tracing::warn!("[agent] Type failed: {:?}", result.err());
                        }
                        
                        // Wait for response if submitted
                        if should_submit {
                            tokio::time::sleep(Duration::from_millis(2000)).await;
                        }
                    }
                    "navigate" => {
                        let url = ai_json["url"].as_str().unwrap_or("");
                        let nav_script = format!("window.location.href = '{}'; JSON.stringify({{success: true}});", 
                            url.replace('\'', "\\'"));
                        
                        let result = execute_script(&req.session, nav_script, state, 5000).await;
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "navigate",
                            "url": url,
                            "reason": ai_json["reason"],
                            "result": result.unwrap_or_else(|e| e)
                        }));
                        
                        // Wait for navigation
                        tokio::time::sleep(Duration::from_millis(2000)).await;
                    }
                    _ => {
                        steps.push(serde_json::json!({
                            "step": step_num,
                            "action": "unknown",
                            "ai_response": ai_json
                        }));
                    }
                }
            }
            
            McpToolResponse::success_json(serde_json::json!({
                "goal": goal,
                "completed": completed,
                "result": final_result,
                "steps_taken": steps.len(),
                "max_steps": max_steps,
                "steps": steps
            }))
        }
        AgentAction::Resume => {
            // TODO: Implement session-based state recovery
            McpToolResponse::error("NOT_IMPLEMENTED", "Resume not yet implemented")
        }
        AgentAction::Status => {
            // TODO: Implement status tracking
            McpToolResponse::error("NOT_IMPLEMENTED", "Status not yet implemented")
        }
        AgentAction::Cancel => {
            // TODO: Implement cancellation
            McpToolResponse::error("NOT_IMPLEMENTED", "Cancel not yet implemented")
        }
    }
}

fn generate_collect_images_script(selector: Option<&str>, min_width: u32, min_height: u32, max_images: usize) -> String {
    let selector_code = selector
        .map(|s| format!("document.querySelectorAll('{}')", s.replace('\'', "\\'")))
        .unwrap_or_else(|| "document.querySelectorAll('img')".to_string());
    
    format!(r#"
(function() {{
    const minWidth = {};
    const minHeight = {};
    const maxImages = {};
    
    const images = Array.from({})
        .filter(img => {{
            // Check natural dimensions
            if (img.naturalWidth < minWidth || img.naturalHeight < minHeight) return false;
            
            // Check display dimensions
            const rect = img.getBoundingClientRect();
            if (rect.width < minWidth || rect.height < minHeight) return false;
            
            // Skip data URLs and tiny images
            if (!img.src || img.src.startsWith('data:')) return false;
            
            // Skip common tracking/placeholder patterns
            if (img.src.includes('pixel') || img.src.includes('spacer') || img.src.includes('blank')) return false;
            
            return true;
        }})
        .slice(0, maxImages)
        .map(img => ({{
            src: img.src,
            alt: img.alt || null,
            title: img.title || null,
            width: img.naturalWidth,
            height: img.naturalHeight,
            displayWidth: Math.round(img.getBoundingClientRect().width),
            displayHeight: Math.round(img.getBoundingClientRect().height)
        }}));
    
    return JSON.stringify({{
        success: true,
        count: images.length,
        images: images
    }});
}})();
"#, min_width, min_height, max_images, selector_code)
}

// ============================================================================
// 7. Execute
// ============================================================================

async fn handle_execute(req: ExecuteRequest, state: &V2AppState) -> McpToolResponse {
    // Auto-wrap in IIFE if script uses top-level `return` but isn't already wrapped
    let script = {
        let trimmed = req.script.trim();
        let needs_wrap = trimmed.contains("return ")
            && !trimmed.starts_with("(function")
            && !trimmed.starts_with("(async")
            && !trimmed.starts_with("((");
        if needs_wrap {
            format!("(function(){{{}}})();", req.script)
        } else {
            req.script.clone()
        }
    };
    match execute_script_in(&req.session, script, state, req.timeout_ms, req.frame.as_deref()).await {
        Ok(result) => {
            // Parse to proper JSON type for structured response
            let typed = serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result));
            McpToolResponse::success_json(serde_json::json!({
                "session": req.session,
                "result": typed,
                "result_type": match &typed {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                },
            }))
        }
        Err(e) => McpToolResponse::error("EXECUTE_FAILED", &e),
    }
}

// ============================================================================
// 9. Network
// ============================================================================

async fn handle_network(req: NetworkRequest, state: &V2AppState) -> McpToolResponse {
    use crate::mcp_v3::types::NetworkRequestAction;
    
    let manager = get_session_manager_v2();
    let handle = match manager.get_handle(&req.session) {
        Some(h) => h,
        None => return McpToolResponse::error("SESSION_NOT_FOUND", &format!("Session '{}' not found", req.session)),
    };

    let action = match req.action {
        NetworkRequestAction::Enable { max_logs } => crate::core::NetworkAction::Enable { max_logs },
        NetworkRequestAction::Disable => crate::core::NetworkAction::Disable,
        NetworkRequestAction::GetLogs { filter } => crate::core::NetworkAction::GetLogs { filter },
        NetworkRequestAction::ClearLogs => crate::core::NetworkAction::ClearLogs,
    };

    let (tx, rx) = oneshot::channel();
    let cmd = AppCommand::ManageNetwork {
        id: handle.id.clone(),
        action,
        resp_tx: tx,
    };

    if state.cmd_tx.send(cmd).is_err() {
        return McpToolResponse::error("COMMAND_FAILED", "Failed to send network command");
    }

    match tokio::time::timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(Ok(result))) => McpToolResponse::success_text(result), 
        Ok(Ok(Err(e))) => McpToolResponse::error("NETWORK_ERROR", &e),
        Ok(Err(_)) => McpToolResponse::error("CHANNEL_CLOSED", "Network channel closed"),
        Err(_) => McpToolResponse::error("TIMEOUT", "Network command timed out"),
    }
}

// ============================================================================
// MCP Tool List
// ============================================================================

/// Get tool definitions for MCP tools/list
pub fn get_mcp_tools() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "navigate",
            "description": "Load a URL in the browser. Use this to open websites, follow links by URL, or reload pages. Returns page title, final URL (after redirects), and detected challenges (CAPTCHA, Cloudflare). For bot-protected sites (Amazon, etc.): navigate to homepage ONLY, then use 'interact' with human_mode to click links — direct deep-URL navigation triggers bot detection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name. Must be acquired first via the 'session' tool." },
                    "url": { "type": "string", "description": "Full URL to navigate to (e.g. 'https://example.com')" },
                    "wait_for": { "type": "string", "enum": ["load", "stable", "networkidle", "selector"], "default": "stable", "description": "When to consider page ready. 'stable' (default) waits for DOM to stop changing — best for SPAs and dynamic pages. 'selector' waits for a specific CSS element. 'networkidle' waits for no network activity. 'load' waits for basic page load only." },
                    "wait_selector": { "type": "string", "description": "CSS selector to wait for. Required when wait_for='selector'. Example: '#main-content'" },
                    "timeout_ms": { "type": "integer", "default": 30000, "description": "Navigation timeout in milliseconds" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "interact",
            "description": "Perform browser actions: click buttons, type text, scroll, hover, wait for elements, and take screenshots. Accepts an array of actions executed in sequence. Use this instead of 'navigate' to move between pages on bot-protected sites — clicking links naturally avoids detection. Enable human_mode for sites with bot protection (adds realistic mouse curves, typing delays, micro-jitter). Screenshots include HTTP URLs for direct viewing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "actions": {
                        "type": "array",
                        "description": "Ordered list of browser actions to execute sequentially",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["click", "type", "scroll", "hover", "select", "wait", "screenshot"], "description": "Action to perform" },
                                "target": { "type": "string", "description": "CSS selector for the target element. Required for: click, type, hover, select. Example: '#search-btn', '.product-card a', 'input[name=q]'" },
                                "value": { "type": "string", "description": "For 'type': text to enter. For 'select': option value. For 'wait': CSS selector (condition=element) or URL/text pattern (condition=url_contains/text_contains)" },
                                "clear": { "type": "boolean", "default": false, "description": "For 'type': clear existing input value before typing" },
                                "instant": { "type": "boolean", "default": false, "description": "For 'type': set value instantly instead of character-by-character. Use when autocomplete dropdowns interfere (Amazon, Google search)" },
                                "condition": { "type": "string", "enum": ["timeout", "element", "element_visible", "element_clickable", "element_hidden", "url_contains", "url_matches", "text_contains", "network_idle"], "description": "For 'wait': what to wait for. 'element' waits for selector in 'value' to exist in DOM. 'element_visible' waits for it to be visible. 'timeout' simply pauses." },
                                "timeout_ms": { "type": "integer", "default": 10000, "description": "For 'wait': maximum wait time in ms" },
                                "direction": { "type": "string", "enum": ["down", "up", "left", "right"], "default": "down", "description": "For 'scroll': scroll direction" },
                                "amount": { "type": "integer", "default": 500, "description": "For 'scroll': distance in pixels" }
                            },
                            "required": ["type"]
                        }
                    },
                    "options": {
                        "type": "object",
                        "description": "Execution options applied to all actions",
                        "properties": {
                            "wait_timeout_ms": { "type": "integer", "default": 10000, "description": "Default timeout for wait actions" },
                            "retry_count": { "type": "integer", "default": 3, "description": "Number of retries on action failure" },
                            "screenshot_on_error": { "type": "boolean", "default": false, "description": "Automatically take screenshot when an action fails (returned as HTTP URL)" },
                            "human_mode": { "type": "boolean", "default": false, "description": "Simulate human behavior: bezier-curve mouse movement, random micro-jitter, 10% overshoot, 3% typo rate with self-correction, variable delays. Essential for bot-protected sites (Amazon, banks, social media)." },
                            "slow_mode_ms": { "type": "integer", "default": 0, "description": "Add fixed delay (ms) between each action" }
                        }
                    },
                    "frame": { "type": "string", "description": "Target iframe for script execution. Specify by URL substring (e.g. 'flow.shopifyapps.com'), frame name, or frame ID. Use 'session' tool's frames list to discover available frames. When omitted, actions run in the main page frame." }
                },
                "required": ["actions"]
            }
        },
        {
            "name": "capture",
            "description": "Read the current page state. Returns: URL, title, visible text, and a list of all interactive elements (buttons, links, inputs) with their CSS selectors — use these selectors with 'interact' or 'extract'. Also detects CAPTCHA/challenges and scores page interactivity (0-1). Use capture BEFORE extract to discover the correct CSS selectors for data extraction. Screenshots are saved and accessible via HTTP URL in the response.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "screenshot": { "type": "boolean", "default": true, "description": "Save a screenshot image. The response includes an HTTP URL to view it directly." },
                    "include": { "type": "array", "items": { "type": "string", "enum": ["cookies", "full_text", "html", "images"] }, "description": "Extra data to include. 'full_text' returns complete page text. 'html' returns raw HTML. 'cookies' returns session cookies. 'images' lists all images with src/alt." },
                    "selector": { "type": "string", "description": "Limit capture to a specific element by CSS selector. Example: '#product-detail'" },
                    "full_page": { "type": "boolean", "default": false, "description": "Capture entire scrollable page, not just visible viewport" },
                    "text_max_chars": { "type": "integer", "description": "Truncate text content to this many characters" },
                    "summarize": { "type": "boolean", "default": false, "description": "Use AI to generate a summary of page content" },
                    "analyze_vision": { "type": "boolean", "default": false, "description": "Use Vision LLM to describe images that have no alt text" },
                    "use_cdp": { "type": "boolean", "default": true, "description": "Use CDP for screenshots (higher quality, supports full_page)" },
                    "frame": { "type": "string", "description": "Target iframe for capture. Specify by URL substring (e.g. 'flow.shopifyapps.com'), frame name, or frame ID. When omitted, captures the main page frame." }
                }
            }
        },
        {
            "name": "extract",
            "description": "Extract structured data from repeating page elements into a JSON array. First use 'capture' to inspect the page and find correct CSS selectors. Set 'selector' to the repeating container (e.g. '.product-card'), then map field names to sub-selectors within each container. Use '@attr' suffix to get an attribute instead of text content (e.g. 'a@href' for link URL). If all fields return null, the response includes a diagnostic with the container's actual HTML structure — use it to fix your selectors and retry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "selector": { "type": "string", "description": "CSS selector matching the repeating container elements. Example: '.product-card', 'tr.search-result', '[data-testid=item]'" },
                    "fields": { "type": "object", "description": "Map of output field names to CSS sub-selectors within each container. For text content: 'title': '.product-name'. For attributes: 'link': 'a@href', 'image': 'img@src', 'rating': '.stars@data-score'. The sub-selector is relative to each container element." },
                    "limit": { "type": "integer", "description": "Maximum number of items to return" },
                    "wait_for_count": { "type": "integer", "description": "Wait until at least this many containers exist before extracting" },
                    "wait_timeout_ms": { "type": "integer", "default": 10000, "description": "Timeout for wait_for_count" },
                    "scroll_for_more": { "type": "boolean", "default": false, "description": "Enable scroll-and-collect mode for virtual-scroll sites (e.g. X.com). Extracts items at each scroll position, deduplicates, and accumulates results. Much more effective than scrolling then extracting once." },
                    "scroll_max": { "type": "integer", "default": 5, "description": "Maximum number of scroll-and-extract iterations. Each iteration: extract visible items → deduplicate → scroll down." },
                    "scroll_dedup_key": { "type": "string", "description": "Field name to use for deduplication (e.g. 'text'). If omitted, deduplicates by hashing the entire item JSON." },
                    "scroll_delay_ms": { "type": "integer", "default": 500, "description": "Delay in milliseconds between scroll iterations. Increase for slow-loading sites." },
                    "scroll_amount": { "type": "integer", "default": 800, "description": "Pixels to scroll per iteration. Adjust based on item height." },
                    "frame": { "type": "string", "description": "Target iframe for extraction. Specify by URL substring, frame name, or frame ID. When omitted, extracts from the main page frame." }
                },
                "required": ["selector", "fields"]
            }
        },
        {
            "name": "session",
            "description": "Manage browser sessions — create, resume, clone, release, and configure device emulation. Sessions are persistent: cookies, history, and login state survive server restarts. Once acquired, a session stays alive even after 'release' — re-acquiring the same name resumes exactly where you left off. Key operations: 'acquire' creates/resumes a session. 'release' marks it idle but keeps data. 'clone_to' copies cookies and profile to a new session (useful for sharing login state across parallel tasks). 'import' loads cookies from Firefox for logged-in browsing. 'list' shows all sessions. Device emulation: pass 'device' with acquire to start as mobile/tablet, or pass 'device' with 'session' (no acquire) to switch mid-workflow.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Target session name. When used alone with device/viewport/user_agent params (no acquire), switches device emulation on an already-active session. Also used as the source session for clone_to." },
                    "acquire": { "type": "string", "description": "Create or resume a session by name. If the session already exists, it reuses the same WebView with all cookies and login state intact — you do NOT need to log in again. If new, creates a fresh browser instance. Combine with 'device' to start in mobile/tablet mode." },
                    "release": { "type": "string", "description": "Release a session by name. The WebView, cookies, and all state stay alive for future reuse — this just marks it as idle. You can re-acquire the same name later to resume." },
                    "list": { "type": "boolean", "description": "Return all sessions with status, TTL, expiration time, and whether they are currently acquired." },
                    "clone_to": { "type": "string", "description": "Clone the source session's cookies and profile to a new session name. Source is 'session' param (default: 'default'). Use this to share authentication across multiple parallel sessions — e.g., log in once, then clone to 'worker-1', 'worker-2', etc." },
                    "import": { "type": "string", "description": "Session name to import cookies INTO from a local browser. Reads cookies from Firefox profile and sets them via CDP, enabling logged-in browsing without manual re-authentication. Requires 'browser' and 'domains'." },
                    "headless": { "type": "boolean", "default": false, "description": "true = hidden window (no visible browser). false = visible window (default). Use visible for bot-protected sites that require visual interaction or manual login." },
                    "restore": { "type": "boolean", "default": true, "description": "When resuming an existing session, automatically navigate back to the last URL. Set false to start from a blank page." },
                    "ttl_hours": { "type": "integer", "default": 168, "description": "Session lifetime in hours before auto-expiration. Default: 168 (1 week). Set 0 for permanent sessions that never expire. TTL auto-extends on each use, so active sessions won't expire unexpectedly." },
                    "browser": { "type": "string", "enum": ["firefox"], "description": "Browser to import cookies from. Currently only Firefox is supported (Chrome/Edge use DPAPI encryption which is not yet implemented)." },
                    "domains": { "type": "array", "items": { "type": "string" }, "description": "Cookie domains to import. Example: ['.amazon.co.jp', '.x.com']. Use leading dot for subdomain matching. Required with 'import'." },
                    "device": { "type": "string", "description": "Device preset name for emulation. Sets viewport size, user-agent, device-scale-factor, and touch support. Name matching is flexible: 'iPhone 14', 'iphone_14', 'iphone14' all work. Phones: iphone_se, iphone_14, iphone_14_plus, iphone_14_pro, iphone_14_pro_max, iphone_15, iphone_15_plus, iphone_15_pro, iphone_15_pro_max, iphone_16, iphone_16_plus, iphone_16_pro, iphone_16_pro_max, pixel_7, pixel_7_pro, pixel_8, pixel_8_pro, pixel_9, pixel_9_pro, pixel_9_pro_xl, galaxy_s23, galaxy_s23_ultra, galaxy_s24, galaxy_s24_ultra, galaxy_fold_5. Tablets: ipad, ipad_mini, ipad_air, ipad_pro_11, ipad_pro_12, galaxy_tab_s9, pixel_tablet. Desktops: desktop_1366x768, desktop_1080p, desktop_1440p, desktop_4k, macbook_air_13, macbook_pro_14, macbook_pro_16, imac_24. Generic: mobile_small (320), mobile_medium (375), mobile_large (414), tablet (768), laptop (1366), desktop (1920)." },
                    "viewport_width": { "type": "integer", "description": "Custom viewport width in pixels. Use with viewport_height for arbitrary sizes not covered by device presets. Overrides device preset if both are given." },
                    "viewport_height": { "type": "integer", "description": "Custom viewport height in pixels. Must be used together with viewport_width." },
                    "user_agent": { "type": "string", "description": "Custom User-Agent string. Overrides the user-agent set by device preset if both are given." }
                }
            }
        },
        {
            "name": "media",
            "description": "Media extraction from the current page. Use 'youtube_subtitles' to get video captions/transcripts. Use 'youtube_download' to save video files. Use 'collect_images' to gather all images on the page with src, alt, and dimensions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "action": {
                        "type": "object",
                        "description": "Media action to perform",
                        "properties": {
                            "type": { "type": "string", "enum": ["youtube_download", "youtube_subtitles", "video_analyze", "collect_images", "upload", "inject_file", "list_uploads"], "description": "'youtube_subtitles' = get captions/transcript. 'youtube_download' = download video. 'collect_images' = list images. 'upload' = upload file (base64 data+filename). 'inject_file' = inject uploaded file into <input type=file> via CDP. 'list_uploads' = list uploaded files for session." },
                            "data": { "type": "string", "description": "Base64-encoded file data (for 'upload' action)" },
                            "filename": { "type": "string", "description": "Filename (for 'upload' action)" },
                            "selector": { "type": "string", "description": "CSS selector for file input (for 'inject_file' action, default: input[type=file])" },
                            "file": { "type": "string", "description": "Upload URL or absolute path (for 'inject_file' action)" }
                        },
                        "required": ["type"]
                    }
                },
                "required": ["action"]
            }
        },
        {
            "name": "execute",
            "description": "Run JavaScript code in the browser page context. Use when other tools don't cover your needs — you have full DOM access. The result is returned as a typed JSON value (number, boolean, string, object, array, or null) with a 'result_type' field. Scripts with 'return' statements are auto-wrapped in a function. For complex extraction, prefer the 'extract' tool. For page inspection, prefer 'capture'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "script": { "type": "string", "description": "JavaScript code to execute in page context. Has full DOM access (document, window, etc.). Use 'return' to return values — scripts are auto-wrapped in IIFE if needed. Examples: 'document.title', 'return document.querySelector(\"#price\").textContent', 'return [...document.querySelectorAll(\"a\")].map(a=>({text:a.textContent,href:a.href}))'" },
                    "timeout_ms": { "type": "integer", "default": 30000, "description": "Maximum execution time in milliseconds" },
                    "frame": { "type": "string", "description": "Target iframe for script execution. Specify by URL substring, frame name, or frame ID. When omitted, executes in the main page frame." }
                },
                "required": ["script"]
            }
        },
        {
            "name": "agent",
            "description": "Autonomous browser agent. Give it a natural-language goal and it plans and executes the steps: navigating, clicking, typing, reading page content. Use for multi-step tasks like 'search for X on Y and return the top 3 results'. The agent uses all other tools internally with retries and error recovery. Use 'start' to begin, 'status' to check progress, 'cancel' to stop.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "action": {
                        "type": "object",
                        "description": "Agent control action",
                        "properties": {
                            "type": { "type": "string", "enum": ["start", "resume", "status", "cancel"], "description": "'start' = begin a new goal. 'resume' = continue after pause. 'status' = check current progress. 'cancel' = abort the goal." },
                            "goal": { "type": "string", "description": "Natural language description of what to achieve. Be specific. Example: 'Go to amazon.co.jp, search for mechanical keyboard, and extract the top 5 product names and prices'" },
                            "context": { "type": "string", "description": "Additional context about current state or constraints. Example: 'Already logged in, on the homepage'" },
                            "max_steps": { "type": "integer", "default": 5, "description": "Maximum number of tool calls the agent can make" },
                            "system_prompt": { "type": "string", "description": "Override default agent instructions. Use to constrain behavior or add domain knowledge." },
                            "human_mode": { "type": "boolean", "default": false, "description": "Enable human-like interaction (recommended for bot-protected sites)" },
                            "instant_type": { "type": "boolean", "default": false, "description": "Type text instantly (use when autocomplete interferes)" }
                        },
                        "required": ["type"]
                    }
                },
                "required": ["action"]
            }
        },
        {
            "name": "network",
            "description": "Monitor browser network traffic via CDP. Use 'enable' to start capturing HTTP requests/responses (XHR, fetch, etc.), then perform actions, then 'get_logs' to retrieve captured traffic. Useful for finding hidden API endpoints, inspecting request headers, or debugging failed requests. Use 'filter' to search logs by URL pattern.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default", "description": "Session name" },
                    "action": {
                        "type": "object",
                        "description": "Network monitoring action",
                        "properties": {
                            "type": { "type": "string", "enum": ["enable", "disable", "get_logs", "clear_logs"], "description": "'enable' = start capturing all network traffic. 'disable' = stop capturing. 'get_logs' = retrieve captured requests/responses. 'clear_logs' = delete captured data." },
                            "max_logs": { "type": "integer", "default": 100, "description": "For 'get_logs': maximum number of log entries to return (most recent first)" },
                            "filter": { "type": "string", "description": "For 'get_logs': only return logs whose URL contains this substring. Example: '/api/', '.json', 'graphql'" }
                        },
                        "required": ["type"]
                    }
                },
                "required": ["action"]
            }
        }
    ])
}
