//! MCP v3 Type Definitions
//!
//! Request/Response types for 8 consolidated tools

use serde::{Deserialize, Serialize};

// ============================================================================
// Common Types
// ============================================================================

/// MCP Tool Request wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolRequest {
    pub tool: String,
    #[serde(flatten)]
    pub params: serde_json::Value,
}

/// MCP Tool Response
#[derive(Debug, Clone, Serialize)]
pub struct McpToolResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<McpContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP Content (text or image)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

/// MCP Error
#[derive(Debug, Clone, Serialize)]
pub struct McpError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

// ============================================================================
// 1. Navigate
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct NavigateRequest {
    pub session: String,
    pub url: String,
    #[serde(default = "default_wait_for")]
    pub wait_for: WaitForCondition,
    #[serde(default)]
    pub wait_selector: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaitForCondition {
    Load,
    #[default]
    Stable,
    NetworkIdle,
    Selector,
}

fn default_wait_for() -> WaitForCondition {
    WaitForCondition::Stable
}

fn default_timeout() -> u64 {
    30000
}

// ============================================================================
// 2. Interact
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct InteractRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub options: InteractOptions,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct InteractOptions {
    #[serde(default = "default_wait_timeout")]
    pub wait_timeout_ms: u64,
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
    #[serde(default)]
    pub screenshot_on_error: bool,
    #[serde(default)]
    pub slow_mode_ms: u64,
    /// Enable human-like behavior: random delays
    #[serde(default)]
    pub human_mode: bool,
}

fn default_wait_timeout() -> u64 { 10000 }
fn default_retry_count() -> u32 { 3 }
fn default_retry_delay() -> u64 { 500 }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Click {
        target: String,
        #[serde(default)]
        wait_after_ms: Option<u64>,
    },
    Type {
        target: String,
        value: String,
        #[serde(default)]
        clear: bool,
        /// Instant mode: set value directly instead of character-by-character
        /// Use for autocomplete-heavy inputs like Amazon search
        #[serde(default)]
        instant: bool,
    },
    Scroll {
        #[serde(default)]
        direction: ScrollDirection,
        #[serde(default = "default_scroll_amount")]
        amount: i32,
        #[serde(default)]
        target: Option<String>,
    },
    Hover {
        target: String,
    },
    Select {
        target: String,
        value: String,
    },
    Wait {
        condition: WaitCondition,
        #[serde(default)]
        value: Option<String>,
        #[serde(default = "default_wait_timeout")]
        timeout_ms: u64,
    },
    Screenshot,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

fn default_scroll_amount() -> i32 { 500 }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    Element,
    ElementVisible,
    ElementClickable,
    ElementHidden,
    UrlContains,
    UrlMatches,
    TextContains,
    NetworkIdle,
    Timeout,
}

// ============================================================================
// 3. Capture
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct CaptureRequest {
    #[serde(default = "default_session")]
    pub session: String,
    #[serde(default = "default_true")]
    pub screenshot: bool,
    #[serde(default)]
    pub include: Vec<CaptureInclude>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub full_page: bool,
    #[serde(default)]
    pub text_max_chars: Option<usize>,
    #[serde(default)]
    pub summarize: bool,
    /// Analyze interactivity of elements with LLM (Phase 2)
    #[serde(default)]
    pub analyze_interactivity: bool,
    /// Analyze images without alt text using Vision LLM (Phase 3)
    #[serde(default)]
    pub analyze_vision: bool,
    /// Use CDP for screenshot (better full-page support)
    #[serde(default = "default_true")]
    pub use_cdp: bool,
}


#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureInclude {
    Cookies,
    FullText,
    Html,
    Images,
}

fn default_true() -> bool { true }
fn default_ttl_hours_session() -> u64 { 168 } // 1 week default

// ============================================================================
// 4. Extract
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub selector: String,
    pub fields: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub wait_for_count: Option<usize>,
    #[serde(default = "default_wait_timeout")]
    pub wait_timeout_ms: u64,
    #[serde(default)]
    pub scroll_for_more: bool,
    #[serde(default = "default_scroll_max")]
    pub scroll_max: usize,
}

fn default_scroll_max() -> usize { 5 }

// ============================================================================
// 5. Session
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct SessionRequest {
    #[serde(default)]
    pub session: Option<String>,  // Target session name for device switch etc.
    #[serde(default)]
    pub acquire: Option<String>,
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub list: bool,
    #[serde(default)]
    pub import: Option<String>,
    #[serde(default)]
    pub clone_to: Option<String>,  // Clone session cookies/profile to a new session name
    // acquire options
    #[serde(default)]
    pub headless: bool,
    #[serde(default = "default_true")]
    pub restore: bool,
    #[serde(default = "default_ttl_hours_session")]
    pub ttl_hours: u64,  // 168 = 1 week (default), 0 = no expiration (infinite/persistent)
    // import options
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    // AI configuration
    #[serde(default)]
    pub ai_status: bool,
    #[serde(default)]
    pub ai_models: bool,
    #[serde(default)]
    pub ai_config: Option<AiConfigUpdate>,
    // Device simulation / Viewport
    #[serde(default)]
    pub device: Option<String>,  // Device preset name (e.g., "iPhone 14", "Pixel 7")
    #[serde(default)]
    pub viewport_width: Option<u32>,
    #[serde(default)]
    pub viewport_height: Option<u32>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiConfigUpdate {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
}

// ============================================================================
// 6. Media
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct MediaRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub action: MediaAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaAction {
    YoutubeDownload {
        url: String,
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        audio_only: bool,
        #[serde(default)]
        output_dir: Option<String>,
    },
    YoutubeSubtitles {
        url: String,
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        format: Option<String>,
    },
    VideoAnalyze {
        url: String,
        #[serde(default)]
        keyframes: bool,
        #[serde(default)]
        audio: bool,
        #[serde(default)]
        max_frames: Option<u32>,
    },
    CollectImages {
        #[serde(default)]
        selector: Option<String>,
        #[serde(default)]
        min_width: Option<u32>,
        #[serde(default)]
        min_height: Option<u32>,
        #[serde(default)]
        download: bool,
        #[serde(default)]
        max_images: Option<usize>,
    },
}

// ============================================================================
// 7. Execute
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub script: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

// ============================================================================
// 8. Agent (Agentic Mode - Future)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct AgentRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub action: AgentAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    Start {
        goal: String,
        #[serde(default)]
        context: Option<String>,
        #[serde(default)]
        max_steps: Option<u32>,
        #[serde(default)]
        system_prompt: Option<String>,
        /// Use human-mode for actions (delays, natural mouse movement)
        #[serde(default)]
        human_mode: bool,
        /// Use instant mode for type actions (avoid autocomplete interference)
        #[serde(default)]
        instant_type: bool,
    },
    Resume,
    Status,
    Cancel,
}

// ============================================================================
// 9. Network
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkRequest {
    #[serde(default = "default_session")]
    pub session: String,
    pub action: NetworkRequestAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkRequestAction {
    Enable { 
        #[serde(default)]
        max_logs: Option<usize> 
    },
    Disable,
    GetLogs { 
        #[serde(default)]
        filter: Option<String> 
    },
    ClearLogs,
}

// ============================================================================
// Helpers
// ============================================================================

fn default_session() -> String {
    "default".to_string()
}

impl McpToolResponse {
    pub fn success_text(text: String) -> Self {
        Self {
            success: true,
            content: Some(vec![McpContent::Text { text }]),
            error: None,
        }
    }

    pub fn success_with_image(text: String, image_data: String, mime_type: String) -> Self {
        Self {
            success: true,
            content: Some(vec![
                McpContent::Text { text },
                McpContent::Image { data: image_data, mime_type },
            ]),
            error: None,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self {
            success: false,
            content: None,
            error: Some(McpError {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            }),
        }
    }

    pub fn error_with_details(code: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            success: false,
            content: None,
            error: Some(McpError {
                code: code.to_string(),
                message: message.to_string(),
                details: Some(details),
            }),
        }
    }
    
    /// Success response with JSON data (serialized as text for AI parsing)
    pub fn success_json(data: serde_json::Value) -> Self {
        let text = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        Self {
            success: true,
            content: Some(vec![McpContent::Text { text }]),
            error: None,
        }
    }
}
