//! E2E Workflow Tests for WBP2
//!
//! Full end-to-end tests using the page server and WBP2 API.

mod common;

use serde_json::{json, Value};

// ============================================================================
// Page Server Tests
// ============================================================================

#[tokio::test]
async fn test_page_server_index() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/", port))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert!(body.contains("WBP2 Test Site"));
}

#[tokio::test]
async fn test_page_server_login_page() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/login.html", port))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert!(body.contains("username"));
    assert!(body.contains("password"));
}

#[tokio::test]
async fn test_page_server_product_list() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/product_list.html", port))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert!(body.contains("Product A"));
    assert!(body.contains("$99.99"));
}

#[tokio::test]
async fn test_page_server_spa() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/spa.html", port))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert!(body.contains("SPA App"));
    assert!(body.contains("navigate"));
}

#[tokio::test]
async fn test_mock_login_api_success() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/api/login", port))
        .json(&json!({
            "username": "testuser",
            "password": "password123"
        }))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body: Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert!(body["token"].as_str().unwrap().len() > 0);
}

#[tokio::test]
async fn test_mock_login_api_failure() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/api/login", port))
        .json(&json!({
            "username": "wrong",
            "password": "wrong"
        }))
        .send()
        .await
        .expect("Failed to connect");
    
    assert_eq!(response.status().as_u16(), 401);
    let body: Value = response.json().await.unwrap();
    assert!(!body["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_mock_products_api() {
    use common::page_server::{start_test_page_server, get_test_page_port};
    
    let port = get_test_page_port();
    let _handle = start_test_page_server(port).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/products", port))
        .send()
        .await
        .expect("Failed to connect");
    
    assert!(response.status().is_success());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["total"], 3);
    assert!(body["products"].is_array());
    assert_eq!(body["products"][0]["name"], "Product A");
}

// ============================================================================
// API Server Tests
// ============================================================================

#[tokio::test]
async fn test_api_server_session_list() {
    use common::test_server::create_test_server;
    
    let server = create_test_server().await;
    let response = server.get("/v2/session/list").await;
    
    response.assert_status_ok();
    let body: Value = response.json();
    assert!(body["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_api_server_session_acquire() {
    use common::test_server::{create_test_server, HttpStatusCode};
    
    let server = create_test_server().await;
    let response = server
        .post("/v2/session/acquire")
        .json(&json!({"name": "test", "profile": "default"}))
        .await;
    
    response.assert_status(HttpStatusCode::CREATED);
    let body: Value = response.json();
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["session"], "test");
}

#[tokio::test]
async fn test_api_server_session_not_found() {
    use common::test_server::{create_test_server, HttpStatusCode};
    
    let server = create_test_server().await;
    let response = server.get("/v2/session/nonexistent").await;
    
    response.assert_status(HttpStatusCode::NOT_FOUND);
    let body: Value = response.json();
    assert!(!body["success"].as_bool().unwrap());
    assert_eq!(body["error"]["code"], "WBP2_001");
}

#[tokio::test]
async fn test_api_server_job_list() {
    use common::test_server::create_test_server;
    
    let server = create_test_server().await;
    let response = server.get("/v2/jobs").await;
    
    response.assert_status_ok();
    let body: Value = response.json();
    assert!(body["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_api_server_storage_status() {
    use common::test_server::create_test_server;
    
    let server = create_test_server().await;
    let response = server.get("/v2/storage/status").await;
    
    response.assert_status_ok();
    let body: Value = response.json();
    assert!(body["success"].as_bool().unwrap());
    assert!(body["storage"]["base_path"].is_string());
}

// ============================================================================
// Workflow Scenario Tests (Require Browser - Ignored)
// ============================================================================

#[tokio::test]
#[ignore] // Requires actual WebView2 browser
async fn test_e2e_login_workflow() {
    // Full workflow:
    // 1. Start page server
    // 2. Start WBP2 server
    // 3. Acquire session
    // 4. Navigate to login page
    // 5. Fill form with macro/goal
    // 6. Submit and verify
    // 7. Take screenshot
    // 8. Release session
    
    // This test requires the actual WebView2 browser
    // and is meant to be run manually or in a specialized CI environment
    
    assert!(true, "Placeholder for actual E2E test");
}

#[tokio::test]
#[ignore] // Requires actual WebView2 browser
async fn test_e2e_product_scraping() {
    // Full workflow:
    // 1. Navigate to product list
    // 2. Wait for elements
    // 3. Extract product data with AI
    // 4. Collect images
    // 5. Verify extracted data
    
    assert!(true, "Placeholder for actual E2E test");
}

#[tokio::test]
#[ignore] // Requires actual WebView2 browser
async fn test_e2e_spa_navigation() {
    // Full workflow:
    // 1. Navigate to SPA
    // 2. Detect SPA type
    // 3. Navigate between views
    // 4. Load more content
    // 5. Verify history API changes
    
    assert!(true, "Placeholder for actual E2E test");
}

#[tokio::test]
#[ignore] // Long running test
async fn test_stability_repeated_sessions() {
    // Stress test:
    // 1. Acquire/release session 100 times
    // 2. Monitor memory usage
    // 3. Verify no leaks
    
    assert!(true, "Placeholder for stability test");
}
