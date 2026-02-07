//! Application Configuration
//!
//! Persistent configuration for WebView Bridge

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,
    
    /// AI/Gemini configuration
    #[serde(default)]
    pub ai: AiSettings,
    
    /// Session defaults
    #[serde(default)]
    pub session: SessionSettings,
    
    /// Media/Download settings
    #[serde(default)]
    pub media: MediaSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            ai: AiSettings::default(),
            session: SessionSettings::default(),
            media: MediaSettings::default(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address (default: 0.0.0.0)
    #[serde(default = "default_bind")]
    pub bind: String,
    
    /// Port number (default: 9400)
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Maximum concurrent sessions
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
}

fn default_bind() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 9400 }
fn default_max_sessions() -> usize { 10 }

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_port(),
            max_sessions: default_max_sessions(),
        }
    }
}

/// AI/Gemini configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// AI provider (gemini, openai, etc.)
    #[serde(default = "default_provider")]
    pub provider: String,
    
    /// API key for AI service
    #[serde(default)]
    pub api_key: Option<String>,
    
    /// Model name (default: gemini-1.5-flash)
    #[serde(default = "default_model")]
    pub model: String,
    
    /// Enable AI features
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Request timeout in milliseconds
    #[serde(default = "default_ai_timeout")]
    pub timeout_ms: u64,
    
    /// Daily budget limit in USD (optional)
    #[serde(default)]
    pub daily_budget_usd: Option<f32>,
}

fn default_provider() -> String { "gemini".to_string() }
fn default_model() -> String { "gemini-1.5-flash".to_string() }
fn default_true() -> bool { true }
fn default_ai_timeout() -> u64 { 30000 }

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: None,
            model: default_model(),
            enabled: true,
            timeout_ms: default_ai_timeout(),
            daily_budget_usd: None,
        }
    }
}

/// Session defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettings {
    /// Default headless mode
    #[serde(default)]
    pub default_headless: bool,
    
    /// Default window width
    #[serde(default = "default_width")]
    pub default_width: u32,
    
    /// Default window height
    #[serde(default = "default_height")]
    pub default_height: u32,
    
    /// Session timeout in seconds (0 = no timeout)
    #[serde(default = "default_session_timeout")]
    pub timeout_seconds: u64,
    
    /// Persist session profiles
    #[serde(default = "default_true")]
    pub persist_profiles: bool,
}

fn default_width() -> u32 { 1280 }
fn default_height() -> u32 { 720 }
fn default_session_timeout() -> u64 { 0 }

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            default_headless: false,
            default_width: default_width(),
            default_height: default_height(),
            timeout_seconds: default_session_timeout(),
            persist_profiles: true,
        }
    }
}

/// Media/Download settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSettings {
    /// Default download directory
    #[serde(default)]
    pub download_dir: Option<String>,
    
    /// Screenshots directory
    #[serde(default)]
    pub screenshots_dir: Option<String>,
    
    /// Maximum download size in bytes (0 = unlimited)
    #[serde(default)]
    pub max_download_size: u64,
    
    /// Preferred video quality (best, hd, sd, low)
    #[serde(default = "default_video_quality")]
    pub default_video_quality: String,
}

fn default_video_quality() -> String { "hd".to_string() }

impl Default for MediaSettings {
    fn default() -> Self {
        Self {
            download_dir: None,
            screenshots_dir: None,
            max_download_size: 0,
            default_video_quality: default_video_quality(),
        }
    }
}

impl AppConfig {
    /// Get the base data directory (~/.webview-bridge/)
    pub fn data_dir() -> PathBuf {
        std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".webview-bridge")
            })
    }
    
    /// Get the config file path (~/.webview-bridge/config.toml)
    pub fn config_path() -> PathBuf {
        Self::data_dir().join("config.toml")
    }
    
    /// Load config from file, or create default if not exists
    pub fn load() -> Self {
        let path = Self::config_path();
        
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match toml::from_str(&content) {
                        Ok(config) => {
                            eprintln!("Loaded config from: {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to parse config: {}. Using defaults.", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to read config: {}. Using defaults.", e);
                }
            }
        }
        
        // Create default config and save it
        let config = Self::default();
        let _ = config.save(); // Ignore save errors on first run
        config
    }
    
    /// Save config to file
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        
        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        
        eprintln!("Config saved to: {}", path.display());
        Ok(())
    }
    
    /// Get effective API key (config or environment variable)
    pub fn get_api_key(&self) -> Option<String> {
        self.ai.api_key.clone()
            .or_else(|| std::env::var("WEBVIEW_BRIDGE_AI_API_KEY").ok())
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
    }
    
    /// Get effective download directory
    pub fn get_download_dir(&self) -> PathBuf {
        self.media.download_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Self::data_dir().join("downloads")
            })
    }
    
    /// Get effective screenshots directory
    pub fn get_screenshots_dir(&self) -> PathBuf {
        self.media.screenshots_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Self::data_dir().join("screenshots")
            })
    }
    
    /// Get session-specific directory (~/.webview-bridge/sessions/{name}/)
    pub fn get_session_dir(session_name: &str) -> PathBuf {
        Self::data_dir().join("sessions").join(session_name)
    }
    
    /// Get session-specific download directory
    pub fn get_session_download_dir(session_name: &str) -> PathBuf {
        Self::get_session_dir(session_name).join("downloads")
    }
    
    /// Get session-specific screenshots directory
    pub fn get_session_screenshots_dir(session_name: &str) -> PathBuf {
        Self::get_session_dir(session_name).join("screenshots")
    }
    
    /// Get session-specific media directory (for YouTube downloads, etc.)
    pub fn get_session_media_dir(session_name: &str) -> PathBuf {
        Self::get_session_dir(session_name).join("media")
    }
    
    /// Ensure session directories exist
    pub fn ensure_session_dirs(session_name: &str) -> Result<(), String> {
        let session_dir = Self::get_session_dir(session_name);
        std::fs::create_dir_all(session_dir.join("downloads"))
            .map_err(|e| format!("Failed to create downloads dir: {}", e))?;
        std::fs::create_dir_all(session_dir.join("screenshots"))
            .map_err(|e| format!("Failed to create screenshots dir: {}", e))?;
        std::fs::create_dir_all(session_dir.join("media"))
            .map_err(|e| format!("Failed to create media dir: {}", e))?;
        Ok(())
    }
}

// ============================================================================
// Global Config Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_CONFIG: OnceLock<std::sync::RwLock<AppConfig>> = OnceLock::new();

/// Initialize the global config
pub fn init_config() -> &'static std::sync::RwLock<AppConfig> {
    GLOBAL_CONFIG.get_or_init(|| {
        std::sync::RwLock::new(AppConfig::load())
    })
}

/// Get the global config (read-only)
pub fn get_config() -> AppConfig {
    init_config().read().unwrap().clone()
}

/// Update and save the global config
pub fn update_config<F>(f: F) -> Result<(), String>
where
    F: FnOnce(&mut AppConfig),
{
    let lock = init_config();
    let mut config = lock.write().map_err(|_| "Config lock poisoned")?;
    f(&mut config);
    config.save()
}
