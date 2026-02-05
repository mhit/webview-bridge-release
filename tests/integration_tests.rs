// Integration tests for WebView Bridge APIs
// Tests the actual HTTP endpoints with real server

use std::time::Duration;

/// Wait for server to start
async fn wait_for_server(base_url: &str, max_attempts: u32) -> bool {
    for _ in 0..max_attempts {
        if let Ok(response) = reqwest::get(format!("{}/health", base_url)).await {
            if response.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_health_endpoint() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let response = reqwest::get(format!("{}/health", base_url))
        .await
        .expect("Failed to call health endpoint");
    
    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_webdriver_status_endpoint() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/wd/hub/status", base_url))
        .send()
        .await
        .expect("Failed to call WebDriver status");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["value"]["ready"].as_bool().unwrap());
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_mcp_info_endpoint() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let response = reqwest::get(format!("{}/mcp/info", base_url))
        .await
        .expect("Failed to call MCP info");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "webview-bridge");
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_mcp_tools_endpoint() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let response = reqwest::get(format!("{}/mcp/tools", base_url))
        .await
        .expect("Failed to call MCP tools");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.as_array().map(|a| !a.is_empty()).unwrap_or(false));
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_cdp_version_endpoint() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let response = reqwest::get(format!("{}/json/version", base_url))
        .await
        .expect("Failed to call CDP version");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["Browser"].as_str().is_some());
    assert!(body["Protocol-Version"].as_str().is_some());
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_cdp_list_targets() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let response = reqwest::get(format!("{}/json/list", base_url))
        .await
        .expect("Failed to call CDP list");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.is_array());
}

#[tokio::test]
#[ignore = "Requires running server and WebView2"]
async fn test_session_create_and_close() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let client = reqwest::Client::new();
    
    // Create session
    let response = client
        .post(format!("{}/create", base_url))
        .json(&serde_json::json!({
            "profile": "test",
            "headless": false
        }))
        .send()
        .await
        .expect("Failed to create session");
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await.unwrap();
    let session_id = body["id"].as_str().unwrap();
    assert!(!session_id.is_empty());
    
    // Wait for session to initialize
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Close session
    let response = client
        .delete(format!("{}/close/{}", base_url, session_id))
        .send()
        .await
        .expect("Failed to close session");
    
    assert!(response.status().is_success());
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_concurrent_api_calls() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    // Make 10 concurrent requests to test parallelism
    let mut handles = vec![];
    
    for _ in 0..10 {
        let url = format!("{}/health", base_url);
        handles.push(tokio::spawn(async move {
            reqwest::get(&url).await.map(|r| r.status().is_success())
        }));
    }
    
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(true)) = handle.await {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 10, "All concurrent requests should succeed");
}

#[tokio::test]
#[ignore = "Requires running server"]
async fn test_response_time() {
    let base_url = "http://localhost:9400";
    
    if !wait_for_server(base_url, 30).await {
        panic!("Server not available");
    }
    
    let start = std::time::Instant::now();
    
    for _ in 0..100 {
        let _ = reqwest::get(format!("{}/health", base_url)).await;
    }
    
    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() / 100;
    
    // Average should be under 50ms for health check
    assert!(avg_ms < 50, "Average response time too slow: {}ms", avg_ms);
}
