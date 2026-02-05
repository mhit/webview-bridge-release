//! WBP2 Goal API Module - Declarative goal-based automation
//!
//! High-level goal parsing and execution with auto-retry.
//! See: Plans.md Phase 10

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Goal Types
// ============================================================================

/// Goal type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    /// Navigate to a URL
    Navigate,
    /// Click on an element
    Click,
    /// Fill a form field
    Fill,
    /// Submit a form
    Submit,
    /// Wait for something
    Wait,
    /// Extract data
    Extract,
    /// Login to a site
    Login,
    /// Search for something
    Search,
    /// Scroll the page
    Scroll,
    /// Take a screenshot
    Screenshot,
    /// Custom/composite goal
    Custom,
}

/// Request for POST /v2/goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRequest {
    /// Session name
    pub session: String,
    
    /// Goal type
    #[serde(rename = "type")]
    pub goal_type: GoalType,
    
    /// Goal target (selector, URL, text, etc.)
    pub target: String,
    
    /// Additional parameters
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    
    /// Retry configuration
    #[serde(default)]
    pub retry: RetryConfig,
    
    /// Timeout in ms
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    
    /// Chain of goals to execute in sequence
    #[serde(default)]
    pub chain: Option<Vec<GoalRequest>>,
}

fn default_timeout() -> u64 {
    30000
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial delay in ms
    #[serde(default = "default_initial_delay")]
    pub initial_delay_ms: u64,
    /// Delay multiplier (exponential backoff)
    #[serde(default = "default_multiplier")]
    pub multiplier: f32,
    /// Maximum delay in ms
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u64,
    /// Retry on these errors only
    #[serde(default)]
    pub retry_on: Vec<String>,
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay() -> u64 {
    1000
}

fn default_multiplier() -> f32 {
    2.0
}

fn default_max_delay() -> u64 {
    10000
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay_ms: default_initial_delay(),
            multiplier: default_multiplier(),
            max_delay_ms: default_max_delay(),
            retry_on: vec![
                "element_not_found".to_string(),
                "timeout".to_string(),
                "network_error".to_string(),
            ],
        }
    }
}

/// Response for POST /v2/goal
#[derive(Debug, Clone, Serialize)]
pub struct GoalResponse {
    pub success: bool,
    pub goal_type: String,
    pub target: String,
    /// Result data (extracted data, screenshot, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Attempts made
    pub attempts: u32,
    /// Time taken in ms
    pub elapsed_ms: u64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Chain results if chain was specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_results: Option<Vec<GoalResponse>>,
}

// ============================================================================
// Error Classification
// ============================================================================

/// Error category for retry logic
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// Transient error - retry may help
    Transient,
    /// Permanent error - no point retrying
    Permanent,
    /// Unknown - treat as transient
    Unknown,
}

/// Classify an error for retry decision
pub fn classify_error(error: &str) -> ErrorCategory {
    let error_lower = error.to_lowercase();
    
    // Permanent errors - don't retry
    if error_lower.contains("invalid selector") ||
       error_lower.contains("syntax error") ||
       error_lower.contains("unauthorized") ||
       error_lower.contains("forbidden") ||
       error_lower.contains("not found") && error_lower.contains("page") ||
       error_lower.contains("session closed") {
        return ErrorCategory::Permanent;
    }
    
    // Transient errors - retry
    if error_lower.contains("timeout") ||
       error_lower.contains("element not found") ||
       error_lower.contains("not visible") ||
       error_lower.contains("network") ||
       error_lower.contains("connection") ||
       error_lower.contains("loading") ||
       error_lower.contains("stale") {
        return ErrorCategory::Transient;
    }
    
    ErrorCategory::Unknown
}

/// Check if error should be retried based on config
pub fn should_retry(error: &str, config: &RetryConfig) -> bool {
    let category = classify_error(error);
    
    match category {
        ErrorCategory::Permanent => false,
        ErrorCategory::Transient => true,
        ErrorCategory::Unknown => {
            // Check if error matches retry_on list
            let error_lower = error.to_lowercase();
            config.retry_on.iter().any(|r| error_lower.contains(&r.to_lowercase()))
        }
    }
}

/// Calculate delay for retry attempt
pub fn calculate_delay(attempt: u32, config: &RetryConfig) -> u64 {
    let delay = config.initial_delay_ms as f64 * config.multiplier.powi(attempt as i32) as f64;
    std::cmp::min(delay as u64, config.max_delay_ms)
}

// ============================================================================
// Preset Flows
// ============================================================================

/// Preset flow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetFlow {
    pub name: String,
    pub description: String,
    pub steps: Vec<GoalRequest>,
}

/// Get built-in preset flows
pub fn get_preset_flows() -> Vec<PresetFlow> {
    vec![
        // Login flow
        PresetFlow {
            name: "login".to_string(),
            description: "Standard login flow with username and password".to_string(),
            steps: vec![
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Wait,
                    target: "form".to_string(),
                    params: HashMap::new(),
                    retry: RetryConfig::default(),
                    timeout_ms: 10000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Fill,
                    target: "input[type='email'], input[name='username'], input[name='email'], #email, #username".to_string(),
                    params: [("value".to_string(), serde_json::json!("{{username}}"))].into_iter().collect(),
                    retry: RetryConfig::default(),
                    timeout_ms: 5000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Fill,
                    target: "input[type='password'], input[name='password'], #password".to_string(),
                    params: [("value".to_string(), serde_json::json!("{{password}}"))].into_iter().collect(),
                    retry: RetryConfig::default(),
                    timeout_ms: 5000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Click,
                    target: "button[type='submit'], input[type='submit'], button:contains('Login'), button:contains('Sign in')".to_string(),
                    params: HashMap::new(),
                    retry: RetryConfig::default(),
                    timeout_ms: 5000,
                    chain: None,
                },
            ],
        },
        // Search flow
        PresetFlow {
            name: "search".to_string(),
            description: "Standard search flow".to_string(),
            steps: vec![
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Wait,
                    target: "input[type='search'], input[name='q'], input[name='query'], #search, .search-input".to_string(),
                    params: HashMap::new(),
                    retry: RetryConfig::default(),
                    timeout_ms: 10000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Fill,
                    target: "input[type='search'], input[name='q'], input[name='query'], #search, .search-input".to_string(),
                    params: [("value".to_string(), serde_json::json!("{{query}}"))].into_iter().collect(),
                    retry: RetryConfig::default(),
                    timeout_ms: 5000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Submit,
                    target: "form".to_string(),
                    params: HashMap::new(),
                    retry: RetryConfig::default(),
                    timeout_ms: 5000,
                    chain: None,
                },
            ],
        },
        // Extract list flow
        PresetFlow {
            name: "extract_list".to_string(),
            description: "Extract list of items from page".to_string(),
            steps: vec![
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Wait,
                    target: "{{container}}".to_string(),
                    params: HashMap::new(),
                    retry: RetryConfig::default(),
                    timeout_ms: 10000,
                    chain: None,
                },
                GoalRequest {
                    session: String::new(),
                    goal_type: GoalType::Extract,
                    target: "{{item_selector}}".to_string(),
                    params: [
                        ("all".to_string(), serde_json::json!(true)),
                        ("fields".to_string(), serde_json::json!("{{fields}}")),
                    ].into_iter().collect(),
                    retry: RetryConfig::default(),
                    timeout_ms: 15000,
                    chain: None,
                },
            ],
        },
    ]
}

/// Find a preset flow by name
pub fn find_preset_flow(name: &str) -> Option<PresetFlow> {
    get_preset_flows()
        .into_iter()
        .find(|f| f.name.eq_ignore_ascii_case(name))
}

// ============================================================================
// Goal Execution Scripts
// ============================================================================

/// Generate JavaScript for goal execution
pub fn generate_goal_script(request: &GoalRequest) -> String {
    match request.goal_type {
        GoalType::Navigate => {
            format!(r#"
window.location.href = "{}";
JSON.stringify({{ navigated: true, url: window.location.href }});
"#, request.target.replace('"', "\\\""))
        }
        GoalType::Click => {
            format!(r#"
(function() {{
    const el = document.querySelector("{}");
    if (!el) return JSON.stringify({{ error: "element_not_found", selector: "{}" }});
    el.click();
    return JSON.stringify({{ clicked: true, selector: "{}" }});
}})();
"#, request.target.replace('"', "\\\""), request.target.replace('"', "\\\""), request.target.replace('"', "\\\""))
        }
        GoalType::Fill => {
            let value = request.params.get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!(r#"
(function() {{
    const el = document.querySelector("{}");
    if (!el) return JSON.stringify({{ error: "element_not_found", selector: "{}" }});
    el.value = "{}";
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
    return JSON.stringify({{ filled: true, selector: "{}" }});
}})();
"#, 
                request.target.replace('"', "\\\""), 
                request.target.replace('"', "\\\""),
                value.replace('"', "\\\""),
                request.target.replace('"', "\\\""))
        }
        GoalType::Submit => {
            format!(r#"
(function() {{
    const form = document.querySelector("{}");
    if (!form) return JSON.stringify({{ error: "form_not_found", selector: "{}" }});
    form.submit();
    return JSON.stringify({{ submitted: true }});
}})();
"#, request.target.replace('"', "\\\""), request.target.replace('"', "\\\""))
        }
        GoalType::Wait => {
            format!(r#"
(function() {{
    return new Promise((resolve) => {{
        const check = () => {{
            const el = document.querySelector("{}");
            if (el) {{
                resolve(JSON.stringify({{ found: true, selector: "{}" }}));
            }} else {{
                setTimeout(check, 100);
            }}
        }};
        check();
        setTimeout(() => resolve(JSON.stringify({{ error: "timeout", selector: "{}" }})), {});
    }});
}})();
"#, 
                request.target.replace('"', "\\\""), 
                request.target.replace('"', "\\\""),
                request.target.replace('"', "\\\""),
                request.timeout_ms)
        }
        GoalType::Extract => {
            let all = request.params.get("all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let attr = request.params.get("attribute")
                .and_then(|v| v.as_str())
                .unwrap_or("text");
            
            if all {
                format!(r#"
(function() {{
    const elements = Array.from(document.querySelectorAll("{}"));
    if (elements.length === 0) return JSON.stringify({{ error: "no_elements", selector: "{}" }});
    const data = elements.map(el => {{
        if ("{}" === "text") return el.textContent.trim();
        if ("{}" === "html") return el.innerHTML;
        return el.getAttribute("{}");
    }});
    return JSON.stringify({{ data: data, count: data.length }});
}})();
"#, 
                    request.target.replace('"', "\\\""),
                    request.target.replace('"', "\\\""),
                    attr, attr, attr)
            } else {
                format!(r#"
(function() {{
    const el = document.querySelector("{}");
    if (!el) return JSON.stringify({{ error: "element_not_found", selector: "{}" }});
    let data;
    if ("{}" === "text") data = el.textContent.trim();
    else if ("{}" === "html") data = el.innerHTML;
    else data = el.getAttribute("{}");
    return JSON.stringify({{ data: data }});
}})();
"#, 
                    request.target.replace('"', "\\\""),
                    request.target.replace('"', "\\\""),
                    attr, attr, attr)
            }
        }
        GoalType::Scroll => {
            let direction = request.params.get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("down");
            let amount = request.params.get("amount")
                .and_then(|v| v.as_u64())
                .unwrap_or(500);
            
            let delta = if direction == "up" { -(amount as i64) } else { amount as i64 };
            format!(r#"
window.scrollBy(0, {});
JSON.stringify({{ scrolled: true, scrollY: window.scrollY }});
"#, delta)
        }
        GoalType::Login | GoalType::Search | GoalType::Screenshot | GoalType::Custom => {
            // These are handled as composite goals
            r#"JSON.stringify({ "note": "Composite goal - execute steps individually" });"#.to_string()
        }
    }
}

// ============================================================================
// Custom Flow Registry
// ============================================================================

/// Registry for custom flows
#[derive(Debug, Clone, Default)]
pub struct FlowRegistry {
    pub flows: HashMap<String, PresetFlow>,
}

impl FlowRegistry {
    pub fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }
    
    /// Register a custom flow
    pub fn register(&mut self, flow: PresetFlow) {
        self.flows.insert(flow.name.clone(), flow);
    }
    
    /// Get a flow by name (custom first, then preset)
    pub fn get(&self, name: &str) -> Option<PresetFlow> {
        self.flows.get(name).cloned()
            .or_else(|| find_preset_flow(name))
    }
    
    /// List all available flows
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.flows.keys().cloned().collect();
        for preset in get_preset_flows() {
            if !names.contains(&preset.name) {
                names.push(preset.name);
            }
        }
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_classify_error_transient() {
        assert_eq!(classify_error("timeout waiting for element"), ErrorCategory::Transient);
        assert_eq!(classify_error("Element not found: #btn"), ErrorCategory::Transient);
        assert_eq!(classify_error("Network error occurred"), ErrorCategory::Transient);
    }
    
    #[test]
    fn test_classify_error_permanent() {
        assert_eq!(classify_error("Invalid selector syntax"), ErrorCategory::Permanent);
        assert_eq!(classify_error("Unauthorized access"), ErrorCategory::Permanent);
        assert_eq!(classify_error("Session closed"), ErrorCategory::Permanent);
    }
    
    #[test]
    fn test_calculate_delay() {
        let config = RetryConfig::default();
        assert_eq!(calculate_delay(0, &config), 1000);
        assert_eq!(calculate_delay(1, &config), 2000);
        assert_eq!(calculate_delay(2, &config), 4000);
        assert_eq!(calculate_delay(10, &config), 10000); // capped at max_delay
    }
    
    #[test]
    fn test_preset_flows() {
        let login = find_preset_flow("login");
        assert!(login.is_some());
        assert_eq!(login.unwrap().steps.len(), 4);
        
        let search = find_preset_flow("search");
        assert!(search.is_some());
    }
    
    #[test]
    fn test_generate_click_script() {
        let request = GoalRequest {
            session: "test".to_string(),
            goal_type: GoalType::Click,
            target: "#submit-btn".to_string(),
            params: HashMap::new(),
            retry: RetryConfig::default(),
            timeout_ms: 5000,
            chain: None,
        };
        
        let script = generate_goal_script(&request);
        assert!(script.contains("#submit-btn"));
        assert!(script.contains(".click()"));
    }
    
    #[test]
    fn test_flow_registry() {
        let mut registry = FlowRegistry::new();
        
        let custom = PresetFlow {
            name: "my_flow".to_string(),
            description: "Custom flow".to_string(),
            steps: vec![],
        };
        
        registry.register(custom);
        
        assert!(registry.get("my_flow").is_some());
        assert!(registry.get("login").is_some()); // Fallback to preset
        assert!(registry.list().contains(&"my_flow".to_string()));
    }
}
