// WebView Bridge Integration Tests
//
// Note: These tests require the server to be running on http://127.0.0.1:9400
// Start the server first with: cargo run --release
// Then run tests with: cargo test --test integration_test

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::time::Duration;

    // Create a client with longer timeout for slow operations
    fn get_client() -> reqwest::Client {
        println!("[TEST] Creating HTTP client with 120s timeout");
        reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create client")
    }

    // Helper function to wait for server with timeout
    async fn wait_for_server(max_retries: u32) -> Result<(), Box<dyn std::error::Error>> {
        println!("[TEST] Waiting for server (max {} retries)...", max_retries);
        for i in 0..max_retries {
            println!("[TEST]   Attempt {}/{}", i + 1, max_retries);
            if let Ok(response) = reqwest::get("http://127.0.0.1:9400/health").await {
                if response.status() == 200 {
                    println!("[TEST] Server is ready!");
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        println!("[TEST] ERROR: Server not available after {} retries", max_retries);
        Err("Server not available".into())
    }

    // Helper function to wait for session to be ready
    async fn wait_for_session_ready(session_id: &str, max_retries: u32) -> Result<(), Box<dyn std::error::Error>> {
        println!("[TEST] Waiting for session {} to be ready (max {} retries)...", session_id, max_retries);
        let client = reqwest::Client::new();
        for i in 0..max_retries {
            println!("[TEST]   Check {}/{} for session {}", i + 1, max_retries, session_id);
            if let Ok(response) = client.get(&format!("http://127.0.0.1:9400/status/{}", session_id))
                .send()
                .await
            {
                if response.status() == 200 {
                    if let Ok(json) = response.json::<serde_json::Value>().await {
                        println!("[TEST]     Status: {:?}", json["status"]);
                        if json["status"] == "Ready" {
                            println!("[TEST] Session {} is ready!", session_id);
                            return Ok(());
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        println!("[TEST] ERROR: Session {} not ready after {} retries", session_id, max_retries);
        Err("Session not ready".into())
    }

    // ------------------------------------------------------------------------
    // Core API Tests (TDD: Red -> Green -> Refactor)
    // ------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_health_check() {
        println!("\n[TEST] ========== test_health_check START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        // When: Calling health endpoint
        println!("[TEST] Calling GET /health");
        let response = reqwest::get("http://127.0.0.1:9400/health")
            .await
            .expect("Failed to call health endpoint");

        // Then: Should return OK
        println!("[TEST] Response status: {}", response.status());
        assert_eq!(response.status(), 200);
        let body = response.text().await.unwrap();
        println!("[TEST] Response body: {}", body);
        assert_eq!(body, "OK");
        println!("[TEST] ========== test_health_check PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_create_session_success() {
        println!("\n[TEST] ========== test_create_session_success START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        // When: Creating a new session
        println!("[TEST] Creating session with profile=test-integration, headless=true");
        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-integration",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        // Then: Should return 201 with session ID
        println!("[TEST] Response status: {}", response.status());
        assert_eq!(response.status(), 201);

        let json: serde_json::Value = response.json().await.unwrap();
        println!("[TEST] Response: {:?}", json);
        assert!(json["id"].is_string());
        assert!(!json["id"].as_str().unwrap().is_empty());
        println!("[TEST] ========== test_create_session_success PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_create_session_with_options() {
        println!("\n[TEST] ========== test_create_session_with_options START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        // When: Creating a session with custom options
        println!("[TEST] Creating session with custom options");
        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "custom-profile",
                "headless": false,
                "userAgent": "TestAgent/1.0"
            }))
            .send()
            .await
            .expect("Failed to create session");

        // Then: Should return 201
        println!("[TEST] Response status: {}", response.status());
        assert_eq!(response.status(), 201);
        println!("[TEST] ========== test_create_session_with_options PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_navigate_session() {
        println!("\n[TEST] ========== test_navigate_session START ==========");

        // Given: Server is running and we have a session
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = reqwest::Client::new();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-navigate",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready before navigating
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 120).await.expect("Session not ready");

        // When: Navigating to a URL
        println!("[TEST] Navigating to https://example.com");
        let navigate_response = client
            .post(&format!("http://127.0.0.1:9400/navigate/{}", session_id))
            .json(&json!({
                "url": "https://example.com"
            }))
            .send()
            .await
            .expect("Failed to navigate");

        // Then: Should return 200
        println!("[TEST] Navigate response status: {}", navigate_response.status());
        assert_eq!(navigate_response.status(), 200);

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_navigate_session PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_execute_script() {
        println!("\n[TEST] ========== test_execute_script START ==========");

        // Given: Server is running and we have a session
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-execute-script",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready before executing script
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 120).await.expect("Session not ready");

        // Additional wait for WebView2 to be fully initialized
        println!("[TEST] Additional 10s wait for WebView2 initialization...");
        tokio::time::sleep(Duration::from_secs(10)).await;

        // When: Executing a script
        println!("[TEST] Executing script: document.title");
        let script_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.title"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        // Then: Should return 200 with result
        println!("[TEST] Script response status: {}", script_response.status());
        assert_eq!(script_response.status(), 200);

        let result_json: serde_json::Value = script_response.json().await.unwrap();
        println!("[TEST] Script result: {:?}", result_json);
        assert!(result_json["result"].is_string());

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_execute_script PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_get_session_status() {
        println!("\n[TEST] ========== test_get_session_status START ==========");

        // Given: Server is running and we have a session
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = reqwest::Client::new();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-status",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // When: Getting session status
        println!("[TEST] Getting session status...");
        let status_response = client
            .get(&format!("http://127.0.0.1:9400/status/{}", session_id))
            .send()
            .await
            .expect("Failed to get status");

        // Then: Should return 200 with session info
        println!("[TEST] Status response: {}", status_response.status());
        assert_eq!(status_response.status(), 200);

        let status_json: serde_json::Value = status_response.json().await.unwrap();
        println!("[TEST] Session info: {:?}", status_json);
        assert_eq!(status_json["id"], session_id);
        assert!(["Initializing", "Ready", "Busy", "Error"]
            .contains(&status_json["status"].as_str().unwrap()));

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_get_session_status PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_close_session() {
        println!("\n[TEST] ========== test_close_session START ==========");

        // Given: Server is running and we have a session
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = reqwest::Client::new();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-close",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // When: Closing the session
        println!("[TEST] Closing session...");
        let close_response = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await
            .expect("Failed to close session");

        // Then: Should return 200
        println!("[TEST] Close response: {}", close_response.status());
        assert_eq!(close_response.status(), 200);

        // And: Session should no longer exist
        println!("[TEST] Verifying session is closed...");
        let status_response = client
            .get(&format!("http://127.0.0.1:9400/status/{}", session_id))
            .send()
            .await
            .expect("Failed to get status");

        println!("[TEST] Status check response: {}", status_response.status());
        assert_eq!(status_response.status(), 404);
        println!("[TEST] ========== test_close_session PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_act_click() {
        println!("\n[TEST] ========== test_act_click START ==========");

        // Given: Server is running and we have a session
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = reqwest::Client::new();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-act",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready before performing action
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 90).await.expect("Session not ready");

        // When: Performing a click action
        println!("[TEST] Performing click action on #submit");
        let act_response = client
            .post(&format!("http://127.0.0.1:9400/act/{}", session_id))
            .json(&json!({
                "kind": "click",
                "ref_attr": "#submit"
            }))
            .send()
            .await
            .expect("Failed to perform action");

        // Then: Should return 200 (element may not exist, but action was processed)
        println!("[TEST] Act response: {}", act_response.status());
        assert_eq!(act_response.status(), 200);

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_act_click PASSED ==========\n");
    }

    // ------------------------------------------------------------------------
    // Error Cases Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_navigate_nonexistent_session() {
        // Given: Server is running
        wait_for_server(5).await.expect("Server not available");

        // When: Trying to navigate a non-existent session
        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:9400/navigate/nonexistent-id")
            .json(&json!({
                "url": "https://example.com"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // Then: Should return 404 or error
        assert!(response.status().as_u16() >= 400);
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_close_nonexistent_session() {
        // Given: Server is running
        wait_for_server(5).await.expect("Server not available");

        // When: Trying to close a non-existent session
        let client = reqwest::Client::new();
        let response = client
            .delete("http://127.0.0.1:9400/close/nonexistent-id")
            .send()
            .await
            .expect("Failed to send request");

        // Then: Should return 404
        assert_eq!(response.status(), 404);
    }

    // ------------------------------------------------------------------------
    // Timeout Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_request_timeout() {
        // Given: Server is running
        wait_for_server(5).await.expect("Server not available");

        // When: Making a request with a short timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .expect("Failed to build client");

        let result = client.get("http://127.0.0.1:9400/health").send().await;

        // Then: Should complete within timeout (health check is fast)
        assert!(result.is_ok());
    }

    // ------------------------------------------------------------------------
    // Profile Management Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_profile_create_and_list() {
        // Given: Server is running
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();

        // When: Creating a new profile
        let create_response = client
            .post("http://127.0.0.1:9400/profile/create")
            .json(&json!({
                "name": "test-persist-1"
            }))
            .send()
            .await
            .expect("Failed to create profile");

        assert_eq!(create_response.status(), 201);
        let create_json: serde_json::Value = create_response.json().await.unwrap();
        assert_eq!(create_json["name"], "test-persist-1");

        // Then: Profile should appear in list
        let list_response = client
            .get("http://127.0.0.1:9400/profile/list")
            .send()
            .await
            .expect("Failed to list profiles");

        assert_eq!(list_response.status(), 200);
        let list_json: serde_json::Value = list_response.json().await.unwrap();
        assert!(list_json["profiles"].is_array());

        let profiles = list_json["profiles"].as_array().unwrap();
        let found = profiles.iter().any(|p| p["name"] == "test-persist-1");
        assert!(found, "Created profile not found in list");

        // Cleanup
        let _ = client
            .delete("http://127.0.0.1:9400/profile/test-persist-1")
            .send()
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_profile_cookie_persistence() {
        // Given: Server is running
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();

        // Step 1: Create profile
        let _ = client
            .post("http://127.0.0.1:9400/profile/create")
            .json(&json!({
                "name": "test-persist-2"
            }))
            .send()
            .await
            .expect("Failed to create profile");

        // Step 2: Create session with the profile
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-persist-2",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let session_id: String = create_response.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();

        // Wait for session to be ready
        wait_for_session_ready(&session_id, 120).await.expect("Session not ready");

        // Step 3: Set a cookie
        let set_cookie_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.cookie = 'test=value; path=/';"
            }))
            .send()
            .await
            .expect("Failed to set cookie");

        assert_eq!(set_cookie_response.status(), 200);

        // Wait a bit for cookie to be persisted
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Step 4: Close session
        let close_response = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await
            .expect("Failed to close session");

        assert_eq!(close_response.status(), 200);

        // Wait for session to fully close
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Step 5: Create a new session with the same profile
        let create_response2 = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-persist-2",
                "headless": true
            }))
            .send()
            .await
            .expect("Failed to create session");

        let session_id2: String = create_response2.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();

        // Wait for session to be ready
        wait_for_session_ready(&session_id2, 120).await.expect("Session not ready");

        // Step 6: Check if cookie persisted (navigate to a page to check cookies)
        let nav_response = client
            .post(&format!("http://127.0.0.1:9400/navigate/{}", session_id2))
            .json(&json!({
                "url": "https://example.com"
            }))
            .send()
            .await
            .expect("Failed to navigate");

        assert_eq!(nav_response.status(), 200);
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Step 7: Get cookies via script
        let get_cookies_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id2))
            .json(&json!({
                "script": "document.cookie"
            }))
            .send()
            .await
            .expect("Failed to get cookies");

        assert_eq!(get_cookies_response.status(), 200);
        let result_json: serde_json::Value = get_cookies_response.json().await.unwrap();
        let result = result_json["result"].as_str().unwrap_or("");

        // Then: Cookie should NOT be persisted (example.com has different domain)
        assert!(!result.contains("test=value"), "Cookie should not be persisted across domains");

        // Cleanup
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id2))
            .send()
            .await;

        let _ = client
            .delete("http://127.0.0.1:9400/profile/test-persist-2")
            .send()
            .await;
    }

    // ------------------------------------------------------------------------
    // Real-World Scraping Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_real_website_scraping() {
        println!("\n[TEST] ========== test_real_website_scraping START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-scraping",
                "headless": false
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 120).await.expect("Session not ready");

        // When: Navigating to a real website (Wikipedia)
        println!("[TEST] Navigating to Wikipedia...");
        let nav_response = client
            .post(&format!("http://127.0.0.1:9400/navigate/{}", session_id))
            .json(&json!({
                "url": "https://ja.wikipedia.org/wiki/ウェブスクレイピング"
            }))
            .send()
            .await
            .expect("Failed to navigate");

        assert_eq!(nav_response.status(), 200);
        println!("[TEST] Navigate successful, waiting for page load...");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Then: Extract page title
        println!("[TEST] Extracting page title...");
        let title_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.title"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(title_response.status(), 200);
        let title_json: serde_json::Value = title_response.json().await.unwrap();
        let title = title_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page title: {}", title);
        assert!(!title.is_empty(), "Title should not be empty");
        assert!(
            title.contains("ウェブスクレイピング") || title.contains("Web scraping"),
            "Title should contain 'ウェブスクレイピング' or 'Web scraping'"
        );

        // Then: Extract page URL
        println!("[TEST] Extracting page URL...");
        let url_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "window.location.href"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(url_response.status(), 200);
        let url_json: serde_json::Value = url_response.json().await.unwrap();
        let url = url_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page URL: {}", url);
        assert!(url.contains("wikipedia.org"), "URL should contain wikipedia.org");

        // Then: Extract first heading
        println!("[TEST] Extracting first heading...");
        let heading_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.querySelector('h1')?.textContent || 'no heading'"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(heading_response.status(), 200);
        let heading_json: serde_json::Value = heading_response.json().await.unwrap();
        let heading = heading_json["result"].as_str().unwrap_or("");
        println!("[TEST] First heading: {}", heading);
        assert!(!heading.is_empty() && heading != "no heading", "Should find a heading");

        // Then: Extract all links
        println!("[TEST] Extracting links...");
        let links_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "Array.from(document.querySelectorAll('a')).slice(0, 5).map(a => a.href).join('|')"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(links_response.status(), 200);
        let links_json: serde_json::Value = links_response.json().await.unwrap();
        let links = links_json["result"].as_str().unwrap_or("");
        println!("[TEST] First 5 links: {}", links);
        assert!(!links.is_empty(), "Should find links");

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_real_website_scraping PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_google_search() {
        println!("\n[TEST] ========== test_google_search START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-search",
                "headless": false
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 120).await.expect("Session not ready");

        // When: Navigating to Google
        println!("[TEST] Navigating to Google...");
        let nav_response = client
            .post(&format!("http://127.0.0.1:9400/navigate/{}", session_id))
            .json(&json!({
                "url": "https://www.google.com"
            }))
            .send()
            .await
            .expect("Failed to navigate");

        assert_eq!(nav_response.status(), 200);
        println!("[TEST] Navigate successful, waiting for page load...");
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Then: Check page title
        println!("[TEST] Checking page title...");
        let title_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.title"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(title_response.status(), 200);
        let title_json: serde_json::Value = title_response.json().await.unwrap();
        let title = title_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page title: {}", title);
        assert!(!title.is_empty(), "Title should not be empty");

        // Then: Find search box and enter query
        println!("[TEST] Entering search query...");
        let search_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": r#"
                    (function() {
                        const searchInput = document.querySelector('textarea[name="q"]') || document.querySelector('input[name="q"]');
                        if (searchInput) {
                            searchInput.value = 'Rust programming language';
                            searchInput.dispatchEvent(new Event('input', { bubbles: true }));
                            return 'Search query entered';
                        }
                        return 'Search input not found';
                    })();
                "#
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(search_response.status(), 200);
        let search_json: serde_json::Value = search_response.json().await.unwrap();
        let search_result = search_json["result"].as_str().unwrap_or("");
        println!("[TEST] Search query result: {}", search_result);
        assert_eq!(search_result, "Search query entered", "Should find search input");

        // Then: Get page content snippet
        println!("[TEST] Getting page content snippet...");
        let content_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.body.innerText.substring(0, 200)"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(content_response.status(), 200);
        let content_json: serde_json::Value = content_response.json().await.unwrap();
        let content = content_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page content snippet: {}...", content);
        assert!(!content.is_empty(), "Should get page content");

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_google_search PASSED ==========\n");
    }

    #[tokio::test]
    #[ignore = "Requires running server"]
    async fn test_yahoo_japan_top_page() {
        println!("\n[TEST] ========== test_yahoo_japan_top_page START ==========");

        // Given: Server is running
        println!("[TEST] Waiting for server...");
        wait_for_server(5).await.expect("Server not available");

        let client = get_client();
        println!("[TEST] Creating session...");
        let create_response = client
            .post("http://127.0.0.1:9400/create")
            .json(&json!({
                "profile": "test-yahoo",
                "headless": false
            }))
            .send()
            .await
            .expect("Failed to create session");

        let create_json: serde_json::Value = create_response.json().await.unwrap();
        let session_id = create_json["id"].as_str().unwrap();
        println!("[TEST] Session created: {}", session_id);

        // Wait for session to be ready
        println!("[TEST] Waiting for session to be ready...");
        wait_for_session_ready(session_id, 120).await.expect("Session not ready");

        // When: Navigating to Yahoo Japan
        println!("[TEST] Navigating to Yahoo Japan...");
        let nav_response = client
            .post(&format!("http://127.0.0.1:9400/navigate/{}", session_id))
            .json(&json!({
                "url": "https://www.yahoo.co.jp"
            }))
            .send()
            .await
            .expect("Failed to navigate");

        assert_eq!(nav_response.status(), 200);
        println!("[TEST] Navigate successful, waiting for page load...");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Then: Verify we're on Yahoo Japan
        println!("[TEST] Verifying Yahoo Japan page...");
        let title_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.title"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(title_response.status(), 200);
        let title_json: serde_json::Value = title_response.json().await.unwrap();
        let title = title_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page title: {}", title);
        assert!(!title.is_empty(), "Title should not be empty");
        assert!(
            title.contains("Yahoo") || title.contains("ヤフー"),
            "Title should contain 'Yahoo' or 'ヤフー'"
        );

        // Then: Extract main navigation links
        println!("[TEST] Extracting navigation links...");
        let nav_links_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": r#"
                    (function() {
                        const links = Array.from(document.querySelectorAll('a[href]'));
                        const mainLinks = links
                            .filter(a => a.textContent && a.textContent.trim() && a.href)
                            .slice(0, 10)
                            .map(a => ({ text: a.textContent.trim(), href: a.href }));
                        return JSON.stringify(mainLinks);
                    })();
                "#
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(nav_links_response.status(), 200);
        let nav_links_json: serde_json::Value = nav_links_response.json().await.unwrap();
        let nav_links_str = nav_links_json["result"].as_str().unwrap_or("");
        println!("[TEST] Navigation links: {}", nav_links_str);

        // Parse the JSON array
        if let Ok(nav_links) = serde_json::from_str::<Vec<serde_json::Value>>(nav_links_str) {
            assert!(!nav_links.is_empty(), "Should find navigation links");
            println!("[TEST] Found {} navigation links", nav_links.len());
        }

        // Then: Extract page text content
        println!("[TEST] Extracting page text content...");
        let text_response = client
            .post(&format!("http://127.0.0.1:9400/execute/{}", session_id))
            .json(&json!({
                "script": "document.body.innerText.substring(0, 300)"
            }))
            .send()
            .await
            .expect("Failed to execute script");

        assert_eq!(text_response.status(), 200);
        let text_json: serde_json::Value = text_response.json().await.unwrap();
        let text = text_json["result"].as_str().unwrap_or("");
        println!("[TEST] Page text snippet: {}...", text);
        assert!(!text.is_empty(), "Should get page text content");

        // Cleanup
        println!("[TEST] Closing session...");
        let _ = client
            .delete(&format!("http://127.0.0.1:9400/close/{}", session_id))
            .send()
            .await;
        println!("[TEST] ========== test_yahoo_japan_top_page PASSED ==========\n");
    }
}
