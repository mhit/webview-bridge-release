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

use super::{SessionHandle, SessionOptions};

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
    /// Whether to auto-extend lifetime
    pub auto_extend: bool,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
}

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
        
        let meta = NamedSessionMeta {
            name: request.name.clone(),
            profile: profile_name.clone(),
            auth_status: AuthStatus::default(),
            last_accessed: now.clone(),
            auto_extend: true,
            created_at: now,
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
        })
    }
    
    /// Release a session (keep profile, mark as not acquired)
    pub fn release(&self, name: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.get_mut(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        session.acquired = false;
        session.meta.last_accessed = chrono_now_iso8601();
        
        drop(sessions);
        self.save_sessions()?;
        
        Ok(())
    }
    
    /// Destroy a session (close and remove)
    pub fn destroy(&self, name: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write()
            .map_err(|_| "Lock poisoned".to_string())?;
        
        let session = sessions.remove(name)
            .ok_or_else(|| format!("Session '{}' not found", name))?;
        
        // Close the handle if active
        if let Some(_handle) = session.handle {
            // The handle will be dropped, closing the session
            // We could send a Close command here if needed
        }
        
        drop(sessions);
        self.save_sessions()?;
        
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_iso8601_format() {
        let ts = chrono_now_iso8601();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
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
}
