use std::sync::{Arc, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

pub mod profile;

// ============================================================================
// Command Types for Main Thread Communication
// ============================================================================

/// Main thread command enum
#[derive(Debug)]
pub enum AppCommand {
    CreateSession {
        options: SessionOptions,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    Navigate {
        id: String,
        url: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    GetStatus {
        id: String,
        resp_tx: oneshot::Sender<Result<SessionStatusInfo, String>>,
    },
    ExecuteScript {
        id: String,
        script: String,
        request_id: String,
        resp_tx: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    CloseSession {
        id: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    Act {
        id: String,
        action: ActionItem,
        resp_tx: oneshot::Sender<Result<serde_json::Value, String>>,
    },
}

/// Action item for Act API (OpenClaw compatible)
/// Format: { "kind": "click", "ref_attr": "#submit", "text": null, "delay_ms": null }
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionItem {
    pub kind: String,
    pub ref_attr: Option<String>,
    pub text: Option<String>,
    pub delay_ms: Option<u64>,
}

/// Session status info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusInfo {
    pub id: String,
    pub status: String,
    pub url: Option<String>,
    pub options: Option<SessionOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOptions {
    pub profile: String,
    pub headless: bool,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    Initializing,
    Ready,
    Busy,
    Error,
}

pub struct Session {
    pub id: String,
    pub options: SessionOptions,
    pub status: SessionStatus,
    pub current_url: Option<String>,
    // Note: WebViewInstance integration would be here
    // For now, using mock implementation
}

pub struct SessionManager {
    sessions: Arc<Mutex<Vec<Session>>>,
    max_sessions: usize,
}

impl SessionManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
        }
    }

    pub fn create_session(&self, options: SessionOptions) -> Result<String, String> {
        let mut sessions = self.sessions.lock().unwrap();
        
        if sessions.len() >= self.max_sessions {
            return Err("Maximum session limit reached".to_string());
        }

        let id = Uuid::new_v4().to_string();
        let session = Session {
            id: id.clone(),
            options: options.clone(),
            status: SessionStatus::Initializing,
            current_url: None,
        };

        sessions.push(session);
        
        // Simulate session becoming ready
        let id_clone = id.clone();
        let sessions_clone = self.sessions.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut sessions = sessions_clone.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id_clone) {
                session.status = SessionStatus::Ready;
            }
        });
        
        Ok(id)
    }

    pub fn get_session_status(&self, id: &str) -> Option<SessionStatus> {
        let sessions = self.sessions.lock().unwrap();
        sessions.iter().find(|s| s.id == id).map(|s| s.status)
    }
    
    pub fn get_info(&self, id: &str) -> Result<SessionStatusInfo, String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| SessionStatusInfo {
                id: s.id.clone(),
                status: format!("{:?}", s.status),
                url: s.current_url.clone(),
                options: Some(s.options.clone()),
            })
            .ok_or_else(|| format!("Session not found: {}", id))
    }
    
    pub fn update_status(&self, id: &str, status: SessionStatus) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        sessions
            .iter_mut()
            .find(|s| s.id == id)
            .map(|s| s.status = status)
            .ok_or_else(|| format!("Session not found: {}", id))
    }
    
    pub fn update_url(&self, id: &str, url: String) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        sessions
            .iter_mut()
            .find(|s| s.id == id)
            .map(|s| s.current_url = Some(url))
            .ok_or_else(|| format!("Session not found: {}", id))
    }
    
    pub fn remove_session(&self, id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        let original_len = sessions.len();
        sessions.retain(|s| s.id != id);
        
        if sessions.len() < original_len {
            Ok(())
        } else {
            Err(format!("Session not found: {}", id))
        }
    }
    
    pub fn navigate(&self, id: &str, url: String) -> Result<(), String> {
        // Update URL
        self.update_url(id, url.clone())?;
        
        // Mark as busy during navigation
        self.update_status(id, SessionStatus::Busy)?;
        
        // Simulate navigation completion
        let id_clone = id.to_string();
        let sessions_clone = self.sessions.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let _ = sessions_clone.lock().unwrap().iter_mut().find(|s| s.id == id_clone).map(|s| s.status = SessionStatus::Ready);
        });
        
        Ok(())
    }
    
    pub fn execute_script(&self, id: &str, _script: String) -> Result<String, String> {
        // Check if session exists and is ready
        let status = self.get_session_status(id).ok_or_else(|| format!("Session not found: {}", id))?;
        if status != SessionStatus::Ready {
            return Err(format!("Session not ready: {:?}", status));
        }
        
        // In a real implementation, this would execute JavaScript in WebView2
        // For now, return a mock result that simulates document.title
        // Note: This is a simplified version - full WebView2 integration is complex
        Ok("Mock Page Title".to_string())
    }
    
    pub fn act(&self, id: &str, action: ActionItem) -> Result<serde_json::Value, String> {
        // Check if session exists and is ready
        let status = self.get_session_status(id).ok_or_else(|| format!("Session not found: {}", id))?;
        if status != SessionStatus::Ready {
            return Err(format!("Session not ready: {:?}", status));
        }
        
        // Simulate action execution
        // In a real implementation, this would interact with WebView2
        Ok(serde_json::json!({
            "result": format!("executed {}: {:?}", action.kind, action.ref_attr)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let manager = SessionManager::new(5);
        let options = SessionOptions {
            profile: "default".to_string(),
            headless: true,
            user_agent: None,
        };

        let result = manager.create_session(options);
        assert!(result.is_ok());
        
        let id = result.unwrap();
        assert_eq!(manager.get_session_status(&id), Some(SessionStatus::Initializing));
    }

    #[test]
    fn test_session_limit() {
        let manager = SessionManager::new(1);
        let options = SessionOptions {
            profile: "test".to_string(),
            headless: true,
            user_agent: None,
        };

        assert!(manager.create_session(options.clone()).is_ok());
        assert!(manager.create_session(options).is_err());
    }
}
