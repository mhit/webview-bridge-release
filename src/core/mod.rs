use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

pub mod ai;
pub mod comm;
pub mod config;
pub mod cookie_import;
pub mod download;
pub mod goal;
pub mod macro_engine;
pub mod media;
pub mod network;
pub mod profile;
pub mod screenshot_v2;
pub mod session_v2;
pub mod upload;
pub mod wait_v2;
pub mod websocket;

// Export Network Types
pub use network::*;

// Export WM_CHECK_QUEUE for use in other modules
pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

// ============================================================================
// Command Types for Main Thread Communication
// ============================================================================

/// Main thread command enum (V2 only)
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
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    CloseSession {
        id: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
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
        frame: Option<String>,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    SetVisibility {
        id: String,
        visible: bool,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    // Device simulation commands
    SimulateDevice {
        id: String,
        device_name: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    SetViewport {
        id: String,
        width: u32,
        height: u32,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    SetUserAgent {
        id: String,
        user_agent: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    // CDP screenshot (supports full page and iframe)
    ScreenshotCdp {
        id: String,
        full_page: bool,
        format: String,
        quality: Option<u32>,
        frame: Option<String>,
        resp_tx: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    // Reset device emulation
    ResetDeviceEmulation {
        id: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Set viewport via CDP (with device metrics)
    SetViewportCdp {
        id: String,
        width: u32,
        height: u32,
        device_scale_factor: f64,
        is_mobile: bool,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // CDP Input commands (for bot detection evasion)
    ClickCdp {
        id: String,
        selector: String,
        human_mode: bool,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    TypeCdp {
        id: String,
        text: String,
        char_delay_ms: u64,
        human_mode: bool,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    PressKeyCdp {
        id: String,
        key: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    // Network Monitoring
    ManageNetwork {
        id: String,
        action: NetworkAction,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Frame (iframe) operations
    GetFrames {
        id: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    ExecuteInFrame {
        id: String,
        script: String,
        frame: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Form file injection via CDP
    FormInjectFile {
        id: String,
        selector: String,
        file_paths: Vec<String>,
        frame: Option<String>,
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
        frame: Option<String>,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    Extract {
        selector: String,
        attribute: String,
        extract_all: bool,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    /// Set window visibility (pseudo-headless mode)
    SetVisibility {
        visible: bool,
        resp_tx: oneshot::Sender<Result<bool, String>>,
    },
    /// Bring window to front for user interaction
    BringToFront {
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    // Device simulation commands
    SimulateDevice {
        device_name: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    SetViewport {
        width: u32,
        height: u32,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    SetUserAgent {
        user_agent: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    // CDP Screenshot with full page and iframe support
    ScreenshotCdp {
        full_page: bool,
        format: String,
        quality: Option<u32>,
        frame: Option<String>,
        resp_tx: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    // Reset device emulation
    ResetDeviceEmulation {
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Set viewport via CDP
    SetViewportCdp {
        width: u32,
        height: u32,
        device_scale_factor: f64,
        is_mobile: bool,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // CDP Input commands (for bot detection evasion)
    ClickCdp {
        selector: String,
        human_mode: bool,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    TypeCdp {
        text: String,
        char_delay_ms: u64,
        human_mode: bool,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    PressKeyCdp {
        key: String,
        resp_tx: oneshot::Sender<Result<(), String>>,
    },
    ManageNetwork {
        action: NetworkAction,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Frame (iframe) operations
    GetFrames {
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    ExecuteInFrame {
        script: String,
        frame: String,
        resp_tx: oneshot::Sender<Result<String, String>>,
    },
    // Form file injection via CDP
    FormInjectFile {
        selector: String,
        file_paths: Vec<String>,
        frame: Option<String>,
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
    /// Window title (defaults to session ID if not set)
    #[serde(default)]
    pub window_title: Option<String>,
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
            return Err(format!(
                "Maximum session limit reached ({}/{}). Release unused sessions: wb session release <name> or POST /session/release",
                sessions.len(),
                self.max_sessions
            ));
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

        // Create user data folder based on profile name only (NOT session ID)
        // This ensures cookies persist across sessions with the same profile
        // Use absolute path under data_dir (~/.webview-bridge/) to avoid
        // permission issues when launched from Program Files
        let user_data_folder = crate::core::config::AppConfig::profile_dir(&options.profile)
            .to_string_lossy()
            .to_string();

        // Create WebViewInstance
        let title = options
            .window_title
            .as_deref()
            .map(|t| format!("WB: {}", t))
            .unwrap_or_else(|| format!("WebView Bridge - {}", id));
        let mut webview =
            match crate::webview::webview_instance::WebViewInstance::new(&title, !options.headless)
            {
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
            tracing::error!(
                "[Session:{}] Failed to initialize WebView2: {:?}. Ensure WebView2 Runtime is installed (comes with Microsoft Edge). Install: winget install Microsoft.EdgeWebView2Runtime",
                id,
                e
            );
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
                                    "Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string(),
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
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
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
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
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
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.snapshot(&format);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::Screenshot { resp_tx } => {
                            tracing::info!("[Session:{}] Screenshot command received", id);
                            if !webview.is_ready() {
                                tracing::warn!("[Session:{}] WebView not ready", id);
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }

                            // Improved screenshot script with CSP bypass via fetch+eval
                            // and multiple CDN fallbacks
                            let init_script = r#"
                                (function() {
                                    window.__wbp_ss_data = null;
                                    window.__wbp_ss_error = null;
                                    window.__wbp_ss_status = 'initializing';
                                    
                                    function startCapture() {
                                        window.__wbp_ss_status = 'capturing';
                                        try {
                                            html2canvas(document.body, {
                                                useCORS: true,
                                                allowTaint: true,
                                                scale: 1,
                                                logging: false,
                                                removeContainer: true,
                                                foreignObjectRendering: false,
                                                windowWidth: document.documentElement.scrollWidth,
                                                windowHeight: document.documentElement.scrollHeight
                                            }).then(function(canvas) {
                                                window.__wbp_ss_data = canvas.toDataURL('image/png').replace('data:image/png;base64,', '');
                                                window.__wbp_ss_status = 'done';
                                            }).catch(function(e) {
                                                window.__wbp_ss_error = 'html2canvas error: ' + e.message;
                                                window.__wbp_ss_status = 'error';
                                            });
                                        } catch(e) {
                                            window.__wbp_ss_error = 'startCapture exception: ' + e.message;
                                            window.__wbp_ss_status = 'error';
                                        }
                                    }
                                    
                                    // Check if html2canvas is already loaded
                                    if (typeof html2canvas !== 'undefined') {
                                        startCapture();
                                        return 'html2canvas_ready';
                                    }
                                    
                                    // Try loading via fetch+eval to bypass CSP script-src restrictions
                                    var cdns = [
                                        'https://cdnjs.cloudflare.com/ajax/libs/html2canvas/1.4.1/html2canvas.min.js',
                                        'https://cdn.jsdelivr.net/npm/html2canvas@1.4.1/dist/html2canvas.min.js',
                                        'https://unpkg.com/html2canvas@1.4.1/dist/html2canvas.min.js'
                                    ];
                                    
                                    window.__wbp_ss_status = 'loading';
                                    
                                    async function tryLoadFromCDN(index) {
                                        if (index >= cdns.length) {
                                            // All CDNs failed, try script tag as last resort
                                            tryScriptTag(0);
                                            return;
                                        }
                                        
                                        try {
                                            var response = await fetch(cdns[index]);
                                            if (!response.ok) throw new Error('HTTP ' + response.status);
                                            var code = await response.text();
                                            
                                            // Execute via Function constructor (may work where eval is blocked)
                                            try {
                                                (new Function(code))();
                                            } catch(e) {
                                                // Try eval as fallback
                                                eval(code);
                                            }
                                            
                                            if (typeof html2canvas !== 'undefined') {
                                                startCapture();
                                            } else {
                                                throw new Error('html2canvas not defined after eval');
                                            }
                                        } catch(e) {
                                            console.warn('CDN ' + index + ' failed:', e.message);
                                            tryLoadFromCDN(index + 1);
                                        }
                                    }
                                    
                                    function tryScriptTag(index) {
                                        if (index >= cdns.length) {
                                            window.__wbp_ss_error = 'Failed to load html2canvas from all sources';
                                            window.__wbp_ss_status = 'error';
                                            return;
                                        }
                                        
                                        var script = document.createElement('script');
                                        script.src = cdns[index];
                                        script.onload = function() {
                                            if (typeof html2canvas !== 'undefined') {
                                                startCapture();
                                            } else {
                                                tryScriptTag(index + 1);
                                            }
                                        };
                                        script.onerror = function() {
                                            tryScriptTag(index + 1);
                                        };
                                        document.head.appendChild(script);
                                    }
                                    
                                    tryLoadFromCDN(0);
                                    return 'loading_started';
                                })()
                            "#;

                            // Execute init script
                            let init_result =
                                webview.execute_script(init_script, Uuid::new_v4().to_string());
                            if init_result.is_err() {
                                let _ = resp_tx.send(Err("Failed to start capture".to_string()));
                                continue;
                            }

                            // Step 2: Poll for result (up to 15 seconds for slow pages)
                            let poll_script = r#"
                                JSON.stringify({
                                    status: window.__wbp_ss_status || 'unknown',
                                    hasData: !!window.__wbp_ss_data,
                                    dataLen: window.__wbp_ss_data ? window.__wbp_ss_data.length : 0,
                                    error: window.__wbp_ss_error
                                })
                            "#;

                            let mut result: Result<String, String> =
                                Err("Screenshot timeout".to_string());

                            for i in 0..150 {
                                // 15 seconds max
                                std::thread::sleep(std::time::Duration::from_millis(100));

                                if let Ok(status_json) =
                                    webview.execute_script(poll_script, Uuid::new_v4().to_string())
                                {
                                    // Parse status JSON
                                    if let Ok(status) =
                                        serde_json::from_str::<serde_json::Value>(&status_json)
                                    {
                                        let ss_status = status
                                            .get("status")
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("unknown");
                                        let has_data = status
                                            .get("hasData")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false);
                                        let error = status.get("error").and_then(|e| e.as_str());

                                        if ss_status == "done" && has_data {
                                            // Get the actual data
                                            if let Ok(data) = webview.execute_script(
                                                "window.__wbp_ss_data",
                                                Uuid::new_v4().to_string(),
                                            ) {
                                                let clean_data = data.trim_matches('"').to_string();
                                                if clean_data.starts_with("iVBOR")
                                                    || clean_data.len() > 1000
                                                {
                                                    tracing::info!(
                                                        "[Session:{}] Screenshot success, len={}",
                                                        id,
                                                        clean_data.len()
                                                    );
                                                    result = Ok(clean_data);
                                                    break;
                                                }
                                            }
                                        } else if ss_status == "error" {
                                            let err_msg = error.unwrap_or("Unknown error");
                                            tracing::warn!(
                                                "[Session:{}] html2canvas error: {}, trying native CapturePreview",
                                                id,
                                                err_msg
                                            );
                                            // html2canvas failed (likely CSP), try native API
                                            result =
                                                Err(format!("html2canvas failed: {}", err_msg));
                                            break;
                                        }

                                        // Log progress every 2 seconds
                                        if i % 20 == 0 && i > 0 {
                                            tracing::debug!(
                                                "[Session:{}] Screenshot status: {}",
                                                id,
                                                ss_status
                                            );
                                        }
                                    }
                                }
                            }

                            // Cleanup html2canvas state
                            let _ = webview.execute_script(
                                "delete window.__wbp_ss_data; delete window.__wbp_ss_error; delete window.__wbp_ss_status;",
                                Uuid::new_v4().to_string()
                            );

                            // If html2canvas failed, try native CapturePreview API
                            if result.is_err() {
                                tracing::info!(
                                    "[Session:{}] Falling back to native CapturePreview API",
                                    id
                                );
                                match webview.capture_preview_native() {
                                    Ok(png_data) => {
                                        // Convert PNG bytes to base64
                                        use base64::{
                                            Engine as _, engine::general_purpose::STANDARD,
                                        };
                                        let base64_data = STANDARD.encode(&png_data);
                                        tracing::info!(
                                            "[Session:{}] Native screenshot success, size={} bytes",
                                            id,
                                            base64_data.len()
                                        );
                                        result = Ok(base64_data);
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "[Session:{}] Native CapturePreview also failed: {}",
                                            id,
                                            e
                                        );
                                        // Keep the original error
                                    }
                                }
                            }

                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::GetCookies { resp_tx } => {
                            tracing::debug!("[Session:{}] GetCookies", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.get_cookies_json();
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetCookies { cookies, resp_tx } => {
                            tracing::debug!("[Session:{}] SetCookies", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.set_cookies_json(&cookies);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::WaitForSelector {
                            selector,
                            timeout_ms,
                            frame,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] WaitForSelector: {} frame={:?}",
                                id,
                                selector,
                                frame
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result =
                                webview.wait_for_selector(&selector, timeout_ms, frame.as_deref());
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::Extract {
                            selector,
                            attribute,
                            extract_all,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] Extract: {} attr={}",
                                id,
                                selector,
                                attribute
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.extract(&selector, &attribute, extract_all);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetVisibility { visible, resp_tx } => {
                            tracing::debug!("[Session:{}] SetVisibility: {}", id, visible);
                            webview.set_visible(visible);
                            let is_visible = webview.is_visible();
                            let _ = resp_tx.send(Ok(is_visible));
                        }
                        SessionCommand::BringToFront { resp_tx } => {
                            tracing::debug!("[Session:{}] BringToFront", id);
                            webview.bring_to_front();
                            let _ = resp_tx.send(Ok(()));
                        }
                        SessionCommand::SimulateDevice {
                            device_name,
                            resp_tx,
                        } => {
                            tracing::debug!("[Session:{}] SimulateDevice: {}", id, device_name);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview
                                .simulate_device(&device_name)
                                .map_err(|e| format!("Device simulation failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetViewport {
                            width,
                            height,
                            resp_tx,
                        } => {
                            tracing::debug!("[Session:{}] SetViewport: {}x{}", id, width, height);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview
                                .set_viewport(width, height)
                                .map_err(|e| format!("Set viewport failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetUserAgent {
                            user_agent,
                            resp_tx,
                        } => {
                            tracing::debug!("[Session:{}] SetUserAgent", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview
                                .set_user_agent(&user_agent)
                                .map_err(|e| format!("Set user agent failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ScreenshotCdp {
                            full_page,
                            format,
                            quality,
                            frame,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] ScreenshotCdp: full_page={}, frame={:?}",
                                id,
                                full_page,
                                frame
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = if let Some(ref frame_spec) = frame {
                                webview.capture_screenshot_frame(frame_spec, &format, quality)
                            } else {
                                webview.capture_screenshot_cdp(full_page, &format, quality)
                            };
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ResetDeviceEmulation { resp_tx } => {
                            tracing::debug!("[Session:{}] ResetDeviceEmulation", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview
                                .reset_device_emulation()
                                .map_err(|e| format!("Reset device emulation failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::SetViewportCdp {
                            width,
                            height,
                            device_scale_factor,
                            is_mobile,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] SetViewportCdp: {}x{}",
                                id,
                                width,
                                height
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview
                                .set_viewport_cdp(width, height, device_scale_factor, is_mobile)
                                .map_err(|e| format!("Set viewport CDP failed: {:?}", e));
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ClickCdp {
                            selector,
                            human_mode,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] ClickCdp: {}, human={}",
                                id,
                                selector,
                                human_mode
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.click_selector_cdp(&selector, human_mode);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::TypeCdp {
                            text,
                            char_delay_ms,
                            human_mode,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] TypeCdp: {} chars, human={}",
                                id,
                                text.len(),
                                human_mode
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.type_cdp(&text, char_delay_ms, human_mode);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::PressKeyCdp { key, resp_tx } => {
                            tracing::debug!("[Session:{}] PressKeyCdp: {}", id, key);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.press_key_cdp(&key);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ManageNetwork { action, resp_tx } => {
                            tracing::debug!("[Session:{}] ManageNetwork: {:?}", id, action);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.manage_network(action);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::GetFrames { resp_tx } => {
                            tracing::debug!("[Session:{}] GetFrames", id);
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.get_frames().map(|v| v.to_string());
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::ExecuteInFrame {
                            script,
                            frame,
                            resp_tx,
                        } => {
                            tracing::debug!(
                                "[Session:{}] ExecuteInFrame: frame={}, script_len={}",
                                id,
                                frame,
                                script.len()
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = webview.execute_in_frame(&script, &frame);
                            let _ = resp_tx.send(result);
                        }
                        SessionCommand::FormInjectFile {
                            selector,
                            file_paths,
                            frame,
                            resp_tx,
                        } => {
                            tracing::info!(
                                "[Session:{}] FormInjectFile: selector={}, files={:?}",
                                id,
                                selector,
                                file_paths
                            );
                            if !webview.is_ready() {
                                let _ = resp_tx.send(Err("Session is not acquired. Call 'wb session acquire <name>' or POST /session/acquire first.".to_string()));
                                continue;
                            }
                            let result = if let Some(ref frame_spec) = frame {
                                webview.set_file_input_files_in_frame(
                                    &selector,
                                    &file_paths,
                                    frame_spec,
                                )
                            } else {
                                webview.set_file_input_files(&selector, &file_paths)
                            };
                            let _ = resp_tx.send(result);
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // Check if window was closed by user (X button)
                    if !webview.is_window_valid() {
                        tracing::info!("[Session:{}] Window destroyed by user, exiting thread", id);
                        break;
                    }
                    // No commands, use shorter sleep for better responsiveness
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }
        }

        // Notify SessionManagerV2 that this session is no longer active
        // This handles the case where user closes the window via X button
        crate::api_v2::get_session_manager_v2().clear_handle_by_id(&id);

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
        handle.send_command(SessionCommand::SetCookies {
            cookies,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SetCookies response channel closed".to_string()),
        }
    }

    pub async fn wait_for_selector(
        &self,
        id: &str,
        selector: String,
        timeout_ms: u64,
        frame: Option<String>,
    ) -> Result<bool, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::WaitForSelector {
            selector,
            timeout_ms,
            frame,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("WaitForSelector response channel closed".to_string()),
        }
    }

    pub async fn extract(
        &self,
        id: &str,
        selector: String,
        attribute: String,
        extract_all: bool,
    ) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::Extract {
            selector,
            attribute,
            extract_all,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Extract response channel closed".to_string()),
        }
    }

    /// Set window visibility (pseudo-headless mode)
    pub async fn set_visibility(&self, id: &str, visible: bool) -> Result<bool, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SetVisibility {
            visible,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(is_visible)) => Ok(is_visible),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SetVisibility response channel closed".to_string()),
        }
    }

    /// Bring window to front for user interaction
    pub async fn bring_to_front(&self, id: &str) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::BringToFront { resp_tx: tx })?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("BringToFront response channel closed".to_string()),
        }
    }

    /// Simulate a device (set viewport and user agent based on preset)
    pub async fn simulate_device(&self, id: &str, device_name: String) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SimulateDevice {
            device_name,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SimulateDevice response channel closed".to_string()),
        }
    }

    /// Set viewport dimensions
    pub async fn set_viewport(&self, id: &str, width: u32, height: u32) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SetViewport {
            width,
            height,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SetViewport response channel closed".to_string()),
        }
    }

    /// Set user agent
    pub async fn set_user_agent(&self, id: &str, user_agent: String) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SetUserAgent {
            user_agent,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("SetUserAgent response channel closed".to_string()),
        }
    }

    /// CDP screenshot with full page and iframe support
    pub async fn screenshot_cdp(
        &self,
        id: &str,
        full_page: bool,
        format: &str,
        quality: Option<u32>,
        frame: Option<String>,
    ) -> Result<Vec<u8>, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ScreenshotCdp {
            full_page,
            format: format.to_string(),
            quality,
            frame,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("ScreenshotCdp response channel closed".to_string()),
        }
    }

    /// Reset device emulation
    pub async fn reset_device_emulation(&self, id: &str) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ResetDeviceEmulation { resp_tx: tx })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("ResetDeviceEmulation response channel closed".to_string()),
        }
    }

    /// Set viewport via CDP
    pub async fn set_viewport_cdp(
        &self,
        id: &str,
        width: u32,
        height: u32,
        device_scale_factor: f64,
        is_mobile: bool,
    ) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::SetViewportCdp {
            width,
            height,
            device_scale_factor,
            is_mobile,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("SetViewportCdp response channel closed".to_string()),
        }
    }

    /// Click element using CDP Input.dispatchMouseEvent (bot detection evasion)
    pub async fn click_cdp(
        &self,
        id: &str,
        selector: String,
        human_mode: bool,
    ) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ClickCdp {
            selector,
            human_mode,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("ClickCdp response channel closed".to_string()),
        }
    }

    /// Type text using CDP Input.dispatchKeyEvent (bot detection evasion)
    pub async fn type_cdp(
        &self,
        id: &str,
        text: String,
        char_delay_ms: u64,
        human_mode: bool,
    ) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::TypeCdp {
            text,
            char_delay_ms,
            human_mode,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("TypeCdp response channel closed".to_string()),
        }
    }

    /// Press special key using CDP Input.dispatchKeyEvent (bot detection evasion)
    pub async fn press_key_cdp(&self, id: &str, key: String) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::PressKeyCdp { key, resp_tx: tx })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("PressKeyCdp response channel closed".to_string()),
        }
    }

    /// Manage network monitoring for a session
    pub async fn manage_network(&self, id: &str, action: NetworkAction) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ManageNetwork {
            action,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("ManageNetwork response channel closed".to_string()),
        }
    }

    /// Get all frames (iframes) in a session via CDP
    pub async fn get_frames(&self, id: &str) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::GetFrames { resp_tx: tx })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("GetFrames response channel closed".to_string()),
        }
    }

    /// Execute script in a specific frame (iframe) via CDP
    pub async fn execute_in_frame(
        &self,
        id: &str,
        script: &str,
        frame: &str,
    ) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::ExecuteInFrame {
            script: script.to_string(),
            frame: frame.to_string(),
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("ExecuteInFrame response channel closed".to_string()),
        }
    }

    /// Inject file(s) into a WebView file input via CDP DOM.setFileInputFiles
    pub async fn form_inject_file(
        &self,
        id: &str,
        selector: &str,
        file_paths: &[String],
        frame: Option<String>,
    ) -> Result<String, String> {
        let handle = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        };

        let (tx, rx) = oneshot::channel();
        handle.send_command(SessionCommand::FormInjectFile {
            selector: selector.to_string(),
            file_paths: file_paths.to_vec(),
            frame,
            resp_tx: tx,
        })?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err("FormInjectFile response channel closed".to_string()),
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
            tracing::info!(
                "[SessionManager] Cleaned up {} idle sessions",
                removed.len()
            );
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
            .map(|(id, handle)| (id.clone(), handle.idle_duration(), handle.get_status_sync()))
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
            window_title: None,
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
            window_title: None,
        };

        assert!(manager.create_session(options.clone()).is_ok());
        assert!(manager.create_session(options).is_err());
    }
}
