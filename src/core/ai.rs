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
        // Try to load from global config first
        let global_config = crate::core::config::get_config();
        let ai_settings = &global_config.ai;
        
        Self {
            provider: ai_settings.provider.clone(),
            api_key: ai_settings.api_key.clone()
                .or_else(|| std::env::var("WEBVIEW_BRIDGE_AI_API_KEY").ok())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok()),
            model: ai_settings.model.clone(),
            daily_budget_usd: ai_settings.daily_budget_usd,
            daily_usage_usd: 0.0,
            enabled: ai_settings.enabled,
            timeout_ms: ai_settings.timeout_ms,
        }
    }
}

impl AiConfig {
    /// Check if AI is available (has API key for Gemini, or Ollama is running)
    pub fn is_available(&self) -> bool {
        if !self.enabled {
            return false;
        }
        
        match self.provider.to_lowercase().as_str() {
            "ollama" => {
                // Ollama doesn't need API key, just check if running
                let url = std::env::var("OLLAMA_HOST")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                ureq::get(&format!("{}/api/tags", url)).call().is_ok()
            }
            _ => {
                // Gemini needs API key
                self.api_key.is_some()
            }
        }
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

/// Gemini API client with real API integration
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
    
    /// Make a request to Gemini API
    pub fn call(&self, prompt: &str, image_base64: Option<&str>) -> Result<String, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        
        // Build request body based on whether image is provided
        let request_body = if let Some(image_data) = image_base64 {
            // Vision request with image
            serde_json::json!({
                "contents": [{
                    "parts": [
                        {
                            "text": prompt
                        },
                        {
                            "inline_data": {
                                "mime_type": "image/png",
                                "data": image_data
                            }
                        }
                    ]
                }],
                "generationConfig": {
                    "temperature": 0.2,
                    "maxOutputTokens": 4096
                }
            })
        } else {
            // Text-only request
            serde_json::json!({
                "contents": [{
                    "parts": [{
                        "text": prompt
                    }]
                }],
                "generationConfig": {
                    "temperature": 0.2,
                    "maxOutputTokens": 4096
                }
            })
        };
        
        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build();
        
        let response = client
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(&request_body)
            .map_err(|e| format!("Gemini API request failed: {}", e))?;
        
        let response_body: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;
        
        // Extract text from response
        let text = response_body
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                // Check for error in response
                let error = response_body.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                format!("Gemini API error: {}", error)
            })?;
        
        Ok(text.to_string())
    }
    
    /// Analyze screenshot for login form detection
    pub fn analyze_login_form(&self, screenshot_base64: &str) -> Result<LoginFormAnalysis, String> {
        let prompt = r#"
Analyze this webpage screenshot and identify login form elements.
Look for:
1. Username/email input field - identify its likely CSS selector
2. Password input field - identify its likely CSS selector  
3. Login/Submit button - identify its likely CSS selector
4. Any CAPTCHA or verification challenges
5. Current login status (already logged in or not)

Return ONLY valid JSON (no markdown, no explanation) with this exact structure:
{
    "has_login_form": true/false,
    "already_logged_in": true/false,
    "username_selector": "CSS selector string or null",
    "password_selector": "CSS selector string or null", 
    "submit_selector": "CSS selector string or null",
    "captcha_present": true/false,
    "captcha_type": "recaptcha/hcaptcha/image/none",
    "two_factor_field": "CSS selector string or null",
    "error_message_visible": true/false,
    "confidence": 0.0-1.0
}
"#;
        
        let response = self.call(prompt, Some(screenshot_base64))?;
        
        // Parse JSON from response
        let json_str = extract_json_from_response(&response);
        serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse login analysis: {} - Response: {}", e, response))
    }
    
    /// Analyze image for product/content quality
    pub fn analyze_image(&self, image_base64: &str, analysis_type: &ImageAnalysisType, custom_criteria: &[String]) -> Result<ImageAnalysis, String> {
        let prompt = Self::generate_image_analysis_prompt(analysis_type, custom_criteria);
        let response = self.call(&prompt, Some(image_base64))?;
        
        let json_str = extract_json_from_response(&response);
        
        // Parse the analysis result
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse image analysis: {} - Response: {}", e, response))?;
        
        let scores: HashMap<String, f32> = parsed.get("scores")
            .and_then(|s| serde_json::from_value(s.clone()).ok())
            .unwrap_or_default();
        
        Ok(ImageAnalysis {
            image_index: 0,
            scores,
            overall_score: parsed.get("overall_score")
                .and_then(|s| s.as_f64())
                .map(|s| s as f32)
                .unwrap_or(0.0),
            issues: parsed.get("issues")
                .and_then(|i| i.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            improvements: parsed.get("improvements")
                .and_then(|i| i.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            improved_prompt: parsed.get("improved_prompt")
                .and_then(|p| p.as_str())
                .map(String::from),
        })
    }
    
    /// Extract data from webpage screenshot
    pub fn extract_from_screenshot(&self, screenshot_base64: &str, description: &str, schema: Option<&serde_json::Value>) -> Result<serde_json::Value, String> {
        let prompt = Self::generate_extract_prompt(description, schema);
        let response = self.call(&prompt, Some(screenshot_base64))?;
        
        let json_str = extract_json_from_response(&response);
        serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse extraction result: {} - Response: {}", e, response))
    }
    
    /// Analyze screenshot for login form detection
    pub fn generate_login_prompt(_screenshot_base64: &str) -> String {
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

Return ONLY valid JSON (no markdown) with this structure:
{
    "scores": {"clarity": 0-100, "lighting": 0-100, "background": 0-100, "framing": 0-100, "color": 0-100},
    "overall_score": 0-100,
    "issues": ["issue1", "issue2"],
    "improvements": ["improvement1", "improvement2"]
}
"#.to_string(),
            ImageAnalysisType::Composition => r#"
Analyze the visual composition of this image:
- Rule of thirds adherence
- Visual balance
- Focal point clarity
- Color harmony
- Negative space usage

Return ONLY valid JSON (no markdown) with this structure:
{
    "scores": {"thirds": 0-100, "balance": 0-100, "focal_point": 0-100, "color_harmony": 0-100, "negative_space": 0-100},
    "overall_score": 0-100,
    "issues": ["issue1"],
    "improvements": ["improvement1"]
}
"#.to_string(),
            ImageAnalysisType::BrandConsistency => r#"
Analyze this image for brand consistency:
- Color palette adherence
- Typography consistency
- Visual style matching
- Logo placement and visibility
- Overall brand alignment

Return ONLY valid JSON (no markdown) with this structure:
{
    "scores": {"color_palette": 0-100, "typography": 0-100, "visual_style": 0-100, "logo": 0-100, "alignment": 0-100},
    "overall_score": 0-100,
    "issues": ["issue1"],
    "improvements": ["improvement1"]
}
"#.to_string(),
            ImageAnalysisType::Custom => {
                let criteria_str = custom_criteria.join("\n- ");
                format!(r#"
Analyze this image based on the following custom criteria:
- {}

Return ONLY valid JSON (no markdown) with this structure:
{{
    "scores": {{"criterion1": 0-100, "criterion2": 0-100}},
    "overall_score": 0-100,
    "issues": ["issue1"],
    "improvements": ["improvement1"]
}}
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
Analyze this webpage screenshot and extract the following information:
{}
{}

Return ONLY the extracted data as valid JSON. If multiple items are found, return an array.
No markdown formatting, no explanation, just the JSON.
"#, description, schema_hint)
    }
}

// ============================================================================
// Ollama Client (Local LLM Support)
// ============================================================================

/// Ollama client for local LLM support
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub base_url: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl OllamaClient {
    pub fn new(config: &AiConfig) -> Self {
        // Support custom Ollama URL via environment variable
        let base_url = std::env::var("OLLAMA_HOST")
            .ok()
            .filter(|h| !h.is_empty())
            .map(|h| Self::normalize_host(&h))
            .unwrap_or_else(|| "http://localhost:11434".to_string());

        Self {
            base_url,
            model: config.model.clone(),
            timeout_ms: config.timeout_ms,
        }
    }

    /// Normalize an Ollama host value into a usable HTTP URL.
    /// "0.0.0.0" → "http://127.0.0.1:11434", "0.0.0.0:11434" → "http://127.0.0.1:11434"
    pub fn normalize_host(raw: &str) -> String {
        let s = raw.trim().trim_end_matches('/');
        if s.starts_with("http://") || s.starts_with("https://") {
            return s.replace("://0.0.0.0", "://127.0.0.1");
        }
        let with_scheme = format!("http://{}", s);
        let normalized = with_scheme.replace("://0.0.0.0", "://127.0.0.1");
        if !s.contains(':') {
            format!("{}:11434", normalized)
        } else {
            normalized
        }
    }
    
    /// Make a request to Ollama API
    pub fn call(&self, prompt: &str, image_base64: Option<&str>) -> Result<String, String> {
        let url = format!("{}/api/generate", self.base_url);
        
        // Build request with or without images
        let request_body = if let Some(image_data) = image_base64 {
            // Vision request with image
            serde_json::json!({
                "model": self.model,
                "prompt": prompt,
                "images": [image_data],
                "stream": false,
                "options": {
                    "temperature": 0.2,
                    "num_predict": 4096
                }
            })
        } else {
            // Text-only request
            serde_json::json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": 0.2,
                    "num_predict": 4096
                }
            })
        };
        
        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build();
        
        let response = client
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(&request_body)
            .map_err(|e| format!("Ollama API request failed: {}. Is Ollama running?", e))?;
        
        let response_body: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
        
        // Extract response text
        response_body
            .get("response")
            .and_then(|r| r.as_str())
            .map(String::from)
            .ok_or_else(|| {
                let error = response_body.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Unknown error");
                format!("Ollama API error: {}", error)
            })
    }
    
    /// List available models
    pub fn list_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/tags", self.base_url);
        
        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(5))
            .build();
        
        let response = client
            .get(&url)
            .call()
            .map_err(|e| format!("Failed to list Ollama models: {}", e))?;
        
        let body: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse model list: {}", e))?;
        
        let models = body
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        
        Ok(models)
    }
    
    /// Check if Ollama is running (with timeout)
    pub fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(5))
            .build();
        client.get(&url).call().is_ok()
    }
}

// ============================================================================
// Unified AI Client
// ============================================================================

/// Unified AI client that automatically selects provider
pub enum AiClient {
    Gemini(GeminiClient),
    Ollama(OllamaClient),
}

impl AiClient {
    /// Create client based on configuration
    pub fn new(config: &AiConfig) -> Option<Self> {
        match config.provider.to_lowercase().as_str() {
            "ollama" => {
                let client = OllamaClient::new(config);
                if client.is_available() {
                    Some(AiClient::Ollama(client))
                } else {
                    tracing::warn!("Ollama not available, falling back to Gemini");
                    GeminiClient::new(config).map(AiClient::Gemini)
                }
            }
            "gemini" | _ => {
                // Try Gemini first, fall back to Ollama if no API key
                if let Some(gemini) = GeminiClient::new(config) {
                    Some(AiClient::Gemini(gemini))
                } else {
                    // No Gemini API key, try Ollama
                    let ollama = OllamaClient::new(config);
                    if ollama.is_available() {
                        tracing::info!("No Gemini API key, using Ollama");
                        Some(AiClient::Ollama(ollama))
                    } else {
                        None
                    }
                }
            }
        }
    }
    
    /// Make a request to the AI provider
    pub fn call(&self, prompt: &str, image_base64: Option<&str>) -> Result<String, String> {
        match self {
            AiClient::Gemini(client) => client.call(prompt, image_base64),
            AiClient::Ollama(client) => client.call(prompt, image_base64),
        }
    }
    
    /// Get provider name
    pub fn provider_name(&self) -> &str {
        match self {
            AiClient::Gemini(_) => "gemini",
            AiClient::Ollama(_) => "ollama",
        }
    }
}

/// Login form analysis result from AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFormAnalysis {
    pub has_login_form: bool,
    #[serde(default)]
    pub already_logged_in: bool,
    pub username_selector: Option<String>,
    pub password_selector: Option<String>,
    pub submit_selector: Option<String>,
    #[serde(default)]
    pub captcha_present: bool,
    #[serde(default)]
    pub captcha_type: Option<String>,
    #[serde(default)]
    pub two_factor_field: Option<String>,
    #[serde(default)]
    pub error_message_visible: bool,
    #[serde(default)]
    pub confidence: f32,
}

/// Extract JSON from AI response, handling markdown code blocks
fn extract_json_from_response(response: &str) -> String {
    let response = response.trim();
    
    // Handle markdown code blocks
    if response.starts_with("```json") {
        let start = response.find('\n').unwrap_or(7) + 1;
        let end = response.rfind("```").unwrap_or(response.len());
        return response[start..end].trim().to_string();
    }
    if response.starts_with("```") {
        let start = response.find('\n').unwrap_or(3) + 1;
        let end = response.rfind("```").unwrap_or(response.len());
        return response[start..end].trim().to_string();
    }
    
    // Try to find JSON object or array
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return response[start..=end].to_string();
        }
    }
    if let Some(start) = response.find('[') {
        if let Some(end) = response.rfind(']') {
            return response[start..=end].to_string();
        }
    }
    
    response.to_string()
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
    fn test_ai_config_is_available() {
        let mut config = AiConfig::default();
        config.api_key = None;
        assert!(!config.is_available(), "Should not be available without API key");
        
        config.api_key = Some("test-key".to_string());
        assert!(config.is_available(), "Should be available with API key");
        
        config.enabled = false;
        assert!(!config.is_available(), "Should not be available when disabled");
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
    fn test_budget_no_limit() {
        let mut config = AiConfig::default();
        config.daily_budget_usd = None;
        
        // No limit should always allow
        assert!(config.can_use(1000000.0));
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
    fn test_mask_credit_cards_no_spaces() {
        let text = "Card: 1234567890123456";
        let masked = SensitiveDataMasker::mask_credit_cards(text);
        assert!(masked.contains("****"));
    }
    
    #[test]
    fn test_mask_credit_cards_multiple() {
        let text = "Card1: 1111 2222 3333 4444 Card2: 5555 6666 7777 8888";
        let masked = SensitiveDataMasker::mask_credit_cards(text);
        assert!(masked.contains("1111"));
        assert!(masked.contains("5555"));
        assert!(!masked.contains("4444"));
        assert!(!masked.contains("8888"));
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
    fn test_mask_emails_short() {
        let text = "Email: ab@test.org";
        let masked = SensitiveDataMasker::mask_emails(text);
        assert!(masked.contains("ab"));
        assert!(masked.contains("@test.org"));
    }
    
    #[test]
    fn test_mask_emails_multiple() {
        let text = "alice@example.com and bob.smith@company.co.jp";
        let masked = SensitiveDataMasker::mask_emails(text);
        assert!(masked.contains("al****@example.com"));
        assert!(masked.contains("bo****@company.co.jp"));
    }
    
    #[test]
    fn test_mask_all_combined() {
        let text = "Email: user@example.com, Card: 1234 5678 9012 3456";
        let masked = SensitiveDataMasker::mask_all(text);
        assert!(masked.contains("****"));
        assert!(!masked.contains("3456")); // Card masked
        assert!(!masked.contains("user@")); // Email masked
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
    
    #[test]
    fn test_gemini_prompt_composition() {
        let prompt = GeminiClient::generate_image_analysis_prompt(
            &ImageAnalysisType::Composition,
            &[]
        );
        assert!(prompt.contains("composition"));
        assert!(prompt.contains("Rule of thirds"));
    }
    
    #[test]
    fn test_gemini_prompt_brand() {
        let prompt = GeminiClient::generate_image_analysis_prompt(
            &ImageAnalysisType::BrandConsistency,
            &[]
        );
        assert!(prompt.contains("brand"));
        assert!(prompt.contains("Logo"));
    }
    
    #[test]
    fn test_gemini_prompt_custom() {
        let criteria = vec!["背景の清潔さ".to_string(), "商品の見栄え".to_string()];
        let prompt = GeminiClient::generate_image_analysis_prompt(
            &ImageAnalysisType::Custom,
            &criteria
        );
        assert!(prompt.contains("背景の清潔さ"));
        assert!(prompt.contains("商品の見栄え"));
    }
    
    #[test]
    fn test_gemini_extract_prompt() {
        let prompt = GeminiClient::generate_extract_prompt(
            "商品名と価格",
            None
        );
        assert!(prompt.contains("商品名と価格"));
        assert!(prompt.contains("JSON"));
    }
    
    #[test]
    fn test_gemini_extract_prompt_with_schema() {
        let schema = serde_json::json!({
            "name": "string",
            "price": "number"
        });
        let prompt = GeminiClient::generate_extract_prompt(
            "商品情報",
            Some(&schema)
        );
        assert!(prompt.contains("schema"));
        assert!(prompt.contains("name"));
    }
    
    #[test]
    fn test_usage_tracker() {
        let mut tracker = AiUsageTracker::new();
        
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
        
        assert_eq!(tracker.total_requests, 1);
        assert_eq!(tracker.total_input_tokens, 100);
        assert_eq!(tracker.total_output_tokens, 50);
        assert!((tracker.total_cost_usd - 0.001).abs() < 0.0001);
        
        let stats = tracker.get_daily_stats();
        assert_eq!(stats["total_requests"], 1);
    }
    
    #[test]
    fn test_login_status_serialization() {
        let status = LoginStatus::Success;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("success"));
        
        let status = LoginStatus::CaptchaDetected;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("captcha_detected"));
    }
    
    #[test]
    fn test_image_analysis_type_default() {
        let analysis_type = ImageAnalysisType::default();
        assert_eq!(analysis_type, ImageAnalysisType::ProductQuality);
    }
    
    #[test]
    fn test_gemini_client_creation() {
        let mut config = AiConfig::default();
        config.api_key = None;
        
        assert!(GeminiClient::new(&config).is_none());
        
        config.api_key = Some("test-key".to_string());
        let client = GeminiClient::new(&config);
        assert!(client.is_some());
        assert_eq!(client.unwrap().api_key, "test-key");
    }
}
