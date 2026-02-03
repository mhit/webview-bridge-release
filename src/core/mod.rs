use std::sync::{Arc, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

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
    // ここに WebView2 のポインタ（後で追加）
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
            options,
            status: SessionStatus::Initializing,
            current_url: None,
        };

        sessions.push(session);
        Ok(id)
    }

    pub fn get_session_status(&self, id: &str) -> Option<SessionStatus> {
        let sessions = self.sessions.lock().unwrap();
        sessions.iter().find(|s| s.id == id).map(|s| s.status)
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
