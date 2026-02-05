//! WBP2 AI Integration Module
//!
//! AI-powered automation with Gemini for login assistance,
//! image analysis, and dynamic page extraction.
//! See: Plans.md Phase 14

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// AI Configuration (14.1)
// ============================================================================

/// AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// AI provider (currently only "gemini")
    #[serde(default = "default_provider")]
    pub provider: String,
    
    /// API key (can be overridden by WEBVIEW_BRIDGE_AI_API_KEY env var)
    #[serde(default)]
    pub api_key: Option<String>,
    
    /// Model name
    #[serde(default = "default_model")]
    pub model: String,
    
    /// Daily budget limit in USD
    #[serde(default)]
    pub daily_budget_usd: Option<f32>,
    
    /// Current daily usage in USD
    #[serde(default)]
    pub daily_usage_usd: f32,
    
    /// Enable AI features
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Timeout for AI requests in ms
    #[serde(default = "default_ai_timeout")]
    pub timeout_ms: u64,
}

fn default_provider() -> String {
    "gemini".to_string()
}

fn default_model() -> String {
    "gemini-2.0-flash".to_string()
}

fn default_true() -> bool {
    true
}

fn default_ai_timeout() -> u64 {
    30000
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: std::env::var("WEBVIEW_BRIDGE_AI_API_KEY").ok(),
            model: default_model(),
            daily_budget_usd: None,
            daily_usage_usd: 0.0,
            enabled: true,
            timeout_ms: default_ai_timeout(),
        }
    }
}

impl AiConfig {
    /// Check if AI is available (has API key and is enabled)
    pub fn is_available(&self) -> bool {
        self.enabled && self.api_key.is_some()
    }
    
    /// Check if budget allows usage
    pub fn can_use(&self, estimated_cost: f32) -> bool {
        match self.daily_budget_usd {
            Some(budget) => self.daily_usage_usd + estimated_cost <= budget,
            None => true, // No budget limit
        }
    }
    
    /// Get API key from config or environment
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key.clone()
            .or_else(|| std::env::var("WEBVIEW_BRIDGE_AI_API_KEY").ok())
    }
}

// ============================================================================
// AI Login Assistance (14.2)
// ============================================================================

/// Request for POST /v2/ai/login
#[derive(Debug, Clone, Deserialize)]
pub struct AiLoginRequest {
    /// Session name
    pub session: String,
    
    /// Credentials
    pub credentials: LoginCredentials,
    
    /// Target URL (optional, uses current page if not specified)
    #[serde(default)]
    pub url: Option<String>,
    
    /// Wait for login success
    #[serde(default = "default_true")]
    pub wait_for_success: bool,
    
    /// Timeout in ms
    #[serde(default = "default_login_timeout")]
    pub timeout_ms: u64,
}

fn default_login_timeout() -> u64 {
    60000
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    /// Optional 2FA code or handler
    #[serde(default)]
    pub two_factor: Option<TwoFactorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorConfig {
    /// Static code (for testing)
    pub code: Option<String>,
    /// Webhook to call for 2FA code
    pub webhook: Option<String>,
    /// Wait for user input timeout
    pub wait_timeout_ms: Option<u64>,
}

/// Response for AI login
#[derive(Debug, Clone, Serialize)]
pub struct AiLoginResponse {
    pub success: bool,
    pub status: LoginStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_elements: Option<DetectedElements>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LoginStatus {
    /// Login successful
    Success,
    /// Login form detected, filling in progress
    InProgress,
    /// CAPTCHA detected, requires human intervention
    CaptchaDetected,
    /// 2FA required
    TwoFactorRequired,
    /// Login failed (wrong credentials)
    Failed,
    /// Could not detect login form
    FormNotFound,
    /// Timeout
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectedElements {
    pub username_field: Option<String>,
    pub password_field: Option<String>,
    pub submit_button: Option<String>,
    pub captcha_present: bool,
    pub two_factor_field: Option<String>,
}

// ============================================================================
// AI Image Analysis (14.3)
// ============================================================================

/// Request for POST /v2/ai/images/analyze
#[derive(Debug, Clone, Deserialize)]
pub struct AiImageAnalyzeRequest {
    /// Image URLs or base64 data
    pub images: Vec<ImageInput>,
    
    /// Analysis type
    #[serde(default)]
    pub analysis_type: ImageAnalysisType,
    
    /// Custom criteria for evaluation
    #[serde(default)]
    pub custom_criteria: Vec<String>,
    
    /// Compare with competitor images
    #[serde(default)]
    pub competitor_images: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageInput {
    Url(String),
    Base64 { base64: String, mime_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImageAnalysisType {
    /// Product image quality evaluation
    #[default]
    ProductQuality,
    /// Visual composition analysis
    Composition,
    /// Brand consistency check
    BrandConsistency,
    /// Custom analysis with criteria
    Custom,
}

/// Response for image analysis
#[derive(Debug, Clone, Serialize)]
pub struct AiImageAnalyzeResponse {
    pub success: bool,
    pub analyses: Vec<ImageAnalysis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub competitor_comparison: Option<CompetitorComparison>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageAnalysis {
    pub image_index: usize,
    pub scores: HashMap<String, f32>,
    pub overall_score: f32,
    pub issues: Vec<String>,
    pub improvements: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improved_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompetitorComparison {
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub recommendations: Vec<String>,
}

// ============================================================================
// AI Dynamic Page Extraction (14.4)
// ============================================================================

/// Request for POST /v2/ai/extract
#[derive(Debug, Clone, Deserialize)]
pub struct AiExtractRequest {
    /// Session name
    pub session: String,
    
    /// Natural language description of what to extract
    pub description: String,
    
    /// Expected output schema (optional)
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    
    /// Enable auto-scroll
    #[serde(default = "default_true")]
    pub auto_scroll: bool,
    
    /// Maximum scroll iterations
    #[serde(default = "default_max_scroll")]
    pub max_scroll: u32,
    
    /// Timeout in ms
    #[serde(default = "default_extract_timeout")]
    pub timeout_ms: u64,
}

fn default_max_scroll() -> u32 {
    10
}

fn default_extract_timeout() -> u64 {
    60000
}

/// Response for AI extraction
#[derive(Debug, Clone, Serialize)]
pub struct AiExtractResponse {
    pub success: bool,
    /// Extracted data
    pub data: serde_json::Value,
    /// Items extracted
    pub item_count: usize,
    /// Pages/scrolls processed
    pub pages_processed: u32,
    pub elapsed_ms: u64,
}

// ============================================================================
// Privacy & Security (14.5)
// ============================================================================

/// Sensitive data patterns for masking
pub struct SensitiveDataMasker;

impl SensitiveDataMasker {
    /// Mask password input fields in HTML
    pub fn mask_password_fields(html: &str) -> String {
        // Replace password field values
        let re = regex::Regex::new(r#"(<input[^>]*type\s*=\s*["']password["'][^>]*value\s*=\s*["'])[^"']*["']"#).unwrap();
        re.replace_all(html, "${1}********\"").to_string()
    }
    
    /// Mask credit card numbers
    pub fn mask_credit_cards(text: &str) -> String {
        let re = regex::Regex::new(r"\b(\d{4})\s?(\d{4})\s?(\d{4})\s?(\d{4})\b").unwrap();
        re.replace_all(text, "$1 **** **** ****").to_string()
    }
    
    /// Mask email addresses (partial)
    pub fn mask_emails(text: &str) -> String {
        let re = regex::Regex::new(r"\b([a-zA-Z0-9._%+-]{2})[a-zA-Z0-9._%+-]*@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})\b").unwrap();
        re.replace_all(text, "$1****@$2").to_string()
    }
    
    /// Apply all masks
    pub fn mask_all(text: &str) -> String {
        let text = Self::mask_password_fields(text);
        let text = Self::mask_credit_cards(&text);
        Self::mask_emails(&text)
    }
}

// ============================================================================
// Gemini API Integration
// ============================================================================

/// Gemini API client placeholder
#[derive(Debug, Clone)]
pub struct GeminiClient {
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl GeminiClient {
    pub fn new(config: &AiConfig) -> Option<Self> {
        config.get_api_key().map(|api_key| Self {
            api_key,
            model: config.model.clone(),
            timeout_ms: config.timeout_ms,
        })
    }
    
    /// Analyze screenshot for login form detection
    pub fn generate_login_prompt(screenshot_base64: &str) -> String {
        format!(r#"
Analyze this webpage screenshot and identify login form elements.
Return JSON with the following structure:
{{
    "has_login_form": boolean,
    "username_selector": "CSS selector or null",
    "password_selector": "CSS selector or null",
    "submit_selector": "CSS selector or null",
    "captcha_present": boolean,
    "two_factor_visible": boolean,
    "login_status": "logged_out" | "logged_in" | "unknown"
}}

Only return the JSON, no other text.
"#)
    }
    
    /// Generate image analysis prompt
    pub fn generate_image_analysis_prompt(analysis_type: &ImageAnalysisType, custom_criteria: &[String]) -> String {
        let base_prompt = match analysis_type {
            ImageAnalysisType::ProductQuality => r#"
Analyze this product image for e-commerce quality. Evaluate:
- Image clarity and resolution
- Lighting and shadows
- Background cleanliness
- Product visibility and framing
- Color accuracy

Return JSON with scores (0-100) for each criterion, overall score, issues list, and improvements list.
"#.to_string(),
            ImageAnalysisType::Composition => r#"
Analyze the visual composition of this image:
- Rule of thirds adherence
- Visual balance
- Focal point clarity
- Color harmony
- Negative space usage

Return JSON with scores (0-100) for each criterion, overall score, and composition suggestions.
"#.to_string(),
            ImageAnalysisType::BrandConsistency => r#"
Analyze this image for brand consistency:
- Color palette adherence
- Typography consistency
- Visual style matching
- Logo placement and visibility
- Overall brand alignment

Return JSON with scores (0-100) for each criterion and brand consistency notes.
"#.to_string(),
            ImageAnalysisType::Custom => {
                let criteria_str = custom_criteria.join("\n- ");
                format!(r#"
Analyze this image based on the following custom criteria:
- {}

Return JSON with scores (0-100) for each criterion, overall score, issues, and improvements.
"#, criteria_str)
            }
        };
        
        base_prompt
    }
    
    /// Generate extraction prompt
    pub fn generate_extract_prompt(description: &str, schema: Option<&serde_json::Value>) -> String {
        let schema_hint = schema
            .map(|s| format!("\n\nExpected output schema:\n```json\n{}\n```", serde_json::to_string_pretty(s).unwrap_or_default()))
            .unwrap_or_default();
        
        format!(r#"
Analyze this webpage and extract the following information:
{}
{}

Return the extracted data as valid JSON. If multiple items are found, return an array.
Only return the JSON, no other text.
"#, description, schema_hint)
    }
}

// ============================================================================
// AI Request/Response Logging
// ============================================================================

/// AI request log entry
#[derive(Debug, Clone, Serialize)]
pub struct AiLogEntry {
    pub timestamp: String,
    pub request_type: String,
    pub model: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub estimated_cost_usd: Option<f32>,
    pub success: bool,
    pub error: Option<String>,
}

/// AI usage tracker
#[derive(Debug, Clone, Default)]
pub struct AiUsageTracker {
    pub entries: Vec<AiLogEntry>,
    pub total_requests: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cost_usd: f32,
}

impl AiUsageTracker {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn log(&mut self, entry: AiLogEntry) {
        self.total_requests += 1;
        if let Some(input) = entry.input_tokens {
            self.total_input_tokens += input;
        }
        if let Some(output) = entry.output_tokens {
            self.total_output_tokens += output;
        }
        if let Some(cost) = entry.estimated_cost_usd {
            self.total_cost_usd += cost;
        }
        self.entries.push(entry);
    }
    
    pub fn get_daily_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "total_requests": self.total_requests,
            "total_input_tokens": self.total_input_tokens,
            "total_output_tokens": self.total_output_tokens,
            "total_cost_usd": self.total_cost_usd
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ai_config_default() {
        let config = AiConfig::default();
        assert_eq!(config.provider, "gemini");
        assert_eq!(config.model, "gemini-2.0-flash");
        assert!(config.enabled);
    }
    
    #[test]
    fn test_budget_check() {
        let mut config = AiConfig::default();
        config.daily_budget_usd = Some(1.0);
        config.daily_usage_usd = 0.5;
        
        assert!(config.can_use(0.3));
        assert!(config.can_use(0.5));
        assert!(!config.can_use(0.6));
    }
    
    #[test]
    fn test_mask_credit_cards() {
        let text = "Card: 1234 5678 9012 3456";
        let masked = SensitiveDataMasker::mask_credit_cards(text);
        assert!(masked.contains("****"));
        assert!(masked.contains("1234"));
        assert!(!masked.contains("3456"));
    }
    
    #[test]
    fn test_mask_emails() {
        let text = "Email: john.doe@example.com";
        let masked = SensitiveDataMasker::mask_emails(text);
        assert!(masked.contains("****"));
        assert!(masked.contains("jo"));
        assert!(masked.contains("@example.com"));
    }
    
    #[test]
    fn test_gemini_prompts() {
        let login_prompt = GeminiClient::generate_login_prompt("base64data");
        assert!(login_prompt.contains("login form"));
        
        let image_prompt = GeminiClient::generate_image_analysis_prompt(
            &ImageAnalysisType::ProductQuality,
            &[]
        );
        assert!(image_prompt.contains("product image"));
    }
}
