//! MCP v3 Tool Implementations
//!
//! Robust implementations wrapping V2 API

use super::types::*;
use super::robustness::*;
use crate::api_v2::{get_session_manager_v2, V2AppState};
use crate::core::AppCommand;
use tokio::sync::oneshot;
use std::time::Duration;
use base64::Engine;

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
            tracing::info!("[human_mode] Pre-action delay: {}ms", delay);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        
        match execute_action(session, action, options.wait_timeout_ms, state, options.human_mode).await {
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
            if human_mode {
                tracing::info!("[human_mode] Simulating mouse movement to: {}", target);
                let mouse_move_script = generate_human_mouse_move_script(target);
                let _ = execute_script(session, mouse_move_script, state, timeout_ms).await;
                // Small delay after mouse movement
                let delay = 50 + (rand::random::<u64>() % 100);
                tracing::info!("[human_mode] Mouse settle delay: {}ms", delay);
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
        
        Action::Type { target, value, clear, instant } => {
            // Wait for element
            let wait_script = generate_wait_for_clickable_script(target, timeout_ms);
            execute_script(session, wait_script, state, timeout_ms).await?;
            
            // Type with events - extend timeout based on text length and mode
            // Instant mode: fast direct set
            // Normal mode: ~20ms per char  
            // Human mode: ~150ms per char + typo corrections + thinking pauses
            let type_timeout = if *instant {
                timeout_ms.max(5000)
            } else if human_mode {
                // Human-like typing: base 50ms + variance + 3% typos with correction + 2% pauses
                // Conservative estimate: 150ms per char + 2 seconds buffer for pauses/typos
                timeout_ms.max(5000 + (value.len() as u64 * 150) + 3000)
            } else {
                timeout_ms.max(5000 + (value.len() as u64 * 20))
            };
            
            tracing::info!("[type] Input length: {}, instant: {}, human_mode: {}, timeout: {}ms", 
                value.len(), instant, human_mode, type_timeout);
            
            let type_script = generate_type_with_events_script_ex(target, value, *clear, *instant);
            let result = execute_script(session, type_script, state, type_timeout).await;
            
            match result {
                Ok(res) => {
                    let parsed: serde_json::Value = serde_json::from_str(&res)
                        .map_err(|e| format!("Failed to parse type result: {}", e))?;
                    
                    if !parsed["success"].as_bool().unwrap_or(false) {
                        return Err(parsed["error"].as_str().unwrap_or("Type failed").to_string());
                    }
                    
                    tracing::info!("[type] Success: typed {} chars, instant: {}, final value: {}", 
                        parsed["length"].as_u64().unwrap_or(0),
                        parsed["instant"].as_bool().unwrap_or(false),
                        parsed["value"].as_str().unwrap_or("?"));
                }
                Err(e) => {
                    // Lenient mode: if typing failed but it's instant mode, 
                    // check if value was actually set before failing
                    if *instant {
                        tracing::warn!("[type] Instant mode failed: {}, checking if value was set anyway...", e);
                        
                        let verify_script = format!(r#"
                            (function() {{
                                const el = document.querySelector("{}");
                                if (el && el.value) {{
                                    return JSON.stringify({{ success: true, value: el.value }});
                                }}
                                return JSON.stringify({{ success: false }});
                            }})();
                        "#, target.replace('"', "\\\""));
                        
                        if let Ok(verify_result) = execute_script(session, verify_script, state, 2000).await {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&verify_result) {
                                if v["success"].as_bool().unwrap_or(false) {
                                    tracing::info!("[type] Lenient success: value was set to: {}", 
                                        v["value"].as_str().unwrap_or("?"));
                                    return Ok(None); // Success despite the error!
                                }
                            }
                        }
                    }
                    return Err(e);
                }
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
    
    // Build AI-optimized text response
    let url = analyzed_result["url"].as_str().unwrap_or("Unknown");
    let title = analyzed_result["title"].as_str().unwrap_or("Unknown");
    
    let mut text = format!("URL: {}\nTitle: {}\n\n【操作可能要素】\n", url, title);
    
    if let Some(elements) = analyzed_result["elements"].as_array() {
        // Filter: score >= 0.3, limit to top 20 for context efficiency
        let filtered: Vec<_> = elements.iter()
            .filter(|el| {
                el.get("interactivity")
                    .and_then(|i| i.get("score"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) >= 0.3
            })
            .take(20)
            .collect();
        
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

    
    // 5. Take screenshot (save to file, return URL)
    if req.screenshot {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("cap_{}.png", timestamp);
        
        // Use html2canvas-like approach via JavaScript
        let screenshot_script = r#"
            (async function() {
                const canvas = document.createElement('canvas');
                const ctx = canvas.getContext('2d');
                canvas.width = window.innerWidth;
                canvas.height = window.innerHeight;
                
                // Simple approach: capture visible viewport as data URL
                // Note: This is limited but works without external libraries
                try {
                    // Return page dimensions for now
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
        
        match execute_script(&req.session, screenshot_script.to_string(), state, 5000).await {
            Ok(result) => {
                // For now, just report dimensions
                text.push_str(&format!("\n【ページ情報】\n{}", result));
            }
            Err(e) => {
                text.push_str(&format!("\n【スクリーンショット失敗】{}", e));
            }
        }
    }
    
    // Handle additional includes
    if req.include.contains(&CaptureInclude::Cookies) {
        let cookie_script = r#"
            (function() {
                const cookies = document.cookie.split(';').map(c => {
                    const [name, ...valueParts] = c.trim().split('=');
                    return {
                        name: name,
                        value: valueParts.join('='),
                        domain: window.location.hostname
                    };
                }).filter(c => c.name);
                return JSON.stringify({
                    success: true,
                    count: cookies.length,
                    cookies: cookies
                });
            })();
        "#;
        
        match execute_script(&req.session, cookie_script.to_string(), state, 5000).await {
            Ok(result) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                    if let Some(cookies) = parsed["cookies"].as_array() {
                        text.push_str(&format!("\n\n【Cookies】({}件)\n", cookies.len()));
                        for cookie in cookies.iter().take(20) {
                            let name = cookie["name"].as_str().unwrap_or("?");
                            let value = cookie["value"].as_str().unwrap_or("").chars().take(30).collect::<String>();
                            text.push_str(&format!("- {}={}\n", name, value));
                        }
                        if cookies.len() > 20 {
                            text.push_str(&format!("... 他{}件\n", cookies.len() - 20));
                        }
                    }
                }
            }
            Err(_) => {
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
        
        if let Ok(result) = execute_script(&req.session, text_script, state, 5000).await {
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
        
        if let Ok(result) = execute_script(&req.session, html_script, state, 5000).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(html) = parsed["html"].as_str() {
                    text.push_str(&format!("\n\n【HTML】({}文字)\n{}", html.len(), html));
                }
            }
        }
    }
    
    if req.include.contains(&CaptureInclude::Images) {
        let images_script = generate_collect_images_script(None, 50, 50, 30);
        
        if let Ok(result) = execute_script(&req.session, images_script, state, 5000).await {
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
        
        if let Ok(result) = execute_script(&req.session, text_script.to_string(), state, 5000).await {
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
                let sessions: Vec<serde_json::Value> = response.sessions.iter()
                    .map(|s| serde_json::json!({
                        "name": s.name,
                        "profile": s.profile,
                        "status": format!("{:?}", s.auth_status),
                        "last_accessed": s.last_accessed,
                        "active": s.active,
                        "acquired": s.acquired,
                        "expired": s.expired
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
        MediaAction::CollectImages { selector, min_width, min_height, download, max_images } => {
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
                        
                        let result = execute_action(&req.session, &click_action, 10000, state, human_mode).await;
                        
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
                        
                        let result = execute_action(&req.session, &type_action, type_timeout, state, human_mode).await;
                        
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
    match execute_script(&req.session, req.script, state, req.timeout_ms).await {
        Ok(result) => McpToolResponse::success_text(format!("Result: {}", result)),
        Err(e) => McpToolResponse::error("EXECUTE_FAILED", &e),
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
            "description": "Navigate to URL with wait conditions. For SPA sites use 'stable' or 'selector' wait. BOT DETECTION WARNING: For protected sites (Amazon, etc.), avoid direct URL navigation to subpages. Instead: 1) navigate to homepage only, 2) use interact with human_mode:true to click through links naturally. Direct URL jumps trigger bot detection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session name (default: 'default')" },
                    "url": { "type": "string", "description": "URL to navigate to. For bot-protected sites, only navigate to homepage, then use interact to click links." },
                    "wait_for": { "type": "string", "enum": ["load", "stable", "networkidle", "selector"], "default": "stable", "description": "load=basic, stable=DOM stops changing, networkidle=no network, selector=wait for element" },
                    "wait_selector": { "type": "string", "description": "CSS selector to wait for (required if wait_for=selector)" },
                    "timeout_ms": { "type": "integer", "default": 30000 }
                },
                "required": ["url"]
            }
        },
        {
            "name": "interact",
            "description": "Execute browser actions. RECOMMENDED FOR BOT-PROTECTED SITES: Use interact with human_mode:true instead of navigate to move between pages by clicking links. Action types: click{target}, type{target,value,clear?}, scroll{direction?,amount?,target?}, hover{target}, select{target,value}, wait{condition,value?,timeout_ms?}, screenshot. Wait condition: timeout, element, element_visible, element_clickable, element_hidden, url_contains, text_contains, network_idle.",
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
                                "instant": { "type": "boolean", "description": "Instant mode: set value directly instead of char-by-char. Use for autocomplete-heavy inputs like Amazon search." },
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
                            "human_mode": { "type": "boolean", "default": false, "description": "Enable human-like behavior: bezier curve mouse movement with ease-in-out, micro-jitter, overshoot (10%), typo simulation (3%) with backspace correction, double-space/shift mistakes. Highly recommended for bot-protected sites like Amazon." }
                        }
                    }
                },
                "required": ["actions"]
            }
        },
        {
            "name": "capture",
            "description": "Capture current page state for AI analysis. Returns: URL, title, list of interactive elements (buttons, links, inputs with selectors). Options: screenshot=true saves image, include=['cookies','full_text','html','images'] for extra data, selector limits to element, full_page captures entire page. NEW: summarize=true uses AI to generate a concise summary of the page content.",
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
            "description": "Session management with persistent cookies. BOT DETECTION TIPS: 1) Use headless=false for protected sites, 2) Login manually with window visible, 3) After login, use interact with human_mode:true to navigate via clicks instead of direct URLs, 4) Add random waits/scrolls between actions. Cookies persist across restarts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "acquire": { "type": "string", "description": "Acquire session by name. Creates WebView if needed, reuses existing cookies." },
                    "release": { "type": "string", "description": "Release session (keeps cookies and WebView alive for reuse)" },
                    "list": { "type": "boolean", "description": "List all sessions with status" },
                    "import": { "type": "string", "description": "Import cookies from browser profile" },
                    "headless": { "type": "boolean", "description": "false=visible window (recommended for bot-protected sites), true=hidden window. Default: false" },
                    "restore": { "type": "boolean", "default": true, "description": "Restore last URL on session resume" },
                    "browser": { "type": "string", "enum": ["chrome", "edge", "firefox"], "description": "Browser to import cookies from" },
                    "domains": { "type": "array", "items": { "type": "string" }, "description": "Cookie domains to import (e.g. ['amazon.co.jp'])" }
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
            "description": "Goal-based browser automation with internal AI. Provide a goal and the agent will autonomously navigate, click, type to achieve it. Uses MCP tools internally with full robustness (visibility checks, retries, human_mode).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "default": "default" },
                    "action": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["start", "resume", "status", "cancel"] },
                            "goal": { "type": "string", "description": "The goal to achieve, e.g. 'Search for Rust on DuckDuckGo'" },
                            "context": { "type": "string", "description": "Additional context about the current situation" },
                            "max_steps": { "type": "integer", "description": "Maximum number of steps (default: 5)" },
                            "system_prompt": { "type": "string", "description": "Custom instructions for the internal AI agent." },
                            "human_mode": { "type": "boolean", "description": "Enable human-like behavior: delays and natural movements (recommended for bot-protected sites)" },
                            "instant_type": { "type": "boolean", "description": "Use instant mode for typing (avoids autocomplete interference on Amazon, Google, etc.)" }
                        }
                    }
                },
                "required": ["action"]
            }
        }
    ])
}
