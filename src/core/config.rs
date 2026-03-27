//! Application Configuration
//!
//! Persistent configuration for WebView Bridge

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Bind address (default: 127.0.0.1)
    #[serde(default = "default_bind")]
    pub bind: String,

    /// Port number (default: 9400)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Maximum concurrent sessions
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    /// Disable API authentication (default: false)
    #[serde(default)]
    pub no_auth: bool,
}

fn default_bind() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 9400 }
fn default_max_sessions() -> usize { 10 }

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_port(),
            max_sessions: default_max_sessions(),
            no_auth: true,
        }
    }
}

/// AI/Gemini configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// AI provider (gemini, ollama)
    #[serde(default = "default_provider")]
    pub provider: String,

    /// API key for AI service
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name (default: gemini-1.5-flash)
    #[serde(default = "default_model")]
    pub model: String,

    /// Ollama host URL (default: http://localhost:11434)
    #[serde(default)]
    pub ollama_host: Option<String>,

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
            ollama_host: None,
            enabled: true,
            timeout_ms: default_ai_timeout(),
            daily_budget_usd: None,
        }
    }
}

/// Auto-login configuration for a named session using 1Password CLI
///
/// Example config.toml:
/// ```toml
/// [session.auto_login.rms]
/// op_item = "grp02.id.rakuten.co.jp"
/// username_selector = "#loginInner_u"
/// password_selector = "#loginInner_p"
/// submit_selector = "#loginInner_submit"
/// logged_in_selector = ".navi-logout"
/// login_url = "https://grp02.id.rakuten.co.jp/rms/nid/vc"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLoginConfig {
    /// 1Password item name or ID (e.g. "grp02.id.rakuten.co.jp")
    /// If not set, WB will look up the cached item from the session DB
    #[serde(default)]
    pub op_item: Option<String>,
    /// 1Password vault name (required when using a service account token).
    /// e.g. "Personal", "Private", "Employee"
    #[serde(default)]
    pub op_vault: Option<String>,
    /// Plaintext username (OP-free fallback). Use only when 1Password is not available.
    /// If set together with op_item, op_item takes precedence.
    #[serde(default)]
    pub username: Option<String>,
    /// Plaintext password (OP-free fallback). Use only when 1Password is not available.
    /// WARNING: storing passwords in config files is insecure. Use op_item instead.
    #[serde(default)]
    pub password: Option<String>,
    /// CSS selector for the username/email input field
    pub username_selector: String,
    /// CSS selector for the password input field
    pub password_selector: String,
    /// CSS selector for the login submit button
    pub submit_selector: String,
    /// CSS selector to check if already logged in (skip login if found)
    #[serde(default)]
    pub logged_in_selector: Option<String>,
    /// URL to navigate to before filling the login form
    #[serde(default)]
    pub login_url: Option<String>,
    /// Wait for OTP/2FA input after form submit (selector for OTP field)
    #[serde(default)]
    pub otp_selector: Option<String>,
    /// 1Password TOTP field reference (e.g. "op://Personal/item/one-time password")
    #[serde(default)]
    pub otp_op_ref: Option<String>,
    /// Additional login steps for multi-step auth flows (e.g. RMS → Rakuten SSO).
    /// Each step is triggered when the URL contains `wait_url_contains`.
    #[serde(default)]
    pub extra_steps: Vec<AutoLoginStep>,
}

/// A single step in a multi-step login flow.
///
/// Used in `AutoLoginConfig.extra_steps` to handle sites that require
/// multiple authentication stages (e.g. service login → SSO → OTP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLoginStep {
    /// Wait until the page URL contains this substring before executing this step.
    /// If not set, executes immediately after the previous step.
    #[serde(default)]
    pub wait_url_contains: Option<String>,
    /// 1Password item for this step (overrides the parent AutoLoginConfig op_item).
    #[serde(default)]
    pub op_item: Option<String>,
    /// 1Password vault for this step (overrides the parent AutoLoginConfig op_vault).
    #[serde(default)]
    pub op_vault: Option<String>,
    /// Plaintext username for this step (OP-free fallback).
    #[serde(default)]
    pub username: Option<String>,
    /// Plaintext password for this step (OP-free fallback).
    #[serde(default)]
    pub password: Option<String>,
    /// CSS selector for the username/email input (optional — skip if None).
    #[serde(default)]
    pub username_selector: Option<String>,
    /// CSS selector for an intermediate "Next" button (e.g. username-only first screen).
    /// If set: click this button after filling username, then wait for password field.
    #[serde(default)]
    pub next_selector: Option<String>,
    /// CSS selector to wait for before filling password (used after clicking next_selector).
    #[serde(default)]
    pub wait_password_selector: Option<String>,
    /// CSS selector for the password input (optional — skip if None).
    #[serde(default)]
    pub password_selector: Option<String>,
    /// CSS selector for the submit button.
    #[serde(default)]
    pub submit_selector: Option<String>,
    /// CSS selector indicating this step completed successfully (waits up to 15s).
    #[serde(default)]
    pub done_selector: Option<String>,
    /// Wait for OTP/2FA input after submit (selector for OTP field).
    #[serde(default)]
    pub otp_selector: Option<String>,
    /// 1Password TOTP reference for this step (e.g. "op://Vault/Item/one-time password").
    #[serde(default)]
    pub otp_op_ref: Option<String>,
    /// Extra wait (ms) before clicking submit — allows async bot-detection challenges (e.g.
    /// Proof-of-Work like r10-challenger) to complete before the form is submitted.
    /// Default: 1000ms.  Set higher (e.g. 3000) for slow PoW or laggy pages.
    #[serde(default = "default_pre_submit_wait_ms")]
    pub pre_submit_wait_ms: u64,
    /// JavaScript snippet evaluated before submit.  The automation waits until the expression
    /// returns a truthy value (up to `challenge_timeout_ms`).
    /// Example: `"typeof window.r10ChallengeReady !== 'undefined' && window.r10ChallengeReady"`
    /// Leave unset to skip this check (the `pre_submit_wait_ms` delay still applies).
    #[serde(default)]
    pub challenge_done_js: Option<String>,
    /// Timeout (ms) for `challenge_done_js` polling.  Default: 15000.
    #[serde(default = "default_challenge_timeout_ms")]
    pub challenge_timeout_ms: u64,
}

fn default_pre_submit_wait_ms() -> u64 { 1000 }
fn default_challenge_timeout_ms() -> u64 { 15000 }

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

    /// Per-session auto-login config (session_name -> AutoLoginConfig)
    #[serde(default)]
    pub auto_login: HashMap<String, AutoLoginConfig>,

    /// Path to 1Password CLI binary (default: auto-detect op in PATH or winget location)
    #[serde(default)]
    pub op_path: Option<String>,

    /// 1Password service account token (ops_...).
    /// When set, all `op` CLI calls use this token instead of interactive auth.
    /// Create at: 1password.com → Settings → Developer → Service Accounts
    /// Eliminates all password prompts for background/server use.
    #[serde(default)]
    pub op_service_account_token: Option<String>,
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
            auto_login: HashMap::new(),
            op_path: None,
            op_service_account_token: None,
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

/// Application configuration and path management.
///
/// Directory layout (%APPDATA%/webview-bridge/):
/// ```text
/// config.toml              設定ファイル
/// sessions.json            セッション永続化メタデータ
/// profiles/
///   {profile_name}/
///     (WebView2 userdata)  ブラウザデータ (Cookie, Cache)
///     screenshots/         スクリーンショット保存
/// downloads/               ダウンロード出力 (yt-dlp等)
/// subtitles/               字幕抽出出力
/// analysis/                動画分析出力
/// ```
impl AppConfig {
    /// Get the base data directory (%APPDATA%/webview-bridge/)
    ///
    /// Resolution order:
    /// 1. WEBVIEW_BRIDGE_DATA_PATH env var (explicit override)
    /// 2. %APPDATA%/webview-bridge (Windows standard via dirs::data_dir())
    /// 3. Legacy fallback: ~/.webview-bridge/ is auto-migrated if found
    pub fn data_dir() -> PathBuf {
        std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let new_path = dirs::data_dir()
                    .expect("Cannot determine AppData directory. Set WEBVIEW_BRIDGE_DATA_PATH.")
                    .join("webview-bridge");

                // Migrate from legacy ~/.webview-bridge/ if it exists and new path doesn't
                if !new_path.exists() {
                    if let Some(legacy) = dirs::home_dir().map(|h| h.join(".webview-bridge")) {
                        if legacy.exists() {
                            eprintln!("[config] Migrating data: {} -> {}", legacy.display(), new_path.display());
                            if let Some(parent) = new_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            match std::fs::rename(&legacy, &new_path) {
                                Ok(_) => eprintln!("[config] Migration successful"),
                                Err(e) => eprintln!("[config] Migration failed (will use new path): {}", e),
                            }
                        }
                    }
                }

                new_path
            })
    }
    
    /// Get the config file path (%APPDATA%/webview-bridge/config.toml)
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
    
    /// Base directory for all profiles: {data_dir}/profiles/
    pub fn profiles_dir() -> PathBuf {
        Self::data_dir().join("profiles")
    }

    /// Root directory for a specific profile: {data_dir}/profiles/{name}/
    pub fn profile_dir(name: &str) -> PathBuf {
        Self::profiles_dir().join(name)
    }

    /// Screenshots directory for a profile: {data_dir}/profiles/{name}/screenshots/
    pub fn profile_screenshots_dir(name: &str) -> PathBuf {
        Self::profile_dir(name).join("screenshots")
    }

    /// Base directory for uploads: {data_dir}/uploads/
    pub fn uploads_dir() -> PathBuf {
        Self::data_dir().join("uploads")
    }

    /// Uploads directory for a session: {data_dir}/uploads/{session}/
    pub fn session_uploads_dir(session: &str) -> PathBuf {
        Self::uploads_dir().join(session)
    }

    /// Per-session auto_login.toml path: {data_dir}/profiles/{name}/auto_login.toml
    pub fn session_auto_login_path(session_name: &str) -> PathBuf {
        Self::profile_dir(session_name).join("auto_login.toml")
    }
}

/// Load auto-login config for a session.
///
/// Priority:
/// 1. `{data_dir}/profiles/{name}/auto_login.toml` (per-session file)
/// 2. `config.toml` `[session.auto_login.{name}]` (global fallback)
///
/// Returns `None` if neither is configured.
pub fn load_session_auto_login(session_name: &str) -> Option<AutoLoginConfig> {
    // 1. Per-session file
    let path = AppConfig::session_auto_login_path(session_name);
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            match toml::from_str::<AutoLoginConfig>(&contents) {
                Ok(cfg) => return Some(cfg),
                Err(e) => {
                    tracing::warn!(
                        "[AutoLogin] Failed to parse {:?}: {} — falling back to config.toml",
                        path, e
                    );
                }
            }
        }
    }

    // 2. config.toml fallback
    let app_cfg = get_config();
    app_cfg.session.auto_login.get(session_name).cloned()
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
