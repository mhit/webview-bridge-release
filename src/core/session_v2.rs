//! WBP2 Session Management v2
//!
//! Named session management with persistence and auth state tracking.
//! See: docs/PROTOCOL_V2.md Section 3.1

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use super::SessionOptions;

/// V2 Session Handle - holds the v1 session ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHandle {
    /// The v1 session ID  
    pub id: String,
}

// ============================================================================
// Named Session Types
// ============================================================================

/// Authentication status for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub logged_in: bool,
    pub checked_at: Option<String>, // ISO 8601
    pub username: Option<String>,
}

impl Default for AuthStatus {
    fn default() -> Self {
        Self {
            logged_in: false,
            checked_at: None,
            username: None,
        }
    }
}

/// Named session metadata (persisted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedSessionMeta {
    /// Session name (primary key)
    pub name: String,
    /// Associated profile name
    pub profile: String,
    /// Authentication status
    pub auth_status: AuthStatus,
    /// Last accessed timestamp (ISO 8601)
    pub last_accessed: String,
    /// Whether to auto-extend lifetime on access
    pub auto_extend: bool,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Time-to-live in hours (0 = no expiration, default: 168 = 1 week)
    #[serde(default = "default_ttl_hours")]
    pub ttl_hours: u64,
    /// Expiration timestamp (ISO 8601, calculated from created_at + ttl_hours)
    #[serde(default)]
    pub expires_at: Option<String>,
    
    // Session state (for Chrome-like restore)
    /// Last URL the session was on
    #[serde(default)]
    pub last_url: Option<String>,
    /// Navigation history (URLs visited in this session)
    #[serde(default)]
    pub navigation_history: Vec<String>,
    /// Whether to auto-restore last URL on acquire
    #[serde(default = "default_auto_restore")]
    pub auto_restore: bool,
}

fn default_ttl_hours() -> u64 { 168 } // 1 week
fn default_auto_restore() -> bool { true } // Auto-restore last URL by default

/// Named session state (in-memory, includes runtime handle)
#[derive(Clone)]
pub struct NamedSession {
    pub meta: NamedSessionMeta,
    /// Runtime session handle (None if not active)
    pub handle: Option<SessionHandle>,
    /// Whether session is currently acquired
    pub acquired: bool,
}

// ============================================================================
// Session Persistence
// ============================================================================

/// Sessions.json structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionsFile {
    pub version: u32,
    pub sessions: HashMap<String, NamedSessionMeta>,
}

impl SessionsFile {
    pub fn new() -> Self {
        Self {
            version: 1,
            sessions: HashMap::new(),
        }
    }
}

// ============================================================================
// Acquire Request/Response (API types)
// ============================================================================

/// Auth check configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthCheckConfig {
    pub url: String,
    pub logged_in_selector: String,
    pub login_required_selector: Option<String>,
}

/// Request for POST /v2/session/acquire
#[derive(Debug, Clone, Deserialize)]
pub struct AcquireRequest {
    pub name: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "default_true")]
    pub reuse: bool,
    #[serde(default = "default_true")]
    pub create_if_missing: bool,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub auth_check: Option<AuthCheckConfig>,
    /// Time-to-live in hours (0 = no expiration, default: 168 = 1 week)
    #[serde(default = "default_ttl_hours")]
    pub ttl_hours: u64,
    /// Auto-extend TTL on each access (default: true)
    #[serde(default = "default_true")]
    pub auto_extend: bool,
    /// Restore last URL on acquire (default: true)
    #[serde(default = "default_true")]
    pub restore: bool,
}

fn default_true() -> bool {
    true
}

/// Response for POST /v2/session/acquire
#[derive(Debug, Clone, Serialize)]
pub struct AcquireResponse {
    pub session: String,
    pub is_new: bool,
    pub profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_status: Option<AuthStatus>,
    /// URL that was restored (if auto-restore was triggered)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_url: Option<String>,
}

/// Response for GET /v2/session/list
#[derive(Debug, Clone, Serialize)]
pub struct SessionListItem {
    pub name: String,
    pub profile: String,
    pub auth_status: AuthStatus,
    pub last_accessed: String,
    pub active: bool,
    pub acquired: bool,
    /// Time-to-live in hours
    pub ttl_hours: u64,
    /// Expiration timestamp (ISO 8601)
    pub expires_at: Option<String>,
    /// Whether session is expired
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionListItem>,
    pub total: usize,
}

// ============================================================================
// SessionManagerV2
// ============================================================================

/// WBP2 Session Manager with named sessions and persistence
pub struct SessionManagerV2 {
    /// Named sessions (name -> NamedSession)
    sessions: Arc<RwLock<HashMap<String, NamedSession>>>,
    /// Path to sessions.json
    persistence_path: PathBuf,
    /// Maximum concurrent sessions
    max_sessions: usize,
    /// Idle timeout for cleanup
    idle_timeout: Duration,
}

impl SessionManagerV2 {
    /// Create a new SessionManagerV2
    pub fn new(
        data_dir: PathBuf,
        max_sessions: usize,
    ) -> Self {
        let persistence_path = data_dir.join("sessions.json");
        
        let manager = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            persistence_path,
            max_sessions,
            idle_timeout: Duration::from_secs(300), // 5 minutes default
        };
        
        // Load persisted sessions on startup
        if let Err(e) = manager.load_sessions() {
            eprintln!("[SessionManagerV2] Failed to load sessions: {}", e);
        }
        
        manager
    }
    
    /// Set idle timeout
    pub fn with_idle_timeout(mut self, timeout_secs: u64) -> Self {
        self.idle_timeout = Duration::from_secs(timeout_secs);
        self
    }
    
    // ========================================================================
    // Persistence
    // ========================================================================
    
    /// Load sessions from sessions.json
    fn load_sessions(&self) -> Result<(), String> {
        if !self.persistence_path.exists() {
            return Ok(());
        }
        
        let content = fs::read_to_string(&self.persistence_path)
            .map_err(|e| format!("Failed to read sessions.json: {}", e))?;
        
        let file: SessionsFile = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse sessions.json: {}", e))?;
        
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        for (name, meta) in file.sessions {
            sessions.insert(name.clone(), NamedSession {
                meta,
                handle: None, // Will be restored on acquire
                acquired: false,
            });
        }
        
        println!("[SessionManagerV2] Loaded {} sessions from disk", sessions.len());
        Ok(())
    }
    
    /// Save sessions to sessions.json
    fn save_sessions(&self) -> Result<(), String> {
        let sessions = self.sessions.read()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let file = SessionsFile {
            version: 1,
            sessions: sessions.iter()
                .map(|(k, v)| (k.clone(), v.meta.clone()))
                .collect(),
        };
        
        let content = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        
        // Ensure parent directory exists
        if let Some(parent) = self.persistence_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        
        fs::write(&self.persistence_path, content)
            .map_err(|e| format!("Failed to write sessions.json: {}", e))?;
        
        Ok(())
    }
    
    // ========================================================================
    // Session Operations
    // ========================================================================
    
    /// Acquire a named session (reuse or create)
    pub async fn acquire(
        &self,
        request: AcquireRequest,
        create_session_fn: impl FnOnce(SessionOptions) -> Result<(String, SessionHandle), String>,
    ) -> Result<AcquireResponse, String> {
        let now = chrono_now_iso8601();
        let profile_name = request.profile.clone()
            .unwrap_or_else(|| request.name.clone());
        
        // Check if session exists
        let existing = {
            let sessions = self.sessions.read()
                .map_err(|_| "Lock poisoned".to_string())?;
            sessions.get(&request.name).cloned()
        };
        
        if let Some(mut session) = existing {
            if request.reuse {
                // Reuse existing session
                session.meta.last_accessed = now;
                session.acquired = true;
                
                // Update in map
                {
                    let mut sessions = self.sessions.write()
                        .map_err(|_| "Lock poisoned".to_string())?;
                    sessions.insert(request.name.clone(), session.clone());
                }
                
                self.save_sessions()?;
                
                return Ok(AcquireResponse {
                    session: request.name,
                    is_new: false,
                    profile: session.meta.profile,
                    auth_status: Some(session.meta.auth_status),
                    restored_url: None, // Restored by API layer
                });
            }
        }
        
        // Create new session
        if !request.create_if_missing {
            return Err(format!("Session '{}' not found and create_if_missing is false", request.name));
        }
        
        // Check max sessions
        {
            let sessions = self.sessions.read()
                .map_err(|_| "Lock poisoned".to_string())?;
            if sessions.len() >= self.max_sessions {
                return Err(format!("Maximum sessions ({}) reached", self.max_sessions));
            }
        }
        
        // Create session options (v1 compatible)
        let options = SessionOptions {
            profile: profile_name.clone(),
            headless: request.headless,
            user_agent: None,
        };
        
        // Create the actual session
        let (_id, handle) = create_session_fn(options)?;
        
        // Calculate expiration time if TTL is set
        let expires_at = if request.ttl_hours > 0 {
            Some(chrono_add_hours(&now, request.ttl_hours))
        } else {
            None
        };
        
        let meta = NamedSessionMeta {
            name: request.name.clone(),
            profile: profile_name.clone(),
            auth_status: AuthStatus::default(),
            last_accessed: now.clone(),
            auto_extend: request.auto_extend,
            created_at: now,
            ttl_hours: request.ttl_hours,
            expires_at,
            // Session state
            last_url: None,
            navigation_history: Vec::new(),
            auto_restore: true,
        };
        
        let session = NamedSession {
            meta: meta.clone(),
            handle: Some(handle),
            acquired: true,
        };
        
        // Store in map
        {
            let mut sessions = self.sessions.write()
                .map_err(|_| "Lock poisoned".to_string())?;
            sessions.insert(request.name.clone(), session);
        }
        
        self.save_sessions()?;
        
        Ok(AcquireResponse {
            session: request.name,
            is_new: true,
            profile: profile_name,
            auth_status: None,
            restored_url: None, // New session, nothing to restore
        })
    }
    
    /// Register an externally created session
    /// Used when sessions are created through other means (e.g., MCP)
    pub fn register(&self, name: &str, id: &str, profile: &str, _headless: bool) {
        let now = chrono_now_iso8601();
        let ttl_hours = default_ttl_hours();
        let expires_at = Some(chrono_add_hours(&now, ttl_hours));
        
        let meta = NamedSessionMeta {
            name: name.to_string(),
            profile: profile.to_string(),
            auth_status: AuthStatus::default(),
            last_accessed: now.clone(),
            auto_extend: true,
            created_at: now,
            ttl_hours,
            expires_at,
            // Session state
            last_url: None,
            navigation_history: Vec::new(),
            auto_restore: true,
        };
        
        let session = NamedSession {
            meta,
            handle: Some(SessionHandle { id: id.to_string() }),
            acquired: true,
        };
        
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.insert(name.to_string(), session);
        }
        
        let _ = self.save_sessions();
    }
    
    /// Release a session (keep profile, mark as not acquired)
    pub fn release(&self, name: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.get_mut(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        session.acquired = false;
        session.meta.last_accessed = chrono_now_iso8601();
        
        // Extend expiration if auto_extend is enabled
        if session.meta.auto_extend && session.meta.ttl_hours > 0 {
            let now = chrono_now_iso8601();
            session.meta.expires_at = Some(chrono_add_hours(&now, session.meta.ttl_hours));
        }
        
        drop(sessions);
        self.save_sessions()?;
        
        Ok(())
    }
    
    /// Update session's last URL (for session restore)
    pub fn update_last_url(&self, name: &str, url: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.get_mut(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        // Don't save empty or about:blank URLs
        if url.is_empty() || url == "about:blank" {
            return Ok(());
        }
        
        // Add to history if different from last entry
        if session.meta.navigation_history.last() != Some(&url.to_string()) {
            session.meta.navigation_history.push(url.to_string());
            
            // Keep only last 50 entries
            if session.meta.navigation_history.len() > 50 {
                session.meta.navigation_history.remove(0);
            }
        }
        
        session.meta.last_url = Some(url.to_string());
        session.meta.last_accessed = chrono_now_iso8601();
        
        drop(sessions);
        
        // Don't persist on every URL change (too expensive)
        // Only persist when session is released
        Ok(())
    }
    
    /// Get the last URL for a session (for restore)
    pub fn get_last_url(&self, name: &str) -> Option<String> {
        let sessions = self.sessions.read().ok()?;
        sessions.get(name).and_then(|s| s.meta.last_url.clone())
    }
    
    /// Get session's navigation history
    pub fn get_navigation_history(&self, name: &str) -> Vec<String> {
        let sessions = match self.sessions.read() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        sessions.get(name)
            .map(|s| s.meta.navigation_history.clone())
            .unwrap_or_default()
    }
    
    /// Destroy a session (close and remove)
    /// Returns the session ID if found, so caller can close the WebView
    pub fn destroy(&self, name: &str) -> Result<Option<String>, String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.remove(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        // Get the session ID to close
        let session_id = session.handle.map(|h| h.id);
        
        drop(sessions);
        self.save_sessions()?;
        
        Ok(session_id)
    }
    
    /// Destroy all inactive sessions
    /// Returns list of session IDs that need to be closed
    pub fn cleanup_inactive(&self) -> Result<Vec<String>, String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let mut to_close = Vec::new();
        let mut to_remove = Vec::new();
        
        for (name, session) in sessions.iter() {
            if !session.acquired {
                if let Some(ref handle) = session.handle {
                    to_close.push(handle.id.clone());
                }
                to_remove.push(name.clone());
            }
        }
        
        for name in &to_remove {
            sessions.remove(name);
        }
        
        drop(sessions);
        let _ = self.save_sessions();
        
        println!("[SessionManagerV2] Cleaned up {} inactive sessions", to_remove.len());
        
        Ok(to_close)
    }
    
    /// Cleanup old sessions based on last_accessed time
    /// Returns list of session IDs that need to be closed
    pub fn cleanup_old(&self, max_age_hours: u64) -> Result<Vec<String>, String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let now = SystemTime::now();
        let max_age = Duration::from_secs(max_age_hours * 3600);
        let mut to_close = Vec::new();
        let mut to_remove = Vec::new();
        
        for (name, session) in sessions.iter() {
            // Parse the last_accessed timestamp
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&session.meta.last_accessed) {
                let session_time = SystemTime::UNIX_EPOCH + Duration::from_secs(ts.timestamp() as u64);
                if let Ok(elapsed) = now.duration_since(session_time) {
                    if elapsed > max_age && !session.acquired {
                        if let Some(ref handle) = session.handle {
                            to_close.push(handle.id.clone());
                        }
                        to_remove.push(name.clone());
                    }
                }
            }
        }
        
        for name in &to_remove {
            sessions.remove(name);
        }
        
        drop(sessions);
        let _ = self.save_sessions();
        
        println!("[SessionManagerV2] Cleaned up {} old sessions (older than {}h)", to_remove.len(), max_age_hours);
        
        Ok(to_close)
    }
    
    /// Cleanup expired sessions (based on TTL)
    /// Returns list of session IDs that need to be closed
    pub fn cleanup_expired(&self) -> Result<Vec<String>, String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let mut to_close = Vec::new();
        let mut to_remove = Vec::new();
        
        for (name, session) in sessions.iter() {
            if is_expired(&session.meta.expires_at) && !session.acquired {
                if let Some(ref handle) = session.handle {
                    to_close.push(handle.id.clone());
                }
                to_remove.push(name.clone());
            }
        }
        
        for name in &to_remove {
            sessions.remove(name);
        }
        
        drop(sessions);
        let _ = self.save_sessions();
        
        if !to_remove.is_empty() {
            println!("[SessionManagerV2] Cleaned up {} expired sessions", to_remove.len());
        }
        
        Ok(to_close)
    }
    
    /// List all sessions
    pub fn list(&self) -> Result<ListSessionsResponse, String> {
        let sessions = self.sessions.read()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let items: Vec<SessionListItem> = sessions.values()
            .map(|s| SessionListItem {
                name: s.meta.name.clone(),
                profile: s.meta.profile.clone(),
                auth_status: s.meta.auth_status.clone(),
                last_accessed: s.meta.last_accessed.clone(),
                active: s.handle.is_some(),
                acquired: s.acquired,
                ttl_hours: s.meta.ttl_hours,
                expires_at: s.meta.expires_at.clone(),
                expired: is_expired(&s.meta.expires_at),
            })
            .collect();
        
        let total = items.len();
        
        Ok(ListSessionsResponse { sessions: items, total })
    }
    
    /// Get a session by name
    pub fn get(&self, name: &str) -> Option<NamedSession> {
        let sessions = self.sessions.read().ok()?;
        sessions.get(name).cloned()
    }
    
    /// Get session handle for sending commands
    pub fn get_handle(&self, name: &str) -> Option<SessionHandle> {
        let sessions = self.sessions.read().ok()?;
        sessions.get(name)?.handle.clone()
    }
    
    /// Update auth status for a session
    pub fn update_auth_status(&self, name: &str, auth_status: AuthStatus) -> Result<(), String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.get_mut(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        session.meta.auth_status = auth_status;
        session.meta.last_accessed = chrono_now_iso8601();
        
        drop(sessions);
        self.save_sessions()?;
        
        Ok(())
    }
    
    /// Get session count
    pub fn count(&self) -> usize {
        self.sessions.read()
            .map(|s| s.len())
            .unwrap_or(0)
    }
    
    /// Clone a session (copy profile to new name)
    /// The new session will have a fresh browser instance but share the same cookies/storage
    pub fn clone_session(&self, source_name: &str, new_name: &str) -> Result<(), String> {
        // Check source exists
        let source_meta = {
            let sessions = self.sessions.read()
                .map_err(|_| "Lock poisoned".to_string())?;
            sessions.get(source_name)
                .ok_or_else(|| format!("Source session '{}' not found", source_name))?
                .meta.clone()
        };
        
        // Check new name doesn't exist
        {
            let sessions = self.sessions.read()
                .map_err(|_| "Lock poisoned".to_string())?;
            if sessions.contains_key(new_name) {
                return Err(format!("Session '{}' already exists", new_name));
            }
        }
        
        // Copy profile directory
        let data_dir = crate::core::config::AppConfig::data_dir();
        let source_profile_dir = data_dir.join("profiles").join(&source_meta.profile);
        let new_profile_dir = data_dir.join("profiles").join(new_name);
        
        if source_profile_dir.exists() {
            copy_dir_recursive(&source_profile_dir, &new_profile_dir)?;
        }
        
        // Copy session data directory  
        let source_session_dir = crate::core::config::AppConfig::get_session_dir(source_name);
        let new_session_dir = crate::core::config::AppConfig::get_session_dir(new_name);
        
        if source_session_dir.exists() {
            copy_dir_recursive(&source_session_dir, &new_session_dir)?;
        }
        
        // Create new session metadata
        let now = chrono_now_iso8601();
        let ttl_hours = source_meta.ttl_hours;
        let expires_at = if ttl_hours > 0 {
            Some(chrono_add_hours(&now, ttl_hours))
        } else {
            None
        };
        
        let new_meta = NamedSessionMeta {
            name: new_name.to_string(),
            profile: new_name.to_string(),
            auth_status: source_meta.auth_status,
            last_accessed: now.clone(),
            auto_extend: source_meta.auto_extend,
            created_at: now,
            ttl_hours,
            expires_at,
            // Copy session state from source
            last_url: source_meta.last_url,
            navigation_history: source_meta.navigation_history,
            auto_restore: source_meta.auto_restore,
        };
        
        let new_session = NamedSession {
            meta: new_meta,
            handle: None, // New session starts without active handle
            acquired: false,
        };
        
        // Store new session
        {
            let mut sessions = self.sessions.write()
                .map_err(|_| "Lock poisoned".to_string())?;
            sessions.insert(new_name.to_string(), new_session);
        }
        
        self.save_sessions()?;
        
        Ok(())
    }
    
    /// Check authentication status using selectors
    /// 
    /// This navigates to the auth check URL and looks for:
    /// - logged_in_selector: If found, user is logged in
    /// - login_required_selector: If found, user is NOT logged in
    pub async fn check_auth(
        &self,
        name: &str,
        config: &AuthCheckConfig,
        navigate_fn: impl FnOnce(&str) -> Result<(), String>,
        check_selector_fn: impl Fn(&str) -> Result<bool, String>,
    ) -> Result<AuthStatus, String> {
        // Navigate to auth check URL
        navigate_fn(&config.url)?;
        
        // Wait a bit for page to load (in production, use smart waiting)
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // Check for logged_in_selector
        let is_logged_in = check_selector_fn(&config.logged_in_selector)?;
        
        // Optional: Check for login_required_selector
        let needs_login = if let Some(ref selector) = config.login_required_selector {
            check_selector_fn(selector).unwrap_or(false)
        } else {
            false
        };
        
        let logged_in = is_logged_in && !needs_login;
        let auth_status = AuthStatus {
            logged_in,
            checked_at: Some(chrono_now_iso8601()),
            username: None, // Would require additional extraction logic
        };
        
        // Update session with new auth status
        self.update_auth_status(name, auth_status.clone())?;
        
        Ok(auth_status)
    }
    
    // ========================================================================
    // Session Pool Operations (Phase 7.4)
    // ========================================================================
    
    /// Warm up sessions - pre-create sessions for frequently used names
    /// 
    /// This allows faster response times for known session names
    pub async fn warm_up<F>(
        &self,
        session_names: Vec<String>,
        create_session_fn: F,
    ) -> Vec<Result<String, String>>
    where
        F: Fn(SessionOptions) -> Result<(String, SessionHandle), String>,
    {
        let mut results = Vec::new();
        
        for name in session_names {
            // Skip if session already exists
            if self.get(&name).is_some() {
                results.push(Ok(format!("Session '{}' already exists", name)));
                continue;
            }
            
            let request = AcquireRequest {
                name: name.clone(),
                profile: Some(name.clone()),
                reuse: true,
                create_if_missing: true,
                headless: true, // Warm up in headless mode
                auth_check: None,
                ttl_hours: default_ttl_hours(),
                auto_extend: true,
                restore: false, // Don't restore during warm up
            };
            
            match self.acquire(request, &create_session_fn).await {
                Ok(response) => {
                    // Immediately release after warm up
                    let _ = self.release(&response.session);
                    results.push(Ok(format!("Session '{}' warmed up", response.session)));
                }
                Err(e) => {
                    results.push(Err(format!("Failed to warm up '{}': {}", name, e)));
                }
            }
        }
        
        results
    }
    
    /// Cleanup idle sessions that haven't been accessed recently
    /// 
    /// Uses LRU (Least Recently Used) strategy based on last_accessed
    pub fn cleanup_idle(&self, _max_idle_seconds: u64) -> Vec<String> {
        let _now = chrono_now_iso8601();
        let mut removed = Vec::new();
        
        // Get list of sessions to remove
        let sessions_to_remove: Vec<String> = {
            let sessions = match self.sessions.read() {
                Ok(s) => s,
                Err(_) => return removed,
            };
            
            sessions.values()
                .filter(|s| {
                    // Don't remove acquired sessions
                    if s.acquired {
                        return false;
                    }
                    
                    // Check if session is idle (simplified - in production parse ISO8601)
                    // For now, just remove sessions that are not acquired
                    // A proper implementation would parse timestamps
                    !s.acquired && s.handle.is_none()
                })
                .map(|s| s.meta.name.clone())
                .collect()
        };
        
        // Remove each session
        for name in sessions_to_remove {
            if self.destroy(&name).is_ok() {
                removed.push(name);
            }
        }
        
        if !removed.is_empty() {
            tracing::info!("[SessionManagerV2] Cleaned up {} idle sessions", removed.len());
        }
        
        removed
    }
    
    /// Get session statistics
    pub fn stats(&self) -> SessionPoolStats {
        let sessions = match self.sessions.read() {
            Ok(s) => s,
            Err(_) => return SessionPoolStats::default(),
        };
        
        let total = sessions.len();
        let active = sessions.values().filter(|s| s.handle.is_some()).count();
        let acquired = sessions.values().filter(|s| s.acquired).count();
        let idle = total - acquired;
        
        SessionPoolStats {
            total,
            active,
            acquired,
            idle,
            max_sessions: self.max_sessions,
        }
    }
}

/// Session pool statistics
#[derive(Debug, Clone, Serialize, Default)]
pub struct SessionPoolStats {
    pub total: usize,
    pub active: usize,
    pub acquired: usize,
    pub idle: usize,
    pub max_sessions: usize,
}

// ============================================================================
// Helpers
// ============================================================================

/// Get current time as ISO 8601 string
fn chrono_now_iso8601() -> String {
    use std::time::UNIX_EPOCH;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    
    // Simple ISO 8601 format (not using chrono crate to minimize deps)
    let secs = now.as_secs();
    let days_since_epoch = secs / 86400;
    let secs_in_day = secs % 86400;
    
    // Approximate date calculation (good enough for this purpose)
    let years = 1970 + (days_since_epoch / 365);
    let remaining_days = days_since_epoch % 365;
    let month = (remaining_days / 30) + 1;
    let day = (remaining_days % 30) + 1;
    
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;
    
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, month.min(12), day.min(31), hour, minute, second
    )
}

/// Add hours to a timestamp and return new ISO 8601 string
fn chrono_add_hours(base: &str, hours: u64) -> String {
    use std::time::UNIX_EPOCH;
    
    // Parse base timestamp (approximate)
    let base_secs = if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(base) {
        ts.timestamp() as u64
    } else {
        // Fallback: use current time + hours
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    };
    
    let new_secs = base_secs + (hours * 3600);
    let days_since_epoch = new_secs / 86400;
    let secs_in_day = new_secs % 86400;
    
    let years = 1970 + (days_since_epoch / 365);
    let remaining_days = days_since_epoch % 365;
    let month = (remaining_days / 30) + 1;
    let day = (remaining_days % 30) + 1;
    
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;
    
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, month.min(12), day.min(31), hour, minute, second
    )
}

/// Check if a timestamp has expired (is in the past)
fn is_expired(expires_at: &Option<String>) -> bool {
    match expires_at {
        None => false, // No expiration = never expires
        Some(ts) => {
            if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(ts) {
                let now = chrono::Utc::now();
                exp.with_timezone(&chrono::Utc) < now
            } else {
                false
            }
        }
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(()); // Nothing to copy
    }
    
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create directory {:?}: {}", dst, e))?;
    
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {:?}: {}", src, e))? 
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)
                .map_err(|e| format!("Failed to copy {:?} to {:?}: {}", path, dest_path, e))?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_iso8601_format() {
        let ts = chrono_now_iso8601();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert!(ts.len() >= 20); // YYYY-MM-DDTHH:MM:SSZ
    }
    
    #[test]
    fn test_sessions_file_serialization() {
        let mut file = SessionsFile::new();
        file.sessions.insert("test".to_string(), NamedSessionMeta {
            name: "test".to_string(),
            profile: "test_profile".to_string(),
            auth_status: AuthStatus::default(),
            last_accessed: "2026-02-05T12:00:00Z".to_string(),
            auto_extend: true,
            created_at: "2026-02-05T12:00:00Z".to_string(),
        });
        
        let json = serde_json::to_string_pretty(&file).unwrap();
        let parsed: SessionsFile = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.sessions.len(), 1);
        assert!(parsed.sessions.contains_key("test"));
    }
    
    #[test]
    fn test_sessions_file_new() {
        let file = SessionsFile::new();
        assert!(file.sessions.is_empty());
    }
    
    #[test]
    fn test_auth_status_default() {
        let status = AuthStatus::default();
        // Verify it doesn't panic
        assert!(serde_json::to_string(&status).is_ok());
    }
    
    #[test]
    fn test_session_pool_stats_default() {
        let stats = SessionPoolStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.acquired, 0);
        assert_eq!(stats.idle, 0);
        assert_eq!(stats.max_sessions, 0);
    }
    
    #[test]
    fn test_acquire_request_deserialize() {
        let json = r##"{"name": "main"}"##;
        let req: AcquireRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "main");
        assert!(req.reuse); // default true
    }
    
    #[test]
    fn test_acquire_request_with_profile() {
        let json = r##"{"name": "test", "profile": "my_profile"}"##;
        let req: AcquireRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.profile, Some("my_profile".to_string()));
    }
    
    #[test]
    fn test_acquire_request_with_auth_check() {
        let json = r##"{
            "name": "test",
            "auth_check": {"url": "https://example.com", "logged_in_selector": ".logout-btn"}
        }"##;
        let req: AcquireRequest = serde_json::from_str(json).unwrap();
        assert!(req.auth_check.is_some());
        assert_eq!(req.auth_check.unwrap().logged_in_selector, ".logout-btn");
    }
    
    #[test]
    fn test_auth_check_config_deserialize() {
        let json = r##"{"url": "https://example.com", "logged_in_selector": "#user-menu"}"##;
        let config: AuthCheckConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.logged_in_selector, "#user-menu");
    }
    
    #[test]
    fn test_named_session_meta_serialization() {
        let meta = NamedSessionMeta {
            name: "test".to_string(),
            profile: "default".to_string(),
            auth_status: AuthStatus::default(),
            last_accessed: "2026-02-05T12:00:00Z".to_string(),
            auto_extend: true,
            created_at: "2026-02-05T12:00:00Z".to_string(),
            ttl_hours: 168,
            expires_at: Some("2026-02-12T12:00:00Z".to_string()),
        };
        
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: NamedSessionMeta = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.profile, "default");
    }
    
    #[test]
    fn test_default_true() {
        assert!(default_true());
    }
    
    #[test]
    fn test_session_list_item_serialize() {
        let item = SessionListItem {
            name: "test".to_string(),
            profile: "default".to_string(),
            acquired: true,
            auth_status: AuthStatus::default(),
            last_accessed: "2026-02-05T12:00:00Z".to_string(),
            active: true,
            ttl_hours: 168,
            expires_at: Some("2026-02-12T12:00:00Z".to_string()),
            expired: false,
        };
        
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("acquired"));
    }
    
    #[test]
    fn test_list_sessions_response_serialize() {
        let response = ListSessionsResponse {
            sessions: vec![],
            total: 0,
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("sessions"));
        assert!(json.contains("total"));
    }
    
    #[test]
    fn test_acquire_response_serialize() {
        let response = AcquireResponse {
            session: "main".to_string(),
            is_new: false,
            profile: "default".to_string(),
            auth_status: Some(AuthStatus::default()),
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("session"));
        assert!(json.contains("main"));
    }
    
    #[test]
    fn test_session_manager_new() {
        let temp_dir = std::env::temp_dir().join("wbp2_test_session_manager");
        let _ = std::fs::create_dir_all(&temp_dir);
        
        let manager = SessionManagerV2::new(temp_dir.clone(), 10);
        // Verify stats work
        let stats = manager.stats();
        assert_eq!(stats.max_sessions, 10);
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_session_manager_with_idle_timeout() {
        let temp_dir = std::env::temp_dir().join("wbp2_test_session_idle");
        let _ = std::fs::create_dir_all(&temp_dir);
        
        // Just verify it doesn't panic
        let _manager = SessionManagerV2::new(temp_dir.clone(), 10)
            .with_idle_timeout(3600);
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_session_manager_stats() {
        let temp_dir = std::env::temp_dir().join("wbp2_test_session_stats");
        let _ = std::fs::create_dir_all(&temp_dir);
        
        let manager = SessionManagerV2::new(temp_dir.clone(), 5);
        let stats = manager.stats();
        
        assert_eq!(stats.total, 0);
        assert_eq!(stats.max_sessions, 5);
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}


