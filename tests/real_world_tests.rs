//! Real World Integration Tests for WBP2
//!
//! These tests use actual websites to verify the V2 protocol works in production-like scenarios.
//! All tests are marked with #[ignore] to prevent running in CI.
//!
//! Run with: cargo test --test real_world_tests -- --ignored --test-threads=1

use std::time::Duration;
use reqwest::blocking::Client;

/// Test configuration
struct TestConfig {
    /// Base URL for the WBP2 server
    server_url: String,
    /// Timeout for requests
    timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:9222".to_string(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl TestConfig {
    fn client(&self) -> Client {
        Client::builder()
            .timeout(self.timeout)
            .build()
            .expect("Failed to create client")
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Acquire a session
fn acquire_session(config: &TestConfig, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = config.client();
    let resp = client
        .post(format!("{}/v2/session/acquire", config.server_url))
        .json(&serde_json::json!({
            "name": name,
            "headless": false
        }))
        .send()?;
    
    let body: serde_json::Value = resp.json()?;
    Ok(body["session"].as_str().unwrap_or(name).to_string())
}

/// Release a session
fn release_session(config: &TestConfig, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = config.client();
    let _ = client
        .post(format!("{}/v2/session/release", config.server_url))
        .json(&serde_json::json!({
            "name": name
        }))
        .send()?;
    Ok(())
}

/// Navigate to URL
fn navigate(config: &TestConfig, session: &str, url: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = config.client();
    let resp = client
        .post(format!("{}/v2/goal", config.server_url))
        .json(&serde_json::json!({
            "session": session,
            "type": "navigate",
            "target": url,
            "timeout_ms": 30000
        }))
        .send()?;
    
    Ok(resp.json()?)
}

/// Execute Goal API
fn execute_goal(
    config: &TestConfig, 
    session: &str, 
    goal_type: &str, 
    target: &str,
    params: Option<serde_json::Value>
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = config.client();
    let mut body = serde_json::json!({
        "session": session,
        "type": goal_type,
        "target": target,
        "timeout_ms": 15000
    });
    
    if let Some(p) = params {
        body["params"] = p;
    }
    
    let resp = client
        .post(format!("{}/v2/goal", config.server_url))
        .json(&body)
        .send()?;
    
    Ok(resp.json()?)
}

/// Wait for element
fn wait_for(
    config: &TestConfig,
    session: &str,
    selector: &str,
    condition: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = config.client();
    let resp = client
        .post(format!("{}/v2/wait", config.server_url))
        .json(&serde_json::json!({
            "session": session,
            "selector": selector,
            "condition": condition,
            "timeout_ms": 15000
        }))
        .send()?;
    
    Ok(resp.json()?)
}

/// Take screenshot
fn screenshot(
    config: &TestConfig,
    session: &str,
    mode: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = config.client();
    let resp = client
        .post(format!("{}/v2/screenshot", config.server_url))
        .json(&serde_json::json!({
            "session": session,
            "mode": mode,
            "format": "png"
        }))
        .send()?;
    
    Ok(resp.json()?)
}

// ============================================================================
// Phase 1: Public Site Basic Tests
// ============================================================================

/// Test: Google Search
/// 
/// Scenario: Navigate to Google, perform a search, verify results
#[test]
#[ignore = "Real world test - run manually"]
fn test_google_search() {
    let config = TestConfig::default();
    let session_name = "test_google";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to Google
    let nav_result = navigate(&config, &session, "https://www.google.com");
    assert!(nav_result.is_ok(), "Navigation to Google failed");
    println!("Navigated to Google");
    
    // 3. Wait for search box
    let wait_result = wait_for(&config, &session, "textarea[name='q'], input[name='q']", "visible");
    assert!(wait_result.is_ok(), "Search box not found");
    println!("Search box visible");
    
    // 4. Type search query
    let fill_result = execute_goal(
        &config, 
        &session, 
        "fill", 
        "textarea[name='q'], input[name='q']",
        Some(serde_json::json!({"value": "WebView Bridge Protocol"}))
    );
    assert!(fill_result.is_ok(), "Failed to fill search box");
    println!("Search query entered");
    
    // 5. Submit search (press Enter)
    let submit_result = execute_goal(&config, &session, "submit", "form", None);
    println!("Search submitted: {:?}", submit_result);
    
    // 6. Wait for results
    let results_wait = wait_for(&config, &session, "#search, #rso", "present");
    assert!(results_wait.is_ok(), "Search results not found");
    println!("Search results loaded");
    
    // 7. Take screenshot
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    println!("Screenshot taken");
    
    // 8. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

/// Test: Amazon Product Page
/// 
/// Scenario: Navigate to Amazon product, extract product info
#[test]
#[ignore = "Real world test - run manually"]
fn test_amazon_product_info() {
    let config = TestConfig::default();
    let session_name = "test_amazon";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to Amazon Japan homepage
    let nav_result = navigate(&config, &session, "https://www.amazon.co.jp");
    assert!(nav_result.is_ok(), "Navigation to Amazon failed");
    println!("Navigated to Amazon");
    
    // 3. Wait for page load
    let wait_result = wait_for(&config, &session, "#nav-logo, #nav-link-amazonprime", "visible");
    println!("Amazon page loaded: {:?}", wait_result);
    
    // 4. Search for a product
    let fill_result = execute_goal(
        &config,
        &session,
        "fill",
        "#twotabsearchtextbox",
        Some(serde_json::json!({"value": "Rust プログラミング"}))
    );
    println!("Search query entered: {:?}", fill_result);
    
    // 5. Click search button
    let click_result = execute_goal(&config, &session, "click", "#nav-search-submit-button", None);
    println!("Search button clicked: {:?}", click_result);
    
    // 6. Wait for search results
    std::thread::sleep(Duration::from_secs(3));
    let results_wait = wait_for(&config, &session, ".s-main-slot, .s-result-list", "present");
    println!("Search results: {:?}", results_wait);
    
    // 7. Extract product titles (first few)
    let extract_result = execute_goal(
        &config,
        &session,
        "extract",
        ".s-result-item h2 a span",
        None
    );
    println!("Extracted products: {:?}", extract_result);
    
    // 8. Take screenshot
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    
    // 9. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

/// Test: YouTube Video Info
/// 
/// Scenario: Navigate to YouTube video, extract video info
#[test]
#[ignore = "Real world test - run manually"]
fn test_youtube_video_info() {
    let config = TestConfig::default();
    let session_name = "test_youtube";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to YouTube
    let nav_result = navigate(&config, &session, "https://www.youtube.com");
    assert!(nav_result.is_ok(), "Navigation to YouTube failed");
    println!("Navigated to YouTube");
    
    // 3. Wait for page load
    std::thread::sleep(Duration::from_secs(2));
    let wait_result = wait_for(&config, &session, "#search, ytd-searchbox", "visible");
    println!("YouTube loaded: {:?}", wait_result);
    
    // 4. Search for a video
    let fill_result = execute_goal(
        &config,
        &session,
        "fill",
        "input#search, ytd-searchbox input",
        Some(serde_json::json!({"value": "Rust programming tutorial"}))
    );
    println!("Search query entered: {:?}", fill_result);
    
    // 5. Click search button
    let click_result = execute_goal(&config, &session, "click", "#search-icon-legacy, button#search-icon-legacy", None);
    println!("Search button clicked: {:?}", click_result);
    
    // 6. Wait for results
    std::thread::sleep(Duration::from_secs(3));
    let results_wait = wait_for(&config, &session, "ytd-video-renderer, ytd-rich-item-renderer", "present");
    println!("Search results: {:?}", results_wait);
    
    // 7. Take screenshot
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    
    // 8. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

/// Test: httpbin Form Submission
/// 
/// Scenario: Fill and submit a form, verify response
#[test]
#[ignore = "Real world test - run manually"]
fn test_httpbin_form_submission() {
    let config = TestConfig::default();
    let session_name = "test_httpbin";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to httpbin forms page
    let nav_result = navigate(&config, &session, "https://httpbin.org/forms/post");
    assert!(nav_result.is_ok(), "Navigation to httpbin failed");
    println!("Navigated to httpbin");
    
    // 3. Wait for form
    let wait_result = wait_for(&config, &session, "form", "visible");
    assert!(wait_result.is_ok(), "Form not found");
    println!("Form visible");
    
    // 4. Fill customer name
    let fill_name = execute_goal(
        &config,
        &session,
        "fill",
        "input[name='custname']",
        Some(serde_json::json!({"value": "Test User"}))
    );
    println!("Name filled: {:?}", fill_name);
    
    // 5. Fill email
    let fill_email = execute_goal(
        &config,
        &session,
        "fill",
        "input[name='custemail']",
        Some(serde_json::json!({"value": "test@example.com"}))
    );
    println!("Email filled: {:?}", fill_email);
    
    // 6. Fill comments
    let fill_comments = execute_goal(
        &config,
        &session,
        "fill",
        "textarea[name='comments']",
        Some(serde_json::json!({"value": "This is a test from WebView Bridge Protocol!"}))
    );
    println!("Comments filled: {:?}", fill_comments);
    
    // 7. Take screenshot before submit
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    println!("Screenshot before submit taken");
    
    // 8. Submit form
    let submit_result = execute_goal(&config, &session, "click", "button[type='submit']", None);
    println!("Form submitted: {:?}", submit_result);
    
    // 9. Wait for response page
    std::thread::sleep(Duration::from_secs(2));
    
    // 10. Take screenshot of result
    let screenshot_result2 = screenshot(&config, &session, "viewport");
    assert!(screenshot_result2.is_ok(), "Screenshot failed");
    println!("Screenshot after submit taken");
    
    // 11. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

/// Test: GitHub Repository Info
/// 
/// Scenario: Navigate to a GitHub repo, extract repository info
#[test]
#[ignore = "Real world test - run manually"]
fn test_github_repo_info() {
    let config = TestConfig::default();
    let session_name = "test_github";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to a popular Rust repo
    let nav_result = navigate(&config, &session, "https://github.com/rust-lang/rust");
    assert!(nav_result.is_ok(), "Navigation to GitHub failed");
    println!("Navigated to GitHub");
    
    // 3. Wait for page load
    let wait_result = wait_for(&config, &session, "[data-pjax='#repo-content-pjax-container'], .repository-content", "present");
    println!("GitHub repo loaded: {:?}", wait_result);
    
    // 4. Extract repo info
    let extract_stars = execute_goal(
        &config,
        &session,
        "extract",
        "#repo-stars-counter-star, .Counter",
        None
    );
    println!("Stars: {:?}", extract_stars);
    
    // 5. Extract description
    let extract_desc = execute_goal(
        &config,
        &session,
        "extract",
        "p.f4, .repository-content p",
        None
    );
    println!("Description: {:?}", extract_desc);
    
    // 6. Take screenshot
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    
    // 7. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

// ============================================================================
// Phase 2: Dynamic Content Tests
// ============================================================================

/// Test: Infinite Scroll (YouTube)
/// 
/// Scenario: Scroll down to load more content
#[test]
#[ignore = "Real world test - run manually"]
fn test_infinite_scroll() {
    let config = TestConfig::default();
    let session_name = "test_scroll";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to YouTube trending
    let nav_result = navigate(&config, &session, "https://www.youtube.com/feed/trending");
    assert!(nav_result.is_ok(), "Navigation failed");
    println!("Navigated to YouTube Trending");
    
    // 3. Wait for initial content
    std::thread::sleep(Duration::from_secs(3));
    let wait_result = wait_for(&config, &session, "ytd-video-renderer", "present");
    println!("Initial content loaded: {:?}", wait_result);
    
    // 4. Scroll down multiple times
    for i in 1..=3 {
        let scroll_result = execute_goal(
            &config,
            &session,
            "scroll",
            "body",
            Some(serde_json::json!({"direction": "down", "amount": 1000}))
        );
        println!("Scroll {}: {:?}", i, scroll_result);
        std::thread::sleep(Duration::from_secs(2));
    }
    
    // 5. Take full page screenshot
    let screenshot_result = screenshot(&config, &session, "full_page");
    println!("Full page screenshot: {:?}", screenshot_result);
    
    // 6. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

/// Test: SPA Navigation (GitHub)
/// 
/// Scenario: Navigate within GitHub SPA
#[test]
#[ignore = "Real world test - run manually"]
fn test_spa_navigation() {
    let config = TestConfig::default();
    let session_name = "test_spa";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to GitHub
    let nav_result = navigate(&config, &session, "https://github.com/tokio-rs/tokio");
    assert!(nav_result.is_ok(), "Navigation failed");
    println!("Navigated to Tokio repo");
    
    // 3. Wait for page load
    std::thread::sleep(Duration::from_secs(2));
    
    // 4. Click on Issues tab
    let click_result = execute_goal(&config, &session, "click", "a[data-tab-item='i1issues-tab']", None);
    println!("Clicked Issues tab: {:?}", click_result);
    
    // 5. Wait for issues to load
    std::thread::sleep(Duration::from_secs(2));
    let wait_issues = wait_for(&config, &session, ".js-issue-row, [data-hovercard-type='issue']", "present");
    println!("Issues loaded: {:?}", wait_issues);
    
    // 6. Take screenshot
    let screenshot_result = screenshot(&config, &session, "viewport");
    assert!(screenshot_result.is_ok(), "Screenshot failed");
    
    // 7. Click on Pull Requests tab
    let click_pr = execute_goal(&config, &session, "click", "a[data-tab-item='i2pull-requests-tab']", None);
    println!("Clicked PRs tab: {:?}", click_pr);
    
    // 8. Wait for PRs to load
    std::thread::sleep(Duration::from_secs(2));
    
    // 9. Take screenshot
    let screenshot_result2 = screenshot(&config, &session, "viewport");
    assert!(screenshot_result2.is_ok(), "Screenshot failed");
    
    // 10. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

// ============================================================================
// Phase 3: Device Emulation Tests
// ============================================================================

/// Test: Mobile Device Emulation
/// 
/// Scenario: View site as mobile device
#[test]
#[ignore = "Real world test - run manually"]
fn test_mobile_emulation() {
    let config = TestConfig::default();
    let session_name = "test_mobile";
    
    // 1. Acquire session
    let session = acquire_session(&config, session_name).expect("Failed to acquire session");
    println!("Session acquired: {}", session);
    
    // 2. Navigate to a responsive site
    let nav_result = navigate(&config, &session, "https://www.google.com");
    assert!(nav_result.is_ok(), "Navigation failed");
    println!("Navigated to Google");
    
    // 3. Wait for page load
    std::thread::sleep(Duration::from_secs(2));
    
    // 4. Take desktop screenshot
    let desktop_screenshot = screenshot(&config, &session, "viewport");
    println!("Desktop screenshot: {:?}", desktop_screenshot);
    
    // 5. Take mobile screenshot with device emulation
    let client = config.client();
    let mobile_screenshot = client
        .post(format!("{}/v2/screenshot", config.server_url))
        .json(&serde_json::json!({
            "session": session,
            "mode": "viewport",
            "device": "iphone_14",
            "format": "png"
        }))
        .send();
    println!("Mobile screenshot: {:?}", mobile_screenshot);
    
    // 6. Release session
    let _ = release_session(&config, &session);
    println!("Test completed successfully!");
}

// ============================================================================
// Utility Test: Server Health Check
// ============================================================================

/// Test: Server is running
#[test]
#[ignore = "Real world test - run manually"]
fn test_server_health() {
    let config = TestConfig::default();
    let client = config.client();
    
    // Check /v2/health endpoint
    let health_result = client
        .get(format!("{}/v2/health", config.server_url))
        .send();
    
    match health_result {
        Ok(resp) => {
            println!("Server responded with status: {}", resp.status());
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().unwrap_or_default();
                println!("Health response: {:?}", body);
            }
        }
        Err(e) => {
            println!("Server not reachable: {}", e);
            println!("Please start the server with: cargo run");
        }
    }
}
