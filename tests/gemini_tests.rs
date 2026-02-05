//! Gemini API Integration Tests
//!
//! Tests against the actual Gemini API with fallback when API key is not available.

use serde_json::{json, Value};
use std::env;

/// Check if Gemini API is available
fn gemini_available() -> bool {
    env::var("GEMINI_API_KEY").is_ok()
}

/// Get Gemini API key
fn get_api_key() -> Option<String> {
    env::var("GEMINI_API_KEY").ok()
}

// ============================================================================
// Credential Masking Tests (Always Run)
// ============================================================================

#[test]
fn test_password_masking() {
    use webview_bridge_rust::core::ai::SensitiveDataMasker;
    
    let html = r#"<input type="password" name="pass" value="secret123">"#;
    let masked = SensitiveDataMasker::mask_password_fields(html);
    
    assert!(!masked.contains("secret123"), "Password should be masked");
    assert!(masked.contains("********"), "Should contain mask");
}

#[test]
fn test_credit_card_masking() {
    use webview_bridge_rust::core::ai::SensitiveDataMasker;
    
    let text = "Card number: 4111 1111 1111 1111";
    let masked = SensitiveDataMasker::mask_credit_cards(text);
    
    assert!(!masked.contains("1111 1111"), "Full card should be masked");
    assert!(masked.contains("4111"), "First 4 digits should be visible");
    assert!(masked.contains("****"), "Should contain mask");
}

#[test]
fn test_email_masking() {
    use webview_bridge_rust::core::ai::SensitiveDataMasker;
    
    let text = "Contact: john.doe@example.com";
    let masked = SensitiveDataMasker::mask_emails(text);
    
    assert!(!masked.contains("john.doe@"), "Full email should be masked");
    assert!(masked.contains("@example.com"), "Domain should be visible");
}

// ============================================================================
// AI Config Tests
// ============================================================================

#[test]
fn test_ai_config_defaults() {
    use webview_bridge_rust::core::ai::AiConfig;
    
    let config = AiConfig::default();
    
    assert_eq!(config.provider, "gemini");
    assert_eq!(config.model, "gemini-2.0-flash");
    assert!(config.enabled);
    assert_eq!(config.timeout_ms, 30000);
}

#[test]
fn test_ai_availability_check() {
    use webview_bridge_rust::core::ai::AiConfig;
    
    let mut config = AiConfig::default();
    config.api_key = None;
    config.enabled = true;
    
    assert!(!config.is_available(), "Should not be available without API key");
    
    config.api_key = Some("test-key".to_string());
    assert!(config.is_available(), "Should be available with API key");
    
    config.enabled = false;
    assert!(!config.is_available(), "Should not be available when disabled");
}

#[test]
fn test_budget_enforcement() {
    use webview_bridge_rust::core::ai::AiConfig;
    
    let mut config = AiConfig::default();
    config.daily_budget_usd = Some(1.0);
    config.daily_usage_usd = 0.8;
    
    assert!(config.can_use(0.1), "Should allow within budget");
    assert!(config.can_use(0.2), "Should allow exact budget");
    assert!(!config.can_use(0.3), "Should reject over budget");
}

// ============================================================================
// Prompt Generation Tests
// ============================================================================

#[test]
fn test_login_prompt_generation() {
    use webview_bridge_rust::core::ai::GeminiClient;
    
    let prompt = GeminiClient::generate_login_prompt("base64data");
    
    assert!(prompt.contains("login form"));
    assert!(prompt.contains("JSON"));
    assert!(prompt.contains("username_selector"));
    assert!(prompt.contains("password_selector"));
}

#[test]
fn test_image_analysis_prompt_product() {
    use webview_bridge_rust::core::ai::{GeminiClient, ImageAnalysisType};
    
    let prompt = GeminiClient::generate_image_analysis_prompt(
        &ImageAnalysisType::ProductQuality,
        &[]
    );
    
    assert!(prompt.contains("product image"));
    assert!(prompt.contains("e-commerce"));
    assert!(prompt.contains("quality"));
}

#[test]
fn test_image_analysis_prompt_custom() {
    use webview_bridge_rust::core::ai::{GeminiClient, ImageAnalysisType};
    
    let criteria = vec![
        "Background clarity".to_string(),
        "Product visibility".to_string(),
    ];
    let prompt = GeminiClient::generate_image_analysis_prompt(
        &ImageAnalysisType::Custom,
        &criteria
    );
    
    assert!(prompt.contains("Background clarity"));
    assert!(prompt.contains("Product visibility"));
}

#[test]
fn test_extract_prompt_with_schema() {
    use webview_bridge_rust::core::ai::GeminiClient;
    
    let schema = json!({
        "products": [{
            "name": "string",
            "price": "number",
            "in_stock": "boolean"
        }]
    });
    
    let prompt = GeminiClient::generate_extract_prompt(
        "Extract all products from the page",
        Some(&schema)
    );
    
    assert!(prompt.contains("products"));
    assert!(prompt.contains("name"));
    assert!(prompt.contains("schema"));
}

// ============================================================================
// Usage Tracking Tests
// ============================================================================

#[test]
fn test_usage_tracker_accumulation() {
    use webview_bridge_rust::core::ai::{AiUsageTracker, AiLogEntry};
    
    let mut tracker = AiUsageTracker::new();
    
    // Add first entry
    tracker.log(AiLogEntry {
        timestamp: "2026-02-05T12:00:00Z".to_string(),
        request_type: "login".to_string(),
        model: "gemini-2.0-flash".to_string(),
        input_tokens: Some(100),
        output_tokens: Some(50),
        estimated_cost_usd: Some(0.001),
        success: true,
        error: None,
    });
    
    // Add second entry
    tracker.log(AiLogEntry {
        timestamp: "2026-02-05T12:01:00Z".to_string(),
        request_type: "extract".to_string(),
        model: "gemini-2.0-flash".to_string(),
        input_tokens: Some(200),
        output_tokens: Some(100),
        estimated_cost_usd: Some(0.002),
        success: true,
        error: None,
    });
    
    assert_eq!(tracker.total_requests, 2);
    assert_eq!(tracker.total_input_tokens, 300);
    assert_eq!(tracker.total_output_tokens, 150);
    assert!((tracker.total_cost_usd - 0.003).abs() < 0.0001);
}

#[test]
fn test_usage_tracker_stats() {
    use webview_bridge_rust::core::ai::{AiUsageTracker, AiLogEntry};
    
    let mut tracker = AiUsageTracker::new();
    
    tracker.log(AiLogEntry {
        timestamp: "2026-02-05T12:00:00Z".to_string(),
        request_type: "analyze".to_string(),
        model: "gemini-2.0-flash".to_string(),
        input_tokens: Some(500),
        output_tokens: Some(200),
        estimated_cost_usd: Some(0.005),
        success: true,
        error: None,
    });
    
    let stats = tracker.get_daily_stats();
    
    assert_eq!(stats["total_requests"], 1);
    assert_eq!(stats["total_input_tokens"], 500);
    assert_eq!(stats["total_output_tokens"], 200);
}

// ============================================================================
// Live API Tests (Require GEMINI_API_KEY)
// ============================================================================

#[tokio::test]
#[ignore] // Requires API key
async fn test_gemini_api_connection() {
    if !gemini_available() {
        eprintln!("Skipping: GEMINI_API_KEY not set");
        return;
    }
    
    let api_key = get_api_key().unwrap();
    
    // Simple API health check
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            api_key
        ))
        .send()
        .await;
    
    assert!(response.is_ok(), "Failed to connect to Gemini API");
    let response = response.unwrap();
    assert!(response.status().is_success(), "API returned error: {}", response.status());
}

#[tokio::test]
#[ignore] // Requires API key and makes actual API call
async fn test_gemini_simple_generation() {
    if !gemini_available() {
        eprintln!("Skipping: GEMINI_API_KEY not set");
        return;
    }
    
    let api_key = get_api_key().unwrap();
    
    let client = reqwest::Client::new();
    let response = client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            api_key
        ))
        .json(&json!({
            "contents": [{
                "parts": [{"text": "Return only the word 'hello' in lowercase"}]
            }]
        }))
        .send()
        .await;
    
    assert!(response.is_ok(), "Failed to send request");
    let response = response.unwrap();
    assert!(response.status().is_success(), "API returned error: {}", response.status());
    
    let body: Value = response.json().await.unwrap();
    let text = body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("");
    
    assert!(text.to_lowercase().contains("hello"), "Expected 'hello' in response: {}", text);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_login_status_variants() {
    use webview_bridge_rust::core::ai::LoginStatus;
    
    let statuses = vec![
        LoginStatus::Success,
        LoginStatus::InProgress,
        LoginStatus::CaptchaDetected,
        LoginStatus::TwoFactorRequired,
        LoginStatus::Failed,
        LoginStatus::FormNotFound,
        LoginStatus::Timeout,
    ];
    
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.is_empty(), "Status should serialize");
    }
}

#[test]
fn test_image_analysis_type_variants() {
    use webview_bridge_rust::core::ai::ImageAnalysisType;
    
    let types = vec![
        ImageAnalysisType::ProductQuality,
        ImageAnalysisType::Composition,
        ImageAnalysisType::BrandConsistency,
        ImageAnalysisType::Custom,
    ];
    
    for t in types {
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.is_empty(), "Type should serialize");
    }
}
