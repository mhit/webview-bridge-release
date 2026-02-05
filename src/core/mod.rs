use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

pub mod profile;
pub mod screenshot_v2;
pub mod session_v2;
pub mod wait_v2;
pub mod websocket;

// Export WM_CHECK_QUEUE for use in other modules
pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

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
    Snapshot {
        id: String,
        format: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    Screenshot {
        id: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    GetCookies {
        id: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    SetCookies {
        id: String,
        cookies: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    WaitForSelector {
        id: String,
        selector: String,
        timeout_ms: u64,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    Extract {
        id: String,
        selector: String,
        attribute: String,
        extract_all: bool,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
}

/// Session thread command enum
#[derive(Debug)]
pub enum SessionCommand {
    Navigate {
        url: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    ExecuteScript {
        script: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    GetStatus {
        resp_tx: oneshot::Sender<SessionStatus>,
    },
    Close {
        resp_tx: oneshot::Sender<()>,
    },
    Act {
        action: ActionItem,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    Snapshot {
        format: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    Screenshot {
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    GetCookies {
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    SetCookies {
        cookies: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    WaitForSelector {
        selector: String,
        timeout_ms: u64,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    Extract {
        selector: String,
        attribute: String,
        extract_all: bool,
        resp_tx: oneshot::Sender<Result<String, String>>,
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

/// Handle to a session running in its own thread
#[derive(Clone)]
pub struct SessionHandle {
    pub id: String,
    pub options: SessionOptions,
    pub status: Arc<Mutex<SessionStatus>>,
    pub current_url: Arc<Mutex<Option<String>>>,
    pub last_accessed: Arc<Mutex<Instant>>,
    cmd_tx: mpsc::UnboundedSender<SessionCommand>,
}

impl SessionHandle {
    pub fn new(
        id: String,
        options: SessionOptions,
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    ) -> Self {
        Self {
            id,
            options,
            status: Arc::new(Mutex::new(SessionStatus::Initializing)),
            current_url: Arc::new(Mutex::new(None)),
            last_accessed: Arc::new(Mutex::new(Instant::now())),
            cmd_tx,
        }
    }

    pub fn send_command(&self, cmd: SessionCommand) -> Result<(), String> {
        // Update last accessed time
        *self.last_accessed.lock().unwrap() = Instant::now();
        self.cmd_tx
            .send(cmd)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    pub fn touch(&self) {
        *self.last_accessed.lock().unwrap() = Instant::now();
    }

    pub fn idle_duration(&self) -> Duration {
        self.last_accessed.lock().unwrap().elapsed()
    }

    pub fn get_status_sync(&self) -> SessionStatus {
        *self.status.lock().unwrap()
    }

    pub fn get_current_url_sync(&self) -> Option<String> {
        self.current_url.lock().unwrap().clone()
    }
}

pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, SessionHandle>>>,
    max_sessions: usize,
    idle_timeout_secs: u64,
}

impl SessionManager {
    pub fn new(max_sessions: usize) -> Self {
        Self::with_idle_timeout(max_sessions, 300) // Default 5 minutes
    }

    pub fn with_idle_timeout(max_sessions: usize, idle_timeout_secs: u64) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            max_sessions,
            idle_timeout_secs,
        }
    }

    pub fn create_session(&self, options: SessionOptions) -> Result<String, String> {
        let mut sessions = self.sessions.lock().unwrap();

        if sessions.len() >= self.max_sessions {
            return Err("Maximum session limit reached".to_string());
        }

        let id = Uuid::new_v4().to_string();

        // Create channel for session thread communication
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();

        // Create session handle
        let handle = SessionHandle::new(id.clone(), options.clone(), cmd_tx);
        sessions.insert(id.clone(), handle);

        // Spawn session thread
        let id_clone = id.clone();
        let sessions_arc = self.sessions.clone();
        std::thread::spawn(move || {
            Self::session_thread(id_clone, options, cmd_rx, sessions_arc);
        });

        Ok(id)
    }

    /// Session thread - runs WebViewInstance in its own thread
    fn session_thread(
        id: String,
        options: SessionOptions,
        cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
        sessions: Arc<Mutex<HashMap<String, SessionHandle>>>,
    ) {
        tracing::info!(
            "[Session:{}] Thread started, profile={}",
            id,
            options.profile
        );

        // Create user data folder for this session
        let user_data_folder = format!("./profiles/{}/{}", options.profile, id);

        // Create WebViewInstance
        let mut webview = match crate::webview::webview_instance::WebViewInstance::new(
            &format!("WebView Bridge - {}", id),
            !options.headless,
        ) {
            Ok(wv) => {
                tracing::info!("[Session:{}] WebViewInstance created", id);
                wv
            }
            Err(e) => {
                tracing::error!("[Session:{}] Failed to create WebViewInstance: {:?}", id, e);
                // Mark session as error
                if let Some(handle) = sessions.lock().unwrap().get(&id) {
                    *handle.status.lock().unwrap() = SessionStatus::Error;
                }
                return;
            }
        };

        // Initialize WebView2
        if let Err(e) = webview.initialize(&user_data_folder) {
            tracing::error!("[Session:{}] Failed to initialize WebView2: {:?}", id, e);
            if let Some(handle) = sessions.lock().unwrap().get(&id) {
                *handle.status.lock().unwrap() = SessionStatus::Error;
            }
            return;
        }

        // Process commands using message loop and channel
        let mut cmd_rx = cmd_rx;
        loop {
            // Win32 Message Loop
            unsafe {
                let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    windows::Win32::Foundation::HWND::default(),
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                )
                .as_bool()
                {
                    if msg.message == crate::webview::webview_instance::WM_WEBVIEW_CREATED {
                        tracing::info!("[Session:{}] WebView created, claiming controller", id);
                        webview.claim_controller();
                        if let Some(handle) = sessions.lock().unwrap().get(&id) {
                            *handle.status.lock().unwrap() = SessionStatus::Ready;
                        }
                    }
                    windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                    windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                }
            }

            // Process Session Commands
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    match cmd {
                        SessionCommand::Navigate { url, resp_tx } => {
                            tracing::debug!("[Session:{}] Navigating to: {}", id, url);

                            // Ensure controller is ready
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err(
                                    "WebView is not ready (controller missing)".to_string(),
                                ));
                                continue;
                            }

                            let url_clone = url.clone();
                            let result = webview
                                .navigate(&url)
                                .map_err(|e| format!("Navigate failed: {:?}", e));

                            if result.is_ok() {
                                if let Some(handle) = sessions.lock().unwrap().get(&id) {
                                    *handle.current_url.lock().unwrap() = Some(url_clone);
                                }
                            }

                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ExecuteScript { script, resp_tx } => {
                            tracing::debug!("[Session:{}] Executing script", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview
                                .execute_script(&script, Uuid::new_v4().to_string())
                                .map_err(|e| format!("Execute script failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::GetStatus { resp_tx } => {
                            let status = if let Some(handle) = sessions.lock().unwrap().get(&id) {
                                *handle.status.lock().unwrap()
                            } else {
                                SessionStatus::Error
                            };
                            let _ = resp_tx.send(status);
                        }
                        SessionCommand::Close { resp_tx } => {
                            tracing::info!("[Session:{}] Closing session", id);
                            let _ = webview.close();
                            let _ = resp_tx.send(());
                            break;
                        }
                        SessionCommand::Act { action, resp_tx } => {
                            tracing::debug!("[Session:{}] Act: {:?}", id, action.kind);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = match action.kind.as_str() {
                                "click" => webview
                                    .click(
                                        &action.ref_attr.as_ref().map(|s| s.as_str()).unwrap_or(""),
                                    )
                                    .map(|_| "Clicked".to_string())
                                    .map_err(|e| format!("Click failed: {:?}", e)),
                                "type" => webview
                                    .type_text(
                                        &action.ref_attr.as_ref().map(|s| s.as_str()).unwrap_or(""),
                                        &action.text.as_ref().map(|s| s.as_str()).unwrap_or(""),
                                    )
                                    .map(|_| "Text typed".to_string())
                                    .map_err(|e| format!("Type failed: {:?}", e)),
                                "press" => webview
                                    .press_key(
                                        &action.text.as_ref().map(|s| s.as_str()).unwrap_or(""),
                                    )
                                    .map(|_| "Key pressed".to_string())
                                    .map_err(|e| format!("Press key failed: {:?}", e)),
                                _ => Err(format!("Unknown action kind: {}", action.kind)),
                            };
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::Snapshot { format, resp_tx } => {
                            tracing::debug!("[Session:{}] Snapshot: format={}", id, format);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.snapshot(&format);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::Screenshot { resp_tx } => {
                            tracing::debug!("[Session:{}] Screenshot", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.screenshot();
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::GetCookies { resp_tx } => {
                            tracing::debug!("[Session:{}] GetCookies", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.get_cookies_json();
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetCookies { cookies, resp_tx } => {
                            tracing::debug!("[Session:{}] SetCookies", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.set_cookies_json(&cookies);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::WaitForSelector { selector, timeout_ms, resp_tx } => {
                            tracing::debug!("[Session:{}] WaitForSelector: {}", id, selector);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.wait_for_selector(&selector, timeout_ms);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::Extract { selector, attribute, extract_all, resp_tx } => {
                            tracing::debug!("[Session:{}] Extract: {} attr={}", id, selector, attribute);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("WebView is not ready".to_string()));
                                continue;
                            }
                            let result = webview.extract(&selector, &attribute, extract_all);
                            let _ = resp_tx.send(result);
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No commands, use shorter sleep for better responsiveness
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }
        }

        tracing::info!("[Session:{}] Thread exiting", id);
    }

    pub fn get_session_status(&self, id: &str) -> Option<SessionStatus> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(id).map(|h| h.get_status_sync())
    }

    pub fn get_info(&self, id: &str) -> Result<SessionStatusInfo, String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(id)
            .map(|h| SessionStatusInfo {
                id: h.id.clone(),
                status: format!("{:?}", h.get_status_sync()),
                url: h.get_current_url_sync(),
                options: Some(h.options.clone()),
            })
            .ok_or_else(|| format!("Session not found: {}", id))
    }

    pub async fn remove_session(&self, id: &str) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions.get(id).cloned()
        };

        if let Some(handle) = handle {
            // Send close command to session thread
            let (tx, rx) = oneshot::channel();
            let _ = handle.send_command(SessionCommand::Close { resp_tx: tx });
            let _ = rx.await;
        }

        let mut sessions = self.sessions.lock().unwrap();
        sessions
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| format!("Session not found: {}", id))
    }

    pub async fn navigate(&self, id: &str, url: String) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Navigate { url, resp_tx: tx })?;

        // Wait for navigation to complete
        match rx.await {
            Ok(Ok(_)) => {
                // Update status
                if let Some(h) = self.sessions.lock().unwrap().get(id) {
                    *h.status.lock().unwrap() = SessionStatus::Ready;
                }
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Navigation response channel closed".to_string()),
        }
    }

    pub async fn execute_script(&self, id: &str, script: String) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ExecuteScript {
            script,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Execute script response channel closed".to_string()),
        }
    }

    pub async fn act(&self, id: &str, action: ActionItem) -> Result<serde_json::Value, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Act {
            action,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(serde_json::json!({ "result": result })),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Act response channel closed".to_string()),
        }
    }

    pub async fn snapshot(&self, id: &str, format: String) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Snapshot {
            format,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Snapshot response channel closed".to_string()),
        }
    }

    pub async fn screenshot(&self, id: &str) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Screenshot { resp_tx: tx })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Screenshot response channel closed".to_string()),
        }
    }

    pub async fn get_cookies(&self, id: &str) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::GetCookies { resp_tx: tx })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("GetCookies response channel closed".to_string()),
        }
    }

    pub async fn set_cookies(&self, id: &str, cookies: String) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SetCookies { cookies, resp_tx: tx })?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SetCookies response channel closed".to_string()),
        }
    }

    pub async fn wait_for_selector(&self, id: &str, selector: String, timeout_ms: u64) -> Result<bool, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::WaitForSelector { selector, timeout_ms, resp_tx: tx })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("WaitForSelector response channel closed".to_string()),
        }
    }

    pub async fn extract(&self, id: &str, selector: String, attribute: String, extract_all: bool) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Extract { selector, attribute, extract_all, resp_tx: tx })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Extract response channel closed".to_string()),
        }
    }

    /// Cleanup idle sessions that have exceeded the timeout
    pub async fn cleanup_idle_sessions(&self) -> Vec<String> {
        let idle_timeout = Duration::from_secs(self.idle_timeout_secs);
        let sessions_to_remove: Vec<String> = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .iter()
                .filter(|(_, handle)| handle.idle_duration() > idle_timeout)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut removed = Vec::new();
        for id in sessions_to_remove {
            tracing::info!("[SessionManager] Cleaning up idle session: {}", id);
            if self.remove_session(&id).await.is_ok() {
                removed.push(id);
            }
        }

        if !removed.is_empty() {
            tracing::info!("[SessionManager] Cleaned up {} idle sessions", removed.len());
        }

        removed
    }

    /// Get the current session count
    pub fn get_session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    /// List all session IDs with their idle duration
    pub fn list_sessions(&self) -> Vec<(String, Duration, SessionStatus)> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .map(|(id, handle)| {
                (id.clone(), handle.idle_duration(), handle.get_status_sync())
            })
            .collect()
    }

    /// Get the idle timeout setting
    pub fn get_idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
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
        // Session should be created, status may vary
        let status = manager.get_session_status(&id);
        assert!(status.is_some());
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
