use crate::webview::window::WebViewWindow;
use std::cell::RefCell;
use std::collections::HashMap;
use uuid::Uuid;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{Error, Result as WinResult, HRESULT, HSTRING};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_USER};


// Custom Messages
pub const WM_WEBVIEW_CREATED: u32 = WM_USER + 100;
pub const WM_SCRIPT_RESULT: u32 = WM_USER + 101;
pub const WM_CAPTURE_RESULT: u32 = WM_USER + 102;

// ============================================================================
// Debug Logging Helpers
// ============================================================================

fn log_webview_start(operation: &str, details: &str) {
    tracing::info!(">>> WV_START: {} | {}", operation, details);
}

fn log_webview_success(operation: &str, duration_ms: Option<u128>) {
    let duration = duration_ms
        .map(|d| format!(" | {}ms", d))
        .unwrap_or_default();
    tracing::info!("<<< WV_SUCCESS: {}{}", operation, duration);
}

fn log_webview_error(operation: &str, error: &str) {
    tracing::error!("<<< WV_ERROR: {} | {}", operation, error);
}

fn log_webview_debug(operation: &str, details: &str) {
    tracing::debug!("    WV_DEBUG: {} | {}", operation, details);
}

// Thread-local storage to temporarily hold controllers until the main loop claims them
thread_local! {
    static PENDING_CONTROLLERS: RefCell<HashMap<usize, ICoreWebView2Controller>> = RefCell::new(HashMap::new());
    pub static PENDING_SCRIPT_RESULTS: RefCell<HashMap<String, Result<String, String>>> = RefCell::new(HashMap::new());

    // CDP screenshot results (Page.captureScreenshot returns base64 data)
    pub static PENDING_CDP_SCREENSHOTS: RefCell<HashMap<String, Result<String, String>>> = RefCell::new(HashMap::new());
    // CDP cookie results (Network.getCookies returns JSON)
    pub static PENDING_CDP_COOKIES: RefCell<HashMap<String, Result<String, String>>> = RefCell::new(HashMap::new());
    // Generic CDP method results (for frame operations, Runtime.evaluate, etc.)
    pub static PENDING_CDP_RESULTS: RefCell<HashMap<String, Result<String, String>>> = RefCell::new(HashMap::new());
    static NAVIGATION_COMPLETION: RefCell<HashMap<usize, bool>> = RefCell::new(HashMap::new());
    static PENDING_SCRIPT_COUNT: RefCell<std::sync::atomic::AtomicUsize> = RefCell::new(std::sync::atomic::AtomicUsize::new(0));

    // Network Monitoring
    pub static NETWORK_LOGS_MAP: RefCell<HashMap<String, crate::core::NetworkLogEntry>> = RefCell::new(HashMap::new());
    pub static NETWORK_MONITORING_ENABLED: RefCell<bool> = RefCell::new(false);
    pub static MAX_NETWORK_LOGS: RefCell<usize> = RefCell::new(100);
    pub static NETWORK_EVENT_TOKENS: RefCell<HashMap<String, windows::Win32::System::WinRT::EventRegistrationToken>> = RefCell::new(HashMap::new());
}

/// Anti-bot detection script that runs on every page load
/// Masks WebView2/automation fingerprints to appear as a normal browser
const ANTI_BOT_SCRIPT: &str = r#"
(function() {
    // Remove navigator.webdriver flag (main automation detection)
    Object.defineProperty(navigator, 'webdriver', {
        get: () => undefined,
        configurable: true
    });
    
    // Remove WebDriver indicator from navigator prototype
    delete Object.getPrototypeOf(navigator).webdriver;
    
    // Override navigator.plugins to appear as normal browser
    Object.defineProperty(navigator, 'plugins', {
        get: () => {
            const plugins = [
                { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer' },
                { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai' },
                { name: 'Native Client', filename: 'internal-nacl-plugin' }
            ];
            plugins.item = (i) => plugins[i];
            plugins.namedItem = (name) => plugins.find(p => p.name === name);
            plugins.refresh = () => {};
            return plugins;
        },
        configurable: true
    });
    
    // Override navigator.languages
    Object.defineProperty(navigator, 'languages', {
        get: () => ['ja-JP', 'ja', 'en-US', 'en'],
        configurable: true
    });
    
    // Mask WebGL vendor/renderer if needed
    const getParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = function(param) {
        if (param === 37445) return 'Intel Inc.';  // UNMASKED_VENDOR_WEBGL
        if (param === 37446) return 'Intel Iris OpenGL Engine';  // UNMASKED_RENDERER_WEBGL
        return getParameter.call(this, param);
    };
    
    // Remove automation console message
    const originalConsoleDebug = console.debug;
    console.debug = function(...args) {
        if (args[0]?.includes?.('webdriver')) return;
        return originalConsoleDebug.apply(this, args);
    };
    
    // Mask permission query override (used by some detection)
    const originalQuery = navigator.permissions?.query;
    if (originalQuery) {
        navigator.permissions.query = (parameters) => {
            if (parameters.name === 'notifications') {
                return Promise.resolve({ state: 'denied', onchange: null });
            }
            return originalQuery.call(navigator.permissions, parameters);
        };
    }
})();
"#;

pub struct WebViewInstance {
    window: WebViewWindow,
    controller: Option<ICoreWebView2Controller>,
}

// ----------------------------------------------------------------
// CDP (Chrome DevTools Protocol) Completed Handler
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct CdpCompletedHandler;

impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for CdpCompletedHandler {
    fn Invoke(&self, error_code: HRESULT, return_object_as_json: &windows::core::PCWSTR) -> WinResult<()> {
        if error_code.is_ok() {
            let result = unsafe { return_object_as_json.to_string().unwrap_or_default() };
            tracing::debug!("[CDP] Method completed successfully: {}", result);
        } else {
            tracing::warn!("[CDP] Method failed: {:?}", error_code);
        }
        Ok(())
    }
}

// ----------------------------------------------------------------
// CDP Screenshot Handler (for Page.captureScreenshot)
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct CdpScreenshotHandler {
    request_id: String,
}

impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for CdpScreenshotHandler {
    fn Invoke(&self, error_code: HRESULT, return_object_as_json: &windows::core::PCWSTR) -> WinResult<()> {
        if error_code.is_ok() {
            let result = unsafe { return_object_as_json.to_string().unwrap_or_default() };
            tracing::debug!("[CDP Screenshot] Completed, request_id={}", self.request_id);
            
            // Parse the JSON to extract base64 data
            // CDP returns: {"data": "base64_encoded_image_data"}
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(data) = json.get("data").and_then(|v| v.as_str()) {
                    PENDING_CDP_SCREENSHOTS.with(|map| {
                        map.borrow_mut().insert(self.request_id.clone(), Ok(data.to_string()));
                    });
                } else {
                    PENDING_CDP_SCREENSHOTS.with(|map| {
                        map.borrow_mut().insert(self.request_id.clone(), Err("No data in CDP response".to_string()));
                    });
                }
            } else {
                PENDING_CDP_SCREENSHOTS.with(|map| {
                    map.borrow_mut().insert(self.request_id.clone(), Err(format!("Failed to parse CDP response: {}", result)));
                });
            }
        } else {
            tracing::warn!("[CDP Screenshot] Failed: {:?}", error_code);
            PENDING_CDP_SCREENSHOTS.with(|map| {
                map.borrow_mut().insert(self.request_id.clone(), Err(format!("CDP error: {:?}", error_code)));
            });
        }
        Ok(())
    }
}

// ----------------------------------------------------------------
// CDP Cookie Handler (for Network.getCookies / Network.setCookie)
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct CdpCookieHandler {
    request_id: String,
}

impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for CdpCookieHandler {
    fn Invoke(&self, error_code: HRESULT, return_object_as_json: &windows::core::PCWSTR) -> WinResult<()> {
        if error_code.is_ok() {
            let result = unsafe { return_object_as_json.to_string().unwrap_or_default() };
            tracing::debug!("[CDP Cookie] Completed, request_id={}", self.request_id);
            
            PENDING_CDP_COOKIES.with(|map| {
                map.borrow_mut().insert(self.request_id.clone(), Ok(result));
            });
        } else {
            tracing::warn!("[CDP Cookie] Failed: {:?}", error_code);
            PENDING_CDP_COOKIES.with(|map| {
                map.borrow_mut().insert(self.request_id.clone(), Err(format!("CDP error: {:?}", error_code)));
            });
        }
        Ok(())
    }
}

// ----------------------------------------------------------------
// Generic CDP Result Handler (for frames, Runtime.evaluate, etc.)
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct CdpResultHandler {
    request_id: String,
}

impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for CdpResultHandler {
    fn Invoke(&self, error_code: HRESULT, return_object_as_json: &windows::core::PCWSTR) -> WinResult<()> {
        if error_code.is_ok() {
            let result = unsafe { return_object_as_json.to_string().unwrap_or_default() };
            tracing::debug!("[CDP Result] Completed, request_id={}", self.request_id);
            PENDING_CDP_RESULTS.with(|map| {
                map.borrow_mut().insert(self.request_id.clone(), Ok(result));
            });
        } else {
            tracing::warn!("[CDP Result] Failed: {:?}, request_id={}", error_code, self.request_id);
            PENDING_CDP_RESULTS.with(|map| {
                map.borrow_mut().insert(self.request_id.clone(), Err(format!("CDP error: {:?}", error_code)));
            });
        }
        Ok(())
    }
}

// ----------------------------------------------------------------
// Network Event Handlers
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2DevToolsProtocolEventReceivedEventHandler)]
struct NetworkRequestReceivedHandler;

impl ICoreWebView2DevToolsProtocolEventReceivedEventHandler_Impl for NetworkRequestReceivedHandler {
    fn Invoke(
        &self,
        _sender: &Option<ICoreWebView2>,
        args: &Option<ICoreWebView2DevToolsProtocolEventReceivedEventArgs>,
    ) -> WinResult<()> {
        if let Some(args) = args {
            let json_str = unsafe {
                let mut pwstr = windows::core::PWSTR::null();
                args.ParameterObjectAsJson(&mut pwstr)?;
                let s = pwstr.to_string().unwrap_or_default();
                if !pwstr.is_null() {
                    windows::Win32::System::Com::CoTaskMemFree(pwstr.0 as *const std::ffi::c_void);
                }
                s
            };
            
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let enabled = NETWORK_MONITORING_ENABLED.with(|e| *e.borrow());
                if !enabled { return Ok(()); }

                let request_id = json["requestId"].as_str().unwrap_or_default().to_string();
                let url = json["request"]["url"].as_str().unwrap_or_default().to_string();
                let method = json["request"]["method"].as_str().unwrap_or_default().to_string();
                let timestamp = json["wallTime"].as_f64().unwrap_or(0.0);
                
                let mut headers = HashMap::new();
                if let Some(h) = json["request"]["headers"].as_object() {
                    for (k, v) in h {
                        headers.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                    }
                }
                
                let post_data = json["request"]["postData"].as_str().map(|s| s.to_string());
                
                let entry = crate::core::NetworkLogEntry {
                    request_id: request_id.clone(),
                    request: crate::core::NetworkRequest {
                        url,
                        method,
                        headers,
                        timestamp,
                        post_data,
                    },
                    response: None,
                };
                
                NETWORK_LOGS_MAP.with(|map| {
                    let mut m = map.borrow_mut();
                    let max = MAX_NETWORK_LOGS.with(|m| *m.borrow());
                    if m.len() >= max {
                        let key = m.keys().next().cloned();
                        if let Some(k) = key { m.remove(&k); }
                    }
                    m.insert(request_id, entry);
                });
            }
        }
        Ok(())
    }
}

#[windows::core::implement(ICoreWebView2DevToolsProtocolEventReceivedEventHandler)]
struct NetworkResponseReceivedHandler;

impl ICoreWebView2DevToolsProtocolEventReceivedEventHandler_Impl for NetworkResponseReceivedHandler {
    fn Invoke(
        &self,
        _sender: &Option<ICoreWebView2>,
        args: &Option<ICoreWebView2DevToolsProtocolEventReceivedEventArgs>,
    ) -> WinResult<()> {
        if let Some(args) = args {
            let json_str = unsafe {
                let mut pwstr = windows::core::PWSTR::null();
                args.ParameterObjectAsJson(&mut pwstr)?;
                let s = pwstr.to_string().unwrap_or_default();
                if !pwstr.is_null() {
                    windows::Win32::System::Com::CoTaskMemFree(pwstr.0 as *const std::ffi::c_void);
                }
                s
            };
             if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let request_id = json["requestId"].as_str().unwrap_or_default();
                
                 NETWORK_LOGS_MAP.with(|map| {
                    let mut m = map.borrow_mut();
                    if let Some(entry) = m.get_mut(request_id) {
                        let url = json["response"]["url"].as_str().unwrap_or_default().to_string();
                        let status = json["response"]["status"].as_i64().unwrap_or(0) as i32;
                        let mime_type = json["response"]["mimeType"].as_str().unwrap_or_default().to_string();
                        let timestamp = json["timestamp"].as_f64().unwrap_or(0.0);
                        
                        let mut headers = HashMap::new();
                        if let Some(h) = json["response"]["headers"].as_object() {
                            for (k, v) in h {
                                headers.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                            }
                        }

                        entry.response = Some(crate::core::NetworkResponse {
                            url,
                            status,
                            headers,
                            mime_type,
                            timestamp,
                        });
                    }
                });
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------
// Environment Handler
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler)]
struct EnvHandler {
    hwnd: HWND,
}

impl ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_Impl for EnvHandler {
    fn Invoke(&self, result: HRESULT, env: &Option<ICoreWebView2Environment>) -> WinResult<()> {
        log_webview_start("EnvHandler", &format!("hwnd={:#x}", self.hwnd.0));

        if let Err(e) = result.ok() {
            log_webview_error("EnvHandler", &format!("HRESULT error: {:?}", e));
            return Err(e);
        }

        let env = env.as_ref().ok_or_else(|| Error::from_win32())?;
        log_webview_debug("EnvHandler", "Environment created successfully");

        unsafe {
            log_webview_debug("EnvHandler", "Creating CoreWebView2Controller...");
            env.CreateCoreWebView2Controller(
                self.hwnd,
                &ICoreWebView2CreateCoreWebView2ControllerCompletedHandler::from(
                    ControllerHandler { hwnd: self.hwnd },
                ),
            )?;
        }

        log_webview_success("EnvHandler", None);
        Ok(())
    }
}

// ----------------------------------------------------------------
// Controller Handler
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2CreateCoreWebView2ControllerCompletedHandler)]
struct ControllerHandler {
    hwnd: HWND,
}

impl ICoreWebView2CreateCoreWebView2ControllerCompletedHandler_Impl for ControllerHandler {
    fn Invoke(
        &self,
        result: HRESULT,
        controller: &Option<ICoreWebView2Controller>,
    ) -> WinResult<()> {
        let start = std::time::Instant::now();
        log_webview_start("ControllerHandler", &format!("hwnd={:#x}", self.hwnd.0));

        if let Err(e) = result.ok() {
            log_webview_error("ControllerHandler", &format!("HRESULT error: {:?}", e));
            return Err(e);
        }

        let controller = controller.as_ref().ok_or_else(|| Error::from_win32())?;
        log_webview_debug("ControllerHandler", "Controller created successfully");

        // Setup initial bounds
        unsafe {
            log_webview_debug("ControllerHandler", "Setting bounds...");
            let mut rect = windows::Win32::Foundation::RECT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(self.hwnd, &mut rect);

            if rect.right == 0 && rect.bottom == 0 {
                log_webview_debug(
                    "ControllerHandler",
                    "Client rect is empty, using default 1024x768",
                );
                rect.right = 1024;
                rect.bottom = 768;
            }

            log_webview_debug(
                "ControllerHandler",
                &format!("Bounds: {}x{}", rect.right, rect.bottom),
            );
            match controller.SetBounds(rect) {
                Ok(_) => log_webview_debug("ControllerHandler", "SetBounds successful"),
                Err(e) => {
                    log_webview_error("ControllerHandler", &format!("SetBounds failed: {:?}", e));
                    return Err(e);
                }
            }

            log_webview_debug("ControllerHandler", "Setting visible...");
            match controller.SetIsVisible(true) {
                Ok(_) => log_webview_debug("ControllerHandler", "SetIsVisible successful"),
                Err(e) => {
                    log_webview_error(
                        "ControllerHandler",
                        &format!("SetIsVisible failed: {:?}", e),
                    );
                    return Err(e);
                }
            }

            log_webview_debug("ControllerHandler", "Getting CoreWebView2...");
            let webview = match controller.CoreWebView2() {
                Ok(wv) => {
                    log_webview_debug("ControllerHandler", "CoreWebView2 retrieved successfully");
                    Some(wv)
                }
                Err(e) => {
                    log_webview_error(
                        "ControllerHandler",
                        &format!("CoreWebView2 failed: {:?}", e),
                    );
                    return Err(e);
                }
            };

            let webview = webview.as_ref().ok_or_else(|| Error::from_win32())?;

            // Register events
            log_webview_debug(
                "ControllerHandler",
                "Registering NavigationStarting event...",
            );
            let _ = webview.add_NavigationStarting(
                &ICoreWebView2NavigationStartingEventHandler::from(NavigationStartingHandler {}),
                &mut Default::default(),
            );

            log_webview_debug(
                "ControllerHandler",
                "Registering NavigationCompleted event...",
            );
            let _ = webview.add_NavigationCompleted(
                &ICoreWebView2NavigationCompletedEventHandler::from(NavigationCompletedHandler {
                    hwnd: self.hwnd,
                }),
                &mut Default::default(),
            );

            log_webview_debug("ControllerHandler", "Events registered successfully");
        }

        // Store controller in thread local storage
        let hwnd_val = self.hwnd.0 as usize;
        log_webview_debug(
            "ControllerHandler",
            &format!("Storing controller for hwnd_val={}", hwnd_val),
        );
        PENDING_CONTROLLERS.with(|map| {
            map.borrow_mut().insert(hwnd_val, controller.clone());
        });
        
        // Register controller in global registry for WM_SIZE handling
        crate::webview::window::register_controller(self.hwnd, controller.clone());

        // Notify main loop
        log_webview_debug("ControllerHandler", "Posting WM_WEBVIEW_CREATED message");
        unsafe {
            PostMessageW(self.hwnd, WM_WEBVIEW_CREATED, WPARAM(0), LPARAM(0));
        }

        log_webview_success("ControllerHandler", Some(start.elapsed().as_millis()));
        Ok(())
    }
}

// ----------------------------------------------------------------
// Event Handlers
// ----------------------------------------------------------------
#[windows::core::implement(ICoreWebView2NavigationStartingEventHandler)]
struct NavigationStartingHandler {}

impl ICoreWebView2NavigationStartingEventHandler_Impl for NavigationStartingHandler {
    fn Invoke(
        &self,
        _webview: &Option<ICoreWebView2>,
        args: &Option<ICoreWebView2NavigationStartingEventArgs>,
    ) -> WinResult<()> {
        if let Some(args) = args {
            let mut uri = windows::core::PWSTR::null();
            unsafe { args.Uri(&mut uri)? };
            let uri_str = unsafe { uri.to_string().unwrap_or_default() };
            tracing::info!("Navigation Starting: {}", uri_str);
        }
        Ok(())
    }
}

#[windows::core::implement(ICoreWebView2NavigationCompletedEventHandler)]
struct NavigationCompletedHandler {
    hwnd: HWND,
}

impl ICoreWebView2NavigationCompletedEventHandler_Impl for NavigationCompletedHandler {
    fn Invoke(
        &self,
        _webview: &Option<ICoreWebView2>,
        args: &Option<ICoreWebView2NavigationCompletedEventArgs>,
    ) -> WinResult<()> {
        if let Some(args) = args {
            let mut success = BOOL::default();
            unsafe { args.IsSuccess(&mut success)? };
            if success.as_bool() {
                tracing::info!("Navigation Completed Successfully");
                // Mark navigation as completed
                let hwnd_val = self.hwnd.0 as usize;
                NAVIGATION_COMPLETION.with(|map| {
                    map.borrow_mut().insert(hwnd_val, true);
                });
            } else {
                tracing::warn!("Navigation Failed");
                let hwnd_val = self.hwnd.0 as usize;
                NAVIGATION_COMPLETION.with(|map| {
                    map.borrow_mut().insert(hwnd_val, false);
                });
            }
        }
        Ok(())
    }
}

#[windows::core::implement(ICoreWebView2ExecuteScriptCompletedHandler)]
struct ExecuteScriptHandler {
    request_id: String,
    hwnd: HWND,
}

impl ICoreWebView2ExecuteScriptCompletedHandler_Impl for ExecuteScriptHandler {
    fn Invoke(
        &self,
        _error_code: windows::core::HRESULT,
        result_as_json: &windows::core::PCWSTR,
    ) -> WinResult<()> {
        let result_str = unsafe { result_as_json.to_string().unwrap_or_default() };

        // Debug log the raw result
        tracing::debug!("ExecuteScriptHandler: raw result_str (len={}): {}", result_str.len(), 
            if result_str.len() > 200 { &result_str[..200] } else { &result_str });

        // Parse JSON result (WebView2 returns JSON string for any result)
        let result = match serde_json::from_str::<serde_json::Value>(&result_str) {
            Ok(json_val) => {
                if let Some(s) = json_val.as_str() {
                    Ok(s.to_string())
                } else {
                    Ok(json_val.to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse result: {}", e)),
        };

        // Store result
        let request_id = self.request_id.clone();
        PENDING_SCRIPT_RESULTS.with(|map| {
            map.borrow_mut().insert(request_id, result);
        });

        // Notify main thread
        unsafe {
            PostMessageW(self.hwnd, WM_SCRIPT_RESULT, WPARAM(0), LPARAM(0));
        }

        Ok(())
    }
}

#[windows::core::implement(ICoreWebView2ExecuteScriptCompletedHandler)]
struct FireAndForgetHandler;

impl ICoreWebView2ExecuteScriptCompletedHandler_Impl for FireAndForgetHandler {
    fn Invoke(
        &self,
        _error_code: windows::core::HRESULT,
        _result_as_json: &windows::core::PCWSTR,
    ) -> WinResult<()> {
        // Fire and forget - we don't care about the result
        tracing::trace!("FireAndForgetHandler: Script completed (result ignored)");
        Ok(())
    }
}



// ----------------------------------------------------------------
// Instance Implementation
// ----------------------------------------------------------------
impl WebViewInstance {
    pub fn new(title: &str, visible: bool) -> WinResult<Self> {
        log_webview_start(
            "WebViewInstance::new",
            &format!("title={}, visible={}", title, visible),
        );

        // Note: CoInitializeEx should be called by the thread that owns this instance.
        // We do it here to ensure it's initialized for the session thread.
        unsafe {
            log_webview_debug(
                "WebViewInstance::new",
                "Initializing COM (Apartment Threaded)",
            );
            let _ = windows::Win32::System::Com::CoInitializeEx(
                std::ptr::null_mut(),
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }

        let window = WebViewWindow::create(title, visible)?;
        let hwnd = window.get_hwnd();
        log_webview_debug(
            "WebViewInstance::new",
            &format!("Window created with hwnd={:#x}", hwnd.0),
        );

        log_webview_success("WebViewInstance::new", None);
        Ok(Self {
            window,
            controller: None,
        })
    }

    pub fn get_hwnd(&self) -> HWND {
        self.window.get_hwnd()
    }

    /// Show the window (make visible)
    pub fn show(&self) {
        self.window.show();
    }
    
    /// Hide the window (pseudo-headless)
    pub fn hide(&self) {
        self.window.hide();
    }
    
    /// Set window visibility
    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }
    
    /// Check if window is visible
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
    
    /// Bring window to front and focus
    pub fn bring_to_front(&self) {
        self.window.bring_to_front();
    }

    pub fn initialize(&mut self, user_data_folder: &str) -> WinResult<()> {
        log_webview_start(
            "WebViewInstance::initialize",
            &format!("user_data_folder={}", user_data_folder),
        );

        let hwnd = self.window.get_hwnd();
        unsafe {
            let user_data_folder_h = HSTRING::from(user_data_folder);

            // Set environment variables if not already set (or append)
            // Note: This applies globally to the process usually.
            log_webview_debug(
                "WebViewInstance::initialize",
                "Setting WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            );
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--disable-gpu --no-sandbox",
            );

            log_webview_debug(
                "WebViewInstance::initialize",
                "Calling CreateCoreWebView2EnvironmentWithOptions",
            );
            webview2_com::Microsoft::Web::WebView2::Win32::CreateCoreWebView2EnvironmentWithOptions(
                None,
                &user_data_folder_h,
                None,
                &ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler::from(EnvHandler { hwnd }),
            )?;
        }

        log_webview_success("WebViewInstance::initialize", None);
        Ok(())
    }

    // Called by main loop when WM_WEBVIEW_CREATED is received
    pub fn claim_controller(&mut self) {
        let hwnd_val = self.get_hwnd().0 as usize;
        log_webview_start(
            "WebViewInstance::claim_controller",
            &format!("hwnd_val={}", hwnd_val),
        );

        let controller = PENDING_CONTROLLERS.with(|map| map.borrow_mut().remove(&hwnd_val));

        if let Some(c) = controller {
            log_webview_success("WebViewInstance::claim_controller", None);
            
            // Inject anti-bot script on document creation
            unsafe {
                if let Ok(webview) = c.CoreWebView2() {
                    let anti_bot_script = HSTRING::from(ANTI_BOT_SCRIPT);
                    let _ = webview.AddScriptToExecuteOnDocumentCreated(
                        &anti_bot_script,
                        None,
                    );
                    log_webview_debug("WebViewInstance::claim_controller", "Anti-bot script injected");
                }
            }
            
            self.controller = Some(c);
        } else {
            log_webview_error(
                "WebViewInstance::claim_controller",
                "No pending controller found",
            );
        }
    }

    pub fn navigate(&self, url: &str) -> WinResult<()> {
        log_webview_start("WebViewInstance::navigate", &format!("url={}", url));

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                
                // Ensure anti-bot script is registered (for restored sessions)
                let anti_bot_script = HSTRING::from(ANTI_BOT_SCRIPT);
                let _ = webview.AddScriptToExecuteOnDocumentCreated(
                    &anti_bot_script,
                    None,
                );
                
                log_webview_debug("WebViewInstance::navigate", "Calling webview.Navigate()");
                webview.Navigate(&HSTRING::from(url))?;
            }
            log_webview_success("WebViewInstance::navigate", None);
            Ok(())
        } else {
            log_webview_error("WebViewInstance::navigate", "Controller is not ready");
            Err(Error::from_win32())
        }
    }

    pub fn execute_script(&self, script: &str, request_id: String) -> Result<String, Error> {
        let script_preview = if script.len() > 50 {
            format!("{}...", &script[..50])
        } else {
            script.to_string()
        };

        log_webview_start(
            "WebViewInstance::execute_script",
            &format!("request_id={}, script={}", request_id, script_preview),
        );

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                log_webview_debug(
                    "WebViewInstance::execute_script",
                    "Calling webview.ExecuteScript()",
                );
                webview.ExecuteScript(
                    &HSTRING::from(script),
                    &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                        request_id: request_id.clone(),
                        hwnd: self.get_hwnd(),
                    }),
                )?;

                // Wait for result while pumping messages
                match self.wait_for_script_result(&request_id) {
                    Ok(res) => {
                        log_webview_success("WebViewInstance::execute_script", None);
                        Ok(res)
                    }
                    Err(e) => {
                        log_webview_error("WebViewInstance::execute_script", &e);
                        Err(Error::new(
                            windows::core::HRESULT(0x80004005u32 as i32),
                            HSTRING::from(e),
                        ))
                    }
                }
            }
        } else {
            log_webview_error("WebViewInstance::execute_script", "Controller is not ready");
            Err(Error::from_win32())
        }
    }

    fn wait_for_script_result(&self, request_id: &str) -> Result<String, String> {
        let start = std::time::Instant::now();
        loop {
            // Pump messages to ensure callbacks are called
            unsafe {
                let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    HWND::default(),
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                )
                .as_bool()
                {
                    windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                    windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                }
            }

            if let Some(result) =
                PENDING_SCRIPT_RESULTS.with(|map| map.borrow_mut().remove(request_id))
            {
                return result;
            }

            if start.elapsed() > std::time::Duration::from_secs(30) {
                return Err(format!("Script timeout for request_id: {}", request_id));
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Execute script synchronously and return result
    fn execute_script_sync(&self, script: &str) -> Result<String, String> {
        let request_id = Uuid::new_v4().to_string();
        self.execute_script(script, request_id.clone())
            .map_err(|e| format!("Script execution failed: {:?}", e))
    }

    pub fn resize(&self) {
        log_webview_debug("WebViewInstance::resize", "Resizing WebView");
        if let Some(controller) = &self.controller {
            unsafe {
                let mut rect = windows::Win32::Foundation::RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(
                    self.get_hwnd(),
                    &mut rect,
                );
                let _ = controller.SetBounds(rect);
            }
        }
    }
    
    /// Set the WebView viewport to specific dimensions (for device simulation)
    /// This resizes both the window and the WebView content area
    pub fn set_viewport(&self, width: u32, height: u32) -> WinResult<()> {
        log_webview_start("WebViewInstance::set_viewport", &format!("{}x{}", width, height));
        
        if let Some(controller) = &self.controller {
            unsafe {
                // Calculate window size including non-client area (borders, title bar)
                let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
                    self.get_hwnd(),
                    windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
                ) as u32;
                let ex_style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
                    self.get_hwnd(),
                    windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE,
                ) as u32;
                
                let mut rect = windows::Win32::Foundation::RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                
                let _ = windows::Win32::UI::WindowsAndMessaging::AdjustWindowRectEx(
                    &mut rect,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(style),
                    false,
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(ex_style),
                );
                
                let window_width = rect.right - rect.left;
                let window_height = rect.bottom - rect.top;
                
                // Resize window
                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                    self.get_hwnd(),
                    windows::Win32::UI::WindowsAndMessaging::HWND_TOP,
                    0, 0,
                    window_width, window_height,
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | 
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
                );
                
                // Set WebView bounds to exact viewport size
                let client_rect = windows::Win32::Foundation::RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                controller.SetBounds(client_rect)?;
                
                log_webview_success("WebViewInstance::set_viewport", None);
            }
        } else {
            log_webview_error("WebViewInstance::set_viewport", "Controller not ready");
            return Err(Error::from_win32());
        }
        
        Ok(())
    }
    
    /// Set the User-Agent string via CDP (Emulation.setUserAgentOverride)
    /// This sets UA at the network level, not just JavaScript - important for bot detection evasion
    pub fn set_user_agent(&self, user_agent: &str) -> WinResult<()> {
        log_webview_start("WebViewInstance::set_user_agent", user_agent);
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                
                // Use CDP Emulation.setUserAgentOverride for network-level UA spoofing
                let ua_params = format!(
                    r#"{{"userAgent": "{}"}}"#,
                    user_agent.replace('"', "\\\"")
                );
                
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Emulation.setUserAgentOverride"),
                    &HSTRING::from(&ua_params),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                )?;
                
                log_webview_success("WebViewInstance::set_user_agent", None);
                tracing::info!("[set_user_agent] CDP UA set: {}", user_agent);
            }
        } else {
            log_webview_error("WebViewInstance::set_user_agent", "Controller not ready");
            return Err(Error::from_win32());
        }
        
        Ok(())
    }
    
    /// Apply device simulation (viewport + user agent + touch events)
    pub fn simulate_device(&self, device_name: &str) -> WinResult<String> {
        log_webview_start("WebViewInstance::simulate_device", device_name);
        
        // Get device preset with flexible name matching
        let presets = crate::core::screenshot_v2::get_device_presets();
        
        // Normalize the input name: lowercase, replace spaces with underscores
        let normalized = device_name.to_lowercase().replace(' ', "_");
        
        // Try exact match first, then partial match
        let device = presets.iter()
            .find(|p| p.name.to_lowercase() == normalized)
            .or_else(|| presets.iter().find(|p| p.name.to_lowercase().contains(&normalized)))
            .or_else(|| presets.iter().find(|p| normalized.contains(&p.name.to_lowercase())));
        
        if let Some(preset) = device {
            if let Some(controller) = &self.controller {
                unsafe {
                    let webview = controller.CoreWebView2()?;
                    
                    // 1. Set device metrics via CDP: Emulation.setDeviceMetricsOverride
                    let device_metrics = format!(
                        r#"{{
                            "width": {},
                            "height": {},
                            "deviceScaleFactor": {},
                            "mobile": {},
                            "screenOrientation": {{"angle": 0, "type": "portraitPrimary"}}
                        }}"#,
                        preset.viewport.width,
                        preset.viewport.height,
                        preset.viewport.device_scale_factor,
                        preset.viewport.is_mobile
                    );
                    
                    let _ = webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Emulation.setDeviceMetricsOverride"),
                        &HSTRING::from(&device_metrics),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    );
                    
                    // 2. Enable touch emulation if device has touch
                    if preset.viewport.has_touch {
                        let touch_params = r#"{"enabled": true, "maxTouchPoints": 5}"#;
                        let _ = webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Emulation.setTouchEmulationEnabled"),
                            &HSTRING::from(touch_params),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        );
                    } else {
                        let touch_params = r#"{"enabled": false}"#;
                        let _ = webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Emulation.setTouchEmulationEnabled"),
                            &HSTRING::from(touch_params),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        );
                    }
                    
                    // 3. Set user agent via CDP: Emulation.setUserAgentOverride
                    let ua_params = format!(
                        r#"{{"userAgent": "{}"}}"#,
                        preset.user_agent.replace('"', "\\\"")
                    );
                    let _ = webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Emulation.setUserAgentOverride"),
                        &HSTRING::from(&ua_params),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    );
                    
                    // Also resize the window to match
                    self.set_viewport(preset.viewport.width, preset.viewport.height)?;
                }
                
                log_webview_success("WebViewInstance::simulate_device", None);
                Ok(format!("Device simulation applied via CDP: {} ({}x{}, scale={}, mobile={}, touch={})", 
                    preset.name, 
                    preset.viewport.width, 
                    preset.viewport.height,
                    preset.viewport.device_scale_factor,
                    preset.viewport.is_mobile,
                    preset.viewport.has_touch))
            } else {
                log_webview_error("WebViewInstance::simulate_device", "Controller not ready");
                Err(Error::from_win32())
            }
        } else {
            log_webview_error("WebViewInstance::simulate_device", "Device not found");
            Err(Error::from_win32())
        }
    }

    /// Reset device emulation to default (clear all overrides)
    pub fn reset_device_emulation(&self) -> WinResult<String> {
        log_webview_start("WebViewInstance::reset_device_emulation", "");
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                
                // 1. Clear device metrics override
                let _ = webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Emulation.clearDeviceMetricsOverride"),
                    &HSTRING::from("{}"),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                );
                
                // 2. Disable touch emulation
                let _ = webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Emulation.setTouchEmulationEnabled"),
                    &HSTRING::from(r#"{"enabled": false}"#),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                );
                
                // 3. Clear user agent override
                let _ = webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Emulation.setUserAgentOverride"),
                    &HSTRING::from(r#"{"userAgent": ""}"#),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                );
            }
            
            log_webview_success("WebViewInstance::reset_device_emulation", None);
            Ok("Device emulation reset to defaults".to_string())
        } else {
            log_webview_error("WebViewInstance::reset_device_emulation", "Controller not ready");
            Err(Error::from_win32())
        }
    }

    /// Set viewport dimensions via CDP (with device metrics override)
    pub fn set_viewport_cdp(&self, width: u32, height: u32, device_scale_factor: f64, is_mobile: bool) -> WinResult<String> {
        log_webview_start("WebViewInstance::set_viewport_cdp", &format!("{}x{}, scale={}, mobile={}", width, height, device_scale_factor, is_mobile));
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                
                let device_metrics = format!(
                    r#"{{
                        "width": {},
                        "height": {},
                        "deviceScaleFactor": {},
                        "mobile": {}
                    }}"#,
                    width, height, device_scale_factor, is_mobile
                );
                
                let _ = webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Emulation.setDeviceMetricsOverride"),
                    &HSTRING::from(&device_metrics),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                );
                
                // Also resize the window
                self.set_viewport(width, height)?;
            }
            
            log_webview_success("WebViewInstance::set_viewport_cdp", None);
            Ok(format!("Viewport set via CDP: {}x{}", width, height))
        } else {
            log_webview_error("WebViewInstance::set_viewport_cdp", "Controller not ready");
            Err(Error::from_win32())
        }
    }

    /// Capture screenshot via CDP (Page.captureScreenshot)
    /// Supports full page capture and custom format/quality
    pub fn capture_screenshot_cdp(&self, full_page: bool, format: &str, quality: Option<u32>) -> Result<Vec<u8>, String> {
        log_webview_start("WebViewInstance::capture_screenshot_cdp", &format!("full_page={}, format={}", full_page, format));
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                
                // For full page, first get page dimensions and hide fixed elements
                if full_page {
                    // 1. Get page dimensions
                    let dim_script = r#"
                        JSON.stringify({
                            width: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth),
                            height: Math.max(document.body.scrollHeight, document.documentElement.scrollHeight),
                            viewportWidth: window.innerWidth,
                            viewportHeight: window.innerHeight
                        });
                    "#;
                    
                    // Execute synchronously to get dimensions
                    let dim_result = self.execute_script_sync(dim_script)?;
                    let dims: serde_json::Value = serde_json::from_str(&dim_result)
                        .map_err(|e| format!("Failed to parse dimensions: {}", e))?;
                    
                    let page_width = dims["width"].as_u64().unwrap_or(1280) as u32;
                    let page_height = dims["height"].as_u64().unwrap_or(800) as u32;
                    let original_width = dims["viewportWidth"].as_u64().unwrap_or(1280) as u32;
                    let original_height = dims["viewportHeight"].as_u64().unwrap_or(800) as u32;
                    
                    log_webview_debug("capture_screenshot_cdp", &format!("Page dimensions: {}x{}", page_width, page_height));
                    
                    // 2. Hide fixed/sticky elements
                    let hide_fixed_script = r#"
                        (function() {
                            const fixed = document.querySelectorAll('*');
                            const hidden = [];
                            fixed.forEach(el => {
                                const style = getComputedStyle(el);
                                if (style.position === 'fixed' || style.position === 'sticky') {
                                    hidden.push({ el, display: el.style.display });
                                    el.style.display = 'none';
                                }
                            });
                            window.__wbFixedElements = hidden;
                            return hidden.length;
                        })();
                    "#;
                    let _ = self.execute_script_sync(hide_fixed_script);
                    
                    // 3. Scroll to top
                    let _ = self.execute_script_sync("window.scrollTo(0, 0);");
                    
                    // 4. Expand viewport via CDP
                    let device_metrics = format!(
                        r#"{{
                            "width": {},
                            "height": {},
                            "deviceScaleFactor": 1,
                            "mobile": false
                        }}"#,
                        page_width, page_height
                    );
                    
                    let _ = webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Emulation.setDeviceMetricsOverride"),
                        &HSTRING::from(&device_metrics),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    );
                    
                    // Wait for viewport to apply
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    // 5. Capture screenshot
                    let request_id = Uuid::new_v4().to_string();
                    let params = format!(
                        r#"{{
                            "format": "{}",
                            {}
                            "captureBeyondViewport": true,
                            "fromSurface": true
                        }}"#,
                        format,
                        if let Some(q) = quality { format!(r#""quality": {},"#, q) } else { String::new() }
                    );
                    
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Page.captureScreenshot"),
                        &HSTRING::from(&params),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpScreenshotHandler {
                            request_id: request_id.clone(),
                        }),
                    ).map_err(|e| format!("CDP call failed: {:?}", e))?;
                    
                    // Wait for screenshot result
                    let start = std::time::Instant::now();
                    let screenshot_result = loop {
                        // Pump messages
                        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                        while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                            &mut msg,
                            HWND::default(),
                            0,
                            0,
                            windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                        ).as_bool() {
                            windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                        }
                        
                        if let Some(result) = PENDING_CDP_SCREENSHOTS.with(|map| map.borrow_mut().remove(&request_id)) {
                            break result;
                        }
                        
                        if start.elapsed() > std::time::Duration::from_secs(60) {
                            break Err("CDP screenshot timeout".to_string());
                        }
                        
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    };
                    
                    // 6. Restore fixed elements
                    let restore_script = r#"
                        (function() {
                            if (window.__wbFixedElements) {
                                window.__wbFixedElements.forEach(({el, display}) => {
                                    el.style.display = display;
                                });
                                delete window.__wbFixedElements;
                            }
                        })();
                    "#;
                    let _ = self.execute_script_sync(restore_script);
                    
                    // 7. Restore viewport
                    let restore_metrics = format!(
                        r#"{{
                            "width": {},
                            "height": {},
                            "deviceScaleFactor": 1,
                            "mobile": false
                        }}"#,
                        original_width, original_height
                    );
                    let _ = webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Emulation.setDeviceMetricsOverride"),
                        &HSTRING::from(&restore_metrics),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    );
                    
                    // Process screenshot result
                    match screenshot_result {
                        Ok(base64_data) => {
                            use base64::Engine;
                            match base64::engine::general_purpose::STANDARD.decode(&base64_data) {
                                Ok(bytes) => {
                                    log_webview_success("WebViewInstance::capture_screenshot_cdp", Some(start.elapsed().as_millis()));
                                    return Ok(bytes);
                                }
                                Err(e) => {
                                    return Err(format!("Base64 decode error: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                
                // Non-full-page: simple viewport capture
                let request_id = Uuid::new_v4().to_string();
                let params = format!(
                    r#"{{
                        "format": "{}"
                        {}
                    }}"#,
                    format,
                    if let Some(q) = quality { format!(r#", "quality": {}"#, q) } else { String::new() }
                );
                
                log_webview_debug("capture_screenshot_cdp", &format!("Calling Page.captureScreenshot, request_id={}", request_id));
                
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Page.captureScreenshot"),
                    &HSTRING::from(&params),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpScreenshotHandler {
                        request_id: request_id.clone(),
                    }),
                ).map_err(|e| format!("CDP call failed: {:?}", e))?;
                
                // Wait for result while pumping messages
                let start = std::time::Instant::now();
                loop {
                    // Pump messages
                    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                    while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                        &mut msg,
                        HWND::default(),
                        0,
                        0,
                        windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                    ).as_bool() {
                        windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                    
                    // Check for result
                    if let Some(result) = PENDING_CDP_SCREENSHOTS.with(|map| map.borrow_mut().remove(&request_id)) {
                        match result {
                            Ok(base64_data) => {
                                // Decode base64 to bytes
                                use base64::Engine;
                                match base64::engine::general_purpose::STANDARD.decode(&base64_data) {
                                    Ok(bytes) => {
                                        log_webview_success("WebViewInstance::capture_screenshot_cdp", Some(start.elapsed().as_millis()));
                                        return Ok(bytes);
                                    }
                                    Err(e) => {
                                        log_webview_error("capture_screenshot_cdp", &format!("Base64 decode error: {}", e));
                                        return Err(format!("Base64 decode error: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                log_webview_error("capture_screenshot_cdp", &e);
                                return Err(e);
                            }
                        }
                    }
                    
                    if start.elapsed() > std::time::Duration::from_secs(60) {
                        log_webview_error("capture_screenshot_cdp", "Timeout");
                        return Err("CDP screenshot timeout".to_string());
                    }
                    
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        } else {
            log_webview_error("WebViewInstance::capture_screenshot_cdp", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    pub fn close(&mut self) -> WinResult<()> {
        log_webview_start("WebViewInstance::close", "");
        if let Some(controller) = self.controller.take() {
            unsafe {
                controller.Close()?;
            }
        }
        self.window.close();
        log_webview_success("WebViewInstance::close", None);
        Ok(())
    }

    pub fn click(&self, selector: &str) -> WinResult<()> {
        log_webview_start("WebViewInstance::click", &format!("selector={}", selector));
        let script = format!("document.querySelector('{}')?.click()", selector);
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                webview.ExecuteScript(
                    &HSTRING::from(&script),
                    &ICoreWebView2ExecuteScriptCompletedHandler::from(FireAndForgetHandler),
                )?;
            }
            log_webview_success("WebViewInstance::click", None);
            Ok(())
        } else {
            log_webview_error("WebViewInstance::click", "Controller is not ready");
            Err(Error::from_win32())
        }
    }

    pub fn type_text(&self, selector: &str, text: &str) -> WinResult<()> {
        log_webview_start(
            "WebViewInstance::type_text",
            &format!("selector={}, text={}", selector, text),
        );
        let script = format!(
            "let el = document.querySelector('{}'); if (el) {{ el.value = '{}'; }}",
            selector, text
        );
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                webview.ExecuteScript(
                    &HSTRING::from(&script),
                    &ICoreWebView2ExecuteScriptCompletedHandler::from(FireAndForgetHandler),
                )?;
            }
            log_webview_success("WebViewInstance::type_text", None);
            Ok(())
        } else {
            log_webview_error("WebViewInstance::type_text", "Controller is not ready");
            Err(Error::from_win32())
        }
    }

    pub fn press_key(&self, key: &str) -> WinResult<()> {
        log_webview_start("WebViewInstance::press_key", &format!("key={}", key));
        let script = format!(
            "document.dispatchEvent(new KeyboardEvent('keydown', {{key: '{}', bubbles: true}}));",
            key
        );
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                webview.ExecuteScript(
                    &HSTRING::from(&script),
                    &ICoreWebView2ExecuteScriptCompletedHandler::from(FireAndForgetHandler),
                )?;
            }
            log_webview_success("WebViewInstance::press_key", None);
            Ok(())
        } else {
            log_webview_error("WebViewInstance::press_key", "Controller is not ready");
            Err(Error::from_win32())
        }
    }

    // ========================================================================
    // CDP Input Methods (for bot detection evasion)
    // ========================================================================

    /// Click at coordinates using CDP Input.dispatchMouseEvent
    /// This is more bot-detection resistant than JavaScript click()
    /// human_mode: enables bezier curve mouse movement with jitter
    pub fn click_cdp(&self, x: f64, y: f64, human_mode: bool) -> Result<(), String> {
        log_webview_start("WebViewInstance::click_cdp", &format!("x={}, y={}, human={}", x, y, human_mode));
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                
                use rand::Rng;
                let mut rng = rand::thread_rng();
                
                if human_mode {
                    // Human-like mouse movement with bezier curve
                    
                    // Random start position (simulating where mouse might be)
                    let start_x: f64 = rng.r#gen::<f64>() * 800.0;
                    let start_y: f64 = rng.r#gen::<f64>() * 100.0; // Start from top area
                    
                    // Distance for timing calculations
                    let distance = ((x - start_x).powi(2) + (y - start_y).powi(2)).sqrt();
                    let curviness = 0.2 + rng.r#gen::<f64>() * 0.3;
                    
                    // Bezier control points (perpendicular offset for natural arc)
                    let mid_x = (start_x + x) / 2.0;
                    let mid_y = (start_y + y) / 2.0;
                    let perp_x = -(y - start_y) / distance * curviness * distance;
                    let perp_y = (x - start_x) / distance * curviness * distance;
                    
                    let cp1x = mid_x + perp_x * (0.3 + rng.r#gen::<f64>() * 0.4);
                    let cp1y = mid_y + perp_y * (0.3 + rng.r#gen::<f64>() * 0.4);
                    let cp2x = mid_x + perp_x * (0.6 + rng.r#gen::<f64>() * 0.4);
                    let cp2y = mid_y + perp_y * (0.6 + rng.r#gen::<f64>() * 0.4);
                    
                    // Cubic bezier interpolation
                    let bezier = |t: f64, p0: f64, p1: f64, p2: f64, p3: f64| -> f64 {
                        let u = 1.0 - t;
                        u.powi(3) * p0 + 3.0 * u.powi(2) * t * p1 + 3.0 * u * t.powi(2) * p2 + t.powi(3) * p3
                    };
                    
                    // Ease-in-out function
                    let ease_in_out = |t: f64| -> f64 {
                        if t < 0.5 {
                            4.0 * t.powi(3)
                        } else {
                            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                        }
                    };
                    
                    // Number of steps based on distance
                    let steps = (20.max((distance / 15.0) as i32).min(60) + rng.gen_range(0..10)) as usize;
                    let total_time_ms = 200.0 + distance * 0.8 + rng.r#gen::<f64>() * 150.0;
                    
                    for i in 0..=steps {
                        let linear_t = i as f64 / steps as f64;
                        let t = ease_in_out(linear_t);
                        
                        let bx = bezier(t, start_x, cp1x, cp2x, x);
                        let by = bezier(t, start_y, cp1y, cp2y, y);
                        
                        // Micro-jitter (decreases near target)
                        let jitter_scale = 3.0 * (1.0 - linear_t * 0.7);
                        let jitter_x = (rng.r#gen::<f64>() - 0.5) * jitter_scale;
                        let jitter_y = (rng.r#gen::<f64>() - 0.5) * jitter_scale;
                        
                        let move_x = bx + jitter_x;
                        let move_y = by + jitter_y;
                        
                        // Send mouseMoved event
                        let mouse_move = format!(
                            r#"{{"type": "mouseMoved", "x": {}, "y": {}}}"#,
                            move_x, move_y
                        );
                        webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Input.dispatchMouseEvent"),
                            &HSTRING::from(&mouse_move),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        ).ok();
                        
                        // Variable delay (faster in middle)
                        let speed_factor = 0.5 + (linear_t * std::f64::consts::PI).sin() * 0.5;
                        let base_delay = total_time_ms / steps as f64;
                        let delay = (base_delay / speed_factor) * (0.7 + rng.r#gen::<f64>() * 0.6);
                        std::thread::sleep(std::time::Duration::from_millis(delay as u64));
                    }
                    
                    // Occasional overshoot and correction (10%)
                    if rng.r#gen::<f64>() < 0.1 {
                        let overshoot_x = x + (rng.r#gen::<f64>() - 0.5) * 20.0;
                        let overshoot_y = y + (rng.r#gen::<f64>() - 0.5) * 15.0;
                        let overshoot_move = format!(
                            r#"{{"type": "mouseMoved", "x": {}, "y": {}}}"#,
                            overshoot_x, overshoot_y
                        );
                        webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Input.dispatchMouseEvent"),
                            &HSTRING::from(&overshoot_move),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        ).ok();
                        std::thread::sleep(std::time::Duration::from_millis(30 + rng.gen_range(0..50)));
                        
                        // Correct back to target
                        let correct_move = format!(
                            r#"{{"type": "mouseMoved", "x": {}, "y": {}}}"#,
                            x, y
                        );
                        webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Input.dispatchMouseEvent"),
                            &HSTRING::from(&correct_move),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        ).ok();
                    }
                    
                    // Small pause before click
                    std::thread::sleep(std::time::Duration::from_millis(30 + rng.gen_range(0..50)));
                }
                
                // Mouse down
                let mouse_down = format!(
                    r#"{{"type": "mousePressed", "x": {}, "y": {}, "button": "left", "clickCount": 1}}"#,
                    x, y
                );
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Input.dispatchMouseEvent"),
                    &HSTRING::from(&mouse_down),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                ).map_err(|e| format!("CDP mousePressed failed: {:?}", e))?;
                
                // Delay between down and up (varies for human_mode)
                let click_delay = if human_mode {
                    50 + rng.gen_range(0..100)
                } else {
                    50
                };
                std::thread::sleep(std::time::Duration::from_millis(click_delay));
                
                // Mouse up
                let mouse_up = format!(
                    r#"{{"type": "mouseReleased", "x": {}, "y": {}, "button": "left", "clickCount": 1}}"#,
                    x, y
                );
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Input.dispatchMouseEvent"),
                    &HSTRING::from(&mouse_up),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                ).map_err(|e| format!("CDP mouseReleased failed: {:?}", e))?;
                
                log_webview_success("WebViewInstance::click_cdp", None);
                tracing::info!("[click_cdp] CDP click at ({}, {}), human={}", x, y, human_mode);
                Ok(())
            }
        } else {
            log_webview_error("WebViewInstance::click_cdp", "Controller not ready");
            Err("Controller not ready".to_string())
        }
    }

    /// Type text using CDP Input.dispatchKeyEvent
    /// This emulates real keyboard input at the browser level
    /// human_mode: enables typo simulation, variable delays, and thinking pauses
    pub fn type_cdp(&self, text: &str, char_delay_ms: u64, human_mode: bool) -> Result<(), String> {
        log_webview_start("WebViewInstance::type_cdp", &format!("len={}, human={}", text.len(), human_mode));
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                
                // Adjacent keys for typo simulation
                let adjacent_keys: std::collections::HashMap<char, Vec<char>> = [
                    ('a', vec!['s', 'q', 'w', 'z']), ('b', vec!['v', 'n', 'g', 'h']),
                    ('c', vec!['x', 'v', 'd', 'f']), ('d', vec!['s', 'f', 'e', 'r']),
                    ('e', vec!['w', 'r', 'd', 's']), ('f', vec!['d', 'g', 'r', 't']),
                    ('g', vec!['f', 'h', 't', 'y']), ('h', vec!['g', 'j', 'y', 'u']),
                    ('i', vec!['u', 'o', 'k', 'j']), ('j', vec!['h', 'k', 'u', 'i']),
                    ('k', vec!['j', 'l', 'i', 'o']), ('l', vec!['k', 'o', 'p']),
                    ('m', vec!['n', 'j', 'k']), ('n', vec!['b', 'm', 'h', 'j']),
                    ('o', vec!['i', 'p', 'k', 'l']), ('p', vec!['o', 'l']),
                    ('q', vec!['w', 'a']), ('r', vec!['e', 't', 'd', 'f']),
                    ('s', vec!['a', 'd', 'w', 'e']), ('t', vec!['r', 'y', 'f', 'g']),
                    ('u', vec!['y', 'i', 'h', 'j']), ('v', vec!['c', 'b', 'f', 'g']),
                    ('w', vec!['q', 'e', 'a', 's']), ('x', vec!['z', 'c', 's', 'd']),
                    ('y', vec!['t', 'u', 'g', 'h']), ('z', vec!['a', 's', 'x']),
                ].iter().cloned().collect();
                
                let type_char = |webview: &ICoreWebView2, ch: char| -> Result<(), String> {
                    let escaped = ch.to_string().replace('\\', "\\\\").replace('"', "\\\"");
                    
                    let key_down = format!(r#"{{"type": "keyDown", "key": "{}"}}"#, escaped);
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Input.dispatchKeyEvent"),
                        &HSTRING::from(&key_down),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    ).map_err(|e| format!("CDP keyDown failed: {:?}", e))?;
                    
                    let char_event = format!(r#"{{"type": "char", "text": "{}"}}"#, escaped);
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Input.dispatchKeyEvent"),
                        &HSTRING::from(&char_event),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    ).map_err(|e| format!("CDP char failed: {:?}", e))?;
                    
                    let key_up = format!(r#"{{"type": "keyUp", "key": "{}"}}"#, escaped);
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Input.dispatchKeyEvent"),
                        &HSTRING::from(&key_up),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    ).map_err(|e| format!("CDP keyUp failed: {:?}", e))?;
                    
                    Ok(())
                };
                
                let type_backspace = |webview: &ICoreWebView2| -> Result<(), String> {
                    let key_down = r#"{"type": "keyDown", "key": "Backspace", "code": "Backspace"}"#;
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Input.dispatchKeyEvent"),
                        &HSTRING::from(key_down),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    ).map_err(|e| format!("CDP Backspace failed: {:?}", e))?;
                    
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    
                    let key_up = r#"{"type": "keyUp", "key": "Backspace", "code": "Backspace"}"#;
                    webview.CallDevToolsProtocolMethod(
                        &HSTRING::from("Input.dispatchKeyEvent"),
                        &HSTRING::from(key_up),
                        &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                    ).map_err(|e| format!("CDP Backspace up failed: {:?}", e))?;
                    
                    Ok(())
                };
                
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let chars: Vec<char> = text.chars().collect();
                let base_delay: u64 = 50; // Base typing speed ~20 WPM
                
                for (i, ch) in chars.iter().enumerate() {
                    let ch = *ch;
                    
                    if human_mode {
                        // 1. Adjacent key typo (3% chance for letters)
                        if ch.is_alphabetic() && rng.r#gen::<f64>() < 0.03 {
                            if let Some(typo_chars) = adjacent_keys.get(&ch.to_ascii_lowercase()) {
                                let typo_char = typo_chars[rng.gen_range(0..typo_chars.len())];
                                let typo_char = if ch.is_uppercase() { typo_char.to_ascii_uppercase() } else { typo_char };
                                
                                // Type wrong character
                                type_char(&webview, typo_char)?;
                                std::thread::sleep(std::time::Duration::from_millis(80 + rng.gen_range(0..60)));
                                
                                // Pause (realize mistake)
                                std::thread::sleep(std::time::Duration::from_millis(150 + rng.gen_range(0..200)));
                                
                                // Delete wrong character
                                type_backspace(&webview)?;
                                std::thread::sleep(std::time::Duration::from_millis(40 + rng.gen_range(0..30)));
                            }
                        }
                        
                        // 2. Double space typo (2% chance when typing space)
                        if ch == ' ' && rng.r#gen::<f64>() < 0.02 {
                            type_char(&webview, ' ')?;
                            std::thread::sleep(std::time::Duration::from_millis(50 + rng.gen_range(0..30)));
                            // Realize mistake
                            std::thread::sleep(std::time::Duration::from_millis(100 + rng.gen_range(0..150)));
                            type_backspace(&webview)?;
                        }
                        
                        // 3. Capitalize typo: forgot shift (1.5% at word start)
                        let is_word_start = i == 0 || chars[i - 1] == ' ' || chars[i - 1] == '.';
                        if is_word_start && ch.is_uppercase() && ch.is_alphabetic() && rng.r#gen::<f64>() < 0.015 {
                            // Type lowercase by mistake
                            type_char(&webview, ch.to_ascii_lowercase())?;
                            std::thread::sleep(std::time::Duration::from_millis(120 + rng.gen_range(0..80)));
                            // Notice and fix
                            type_backspace(&webview)?;
                            std::thread::sleep(std::time::Duration::from_millis(30 + rng.gen_range(0..20)));
                        }
                        
                        // 4. Capitalize typo: held shift too long (1% when prev was uppercase)
                        if i > 0 && chars[i - 1].is_uppercase() && chars[i - 1].is_alphabetic() 
                           && ch.is_lowercase() && ch.is_alphabetic() && rng.r#gen::<f64>() < 0.01 {
                            // Type uppercase by mistake  
                            type_char(&webview, ch.to_ascii_uppercase())?;
                            std::thread::sleep(std::time::Duration::from_millis(100 + rng.gen_range(0..80)));
                            // Notice and fix
                            type_backspace(&webview)?;
                            std::thread::sleep(std::time::Duration::from_millis(30 + rng.gen_range(0..20)));
                        }
                        
                        // Type correct character
                        type_char(&webview, ch)?;
                        
                        // Variable delay based on character type
                        let delay: u64 = if ['.', ',', '!', '?', ':', ';'].contains(&ch) {
                            // Punctuation = longer pause (thinking)
                            150 + rng.gen_range(0..200)
                        } else if ch == ' ' {
                            // Space after word = brief pause
                            80 + rng.gen_range(0..100)
                        } else if ch.is_numeric() {
                            // Numbers = slightly more careful
                            70 + rng.gen_range(0..60)
                        } else {
                            // Regular letters = natural variance
                            let prev_char = if i > 0 { chars[i - 1].to_ascii_lowercase() } else { ' ' };
                            let curr_char = ch.to_ascii_lowercase();
                            let fast_combos = ["th", "he", "in", "er", "an", "re", "on", "at", "en", "nd"];
                            let combo = format!("{}{}", prev_char, curr_char);
                            if fast_combos.contains(&combo.as_str()) {
                                // Faster for common letter combos (rolling fingers)
                                30 + rng.gen_range(0..30)
                            } else {
                                // baseDelay * (0.6 + random * 0.8) = 30..70ms range
                                (base_delay as f64 * (0.6 + rng.r#gen::<f64>() * 0.8)) as u64
                            }
                        };
                        
                        // Occasional longer pause (2% chance - thinking/distraction)
                        let final_delay = if rng.r#gen::<f64>() < 0.02 {
                            delay + 300 + rng.gen_range(0..500)
                        } else {
                            delay
                        };
                        
                        std::thread::sleep(std::time::Duration::from_millis(final_delay));
                    } else {
                        // Normal mode: just type with fixed delay
                        type_char(&webview, ch)?;
                        if char_delay_ms > 0 {
                            std::thread::sleep(std::time::Duration::from_millis(char_delay_ms));
                        }
                    }
                }
                
                log_webview_success("WebViewInstance::type_cdp", None);
                tracing::info!("[type_cdp] Typed {} chars via CDP (human={})", text.len(), human_mode);
                Ok(())
            }
        } else {
            log_webview_error("WebViewInstance::type_cdp", "Controller not ready");
            Err("Controller not ready".to_string())
        }
    }

    /// Press a special key using CDP Input.dispatchKeyEvent
    pub fn press_key_cdp(&self, key: &str) -> Result<(), String> {
        log_webview_start("WebViewInstance::press_key_cdp", key);
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                
                // Map common key names to CDP key identifiers
                let (key_code, code) = match key.to_lowercase().as_str() {
                    "enter" | "return" => ("Enter", "Enter"),
                    "tab" => ("Tab", "Tab"),
                    "escape" | "esc" => ("Escape", "Escape"),
                    "backspace" => ("Backspace", "Backspace"),
                    "delete" => ("Delete", "Delete"),
                    "arrowup" | "up" => ("ArrowUp", "ArrowUp"),
                    "arrowdown" | "down" => ("ArrowDown", "ArrowDown"),
                    "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft"),
                    "arrowright" | "right" => ("ArrowRight", "ArrowRight"),
                    "space" => (" ", "Space"),
                    _ => (key, key),
                };
                
                // keyDown
                let key_down = format!(
                    r#"{{"type": "keyDown", "key": "{}", "code": "{}"}}"#,
                    key_code, code
                );
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Input.dispatchKeyEvent"),
                    &HSTRING::from(&key_down),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                ).map_err(|e| format!("CDP keyDown failed: {:?}", e))?;
                
                std::thread::sleep(std::time::Duration::from_millis(30));
                
                // keyUp
                let key_up = format!(
                    r#"{{"type": "keyUp", "key": "{}", "code": "{}"}}"#,
                    key_code, code
                );
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Input.dispatchKeyEvent"),
                    &HSTRING::from(&key_up),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                ).map_err(|e| format!("CDP keyUp failed: {:?}", e))?;
                
                log_webview_success("WebViewInstance::press_key_cdp", None);
                tracing::info!("[press_key_cdp] Pressed {} via CDP", key);
                Ok(())
            }
        } else {
            log_webview_error("WebViewInstance::press_key_cdp", "Controller not ready");
            Err("Controller not ready".to_string())
        }
    }

    /// Get element coordinates by selector (for CDP click)
    pub fn get_element_center(&self, selector: &str) -> Result<(f64, f64), String> {
        log_webview_start("WebViewInstance::get_element_center", selector);
        
        let script = format!(r#"
            (function() {{
                const el = document.querySelector("{}");
                if (!el) return JSON.stringify({{ error: "Element not found" }});
                const rect = el.getBoundingClientRect();
                return JSON.stringify({{
                    x: rect.left + rect.width / 2,
                    y: rect.top + rect.height / 2
                }});
            }})()
        "#, selector.replace('"', "\\\""));
        
        let request_id = Uuid::new_v4().to_string();
        let result = self.execute_script(&script, request_id)
            .map_err(|e| format!("Script error: {:?}", e))?;
        
        let parsed: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| format!("Parse error: {}", e))?;
        
        if let Some(err) = parsed.get("error") {
            return Err(err.as_str().unwrap_or("Unknown error").to_string());
        }
        
        let x = parsed["x"].as_f64().ok_or("Missing x coordinate")?;
        let y = parsed["y"].as_f64().ok_or("Missing y coordinate")?;
        
        log_webview_success("WebViewInstance::get_element_center", None);
        Ok((x, y))
    }

    /// Click element by selector using CDP (combines get_element_center + click_cdp)
    pub fn click_selector_cdp(&self, selector: &str, human_mode: bool) -> Result<(), String> {
        let (x, y) = self.get_element_center(selector)?;
        self.click_cdp(x, y, human_mode)
    }

    // ========================================================================
    // Frame (iframe) Support via CDP
    // ========================================================================

    /// Call a CDP method and wait for the result synchronously
    fn call_cdp_sync(&self, method: &str, params: &str) -> Result<String, String> {
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                let request_id = Uuid::new_v4().to_string();

                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from(method),
                    &HSTRING::from(params),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(
                        CdpResultHandler { request_id: request_id.clone() }
                    ),
                ).map_err(|e| format!("CDP call failed: {:?}", e))?;

                // Wait for result (same pattern as wait_for_script_result)
                let start = std::time::Instant::now();
                loop {
                    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                    while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                        &mut msg, HWND::default(), 0, 0,
                        windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                    ).as_bool() {
                        windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }

                    if let Some(result) = PENDING_CDP_RESULTS.with(|map| map.borrow_mut().remove(&request_id)) {
                        return result;
                    }

                    if start.elapsed() > std::time::Duration::from_secs(30) {
                        return Err(format!("CDP timeout for {}", method));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        } else {
            Err("WebView not ready".to_string())
        }
    }

    /// Get all frames in the page via CDP Page.getFrameTree
    pub fn get_frames(&self) -> Result<serde_json::Value, String> {
        let result = self.call_cdp_sync("Page.getFrameTree", "{}")?;
        let parsed: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| format!("Failed to parse frame tree: {}", e))?;

        fn collect_frames(node: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
            if let Some(frame) = node.get("frame") {
                out.push(serde_json::json!({
                    "id": frame.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "url": frame.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    "name": frame.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "security_origin": frame.get("securityOrigin").and_then(|v| v.as_str()).unwrap_or(""),
                    "parent_id": frame.get("parentId").and_then(|v| v.as_str()),
                }));
            }
            if let Some(children) = node.get("childFrames").and_then(|v| v.as_array()) {
                for child in children {
                    collect_frames(child, out);
                }
            }
        }

        let mut frames = Vec::new();
        if let Some(tree) = parsed.get("frameTree") {
            collect_frames(tree, &mut frames);
        }
        Ok(serde_json::json!({ "frames": frames }))
    }

    /// Resolve a frame specifier (URL substring or frame ID) to a CDP frame ID
    fn resolve_frame_id(&self, frame_spec: &str) -> Result<String, String> {
        let result = self.call_cdp_sync("Page.getFrameTree", "{}")?;
        let parsed: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| format!("Failed to parse frame tree: {}", e))?;

        fn find_frame(node: &serde_json::Value, spec: &str) -> Option<String> {
            if let Some(frame) = node.get("frame") {
                let id = frame.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let url = frame.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let name = frame.get("name").and_then(|v| v.as_str()).unwrap_or("");
                // Match by: exact frame ID, URL contains, or name matches
                if id == spec || url.contains(spec) || (!name.is_empty() && name == spec) {
                    return Some(id.to_string());
                }
            }
            if let Some(children) = node.get("childFrames").and_then(|v| v.as_array()) {
                for child in children {
                    if let Some(found) = find_frame(child, spec) {
                        return Some(found);
                    }
                }
            }
            None
        }

        if let Some(tree) = parsed.get("frameTree") {
            find_frame(tree, frame_spec)
                .ok_or_else(|| format!("Frame not found matching: {}", frame_spec))
        } else {
            Err("No frame tree in response".to_string())
        }
    }

    /// Execute a script in a specific frame via CDP Runtime.evaluate
    /// Uses Page.createIsolatedWorld to get an executionContextId for the frame,
    /// then calls Runtime.evaluate with that contextId.
    pub fn execute_in_frame(&self, script: &str, frame_spec: &str) -> Result<String, String> {
        let frame_id = self.resolve_frame_id(frame_spec)?;
        tracing::info!("[Frame] Resolved '{}' → frame_id={}", frame_spec, frame_id);

        // Create an isolated world in the target frame to get a contextId
        let create_params = serde_json::json!({
            "frameId": frame_id,
            "worldName": "webview-bridge-frame"
        });
        let world_result = self.call_cdp_sync(
            "Page.createIsolatedWorld",
            &create_params.to_string(),
        )?;
        let world_parsed: serde_json::Value = serde_json::from_str(&world_result)
            .map_err(|e| format!("Failed to parse isolated world: {}", e))?;
        let context_id = world_parsed.get("executionContextId")
            .and_then(|v| v.as_i64())
            .ok_or("No executionContextId in response")?;

        tracing::info!("[Frame] Got contextId={} for frame_id={}", context_id, frame_id);

        // Execute the script in that context
        let eval_params = serde_json::json!({
            "expression": script,
            "contextId": context_id,
            "returnByValue": true,
            "awaitPromise": true,
        });
        let eval_result = self.call_cdp_sync(
            "Runtime.evaluate",
            &eval_params.to_string(),
        )?;
        let eval_parsed: serde_json::Value = serde_json::from_str(&eval_result)
            .map_err(|e| format!("Failed to parse eval result: {}", e))?;

        // Check for exceptions
        if let Some(exception) = eval_parsed.get("exceptionDetails") {
            let msg = exception.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("Script exception in frame");
            return Err(format!("Frame script error: {}", msg));
        }

        // Extract the result value
        if let Some(result) = eval_parsed.get("result") {
            if let Some(val) = result.get("value") {
                Ok(val.to_string())
            } else {
                // No value (void return) — return the type/description
                let desc = result.get("description")
                    .or_else(|| result.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("undefined");
                Ok(format!("\"{}\"", desc))
            }
        } else {
            Ok(eval_result)
        }
    }

    // ========================================================================
    // Snapshot, Cookie, etc.
    // ========================================================================

    /// Check if the WebView controller is ready
    pub fn is_ready(&self) -> bool {
        self.controller.is_some()
    }

    /// Check if the underlying window handle is still valid (not destroyed by user)
    pub fn is_window_valid(&self) -> bool {
        self.window.is_valid()
    }

    /// Get page snapshot (HTML, ARIA, or text)
    pub fn snapshot(&self, format: &str) -> Result<String, String> {
        log_webview_start("WebViewInstance::snapshot", &format!("format={}", format));
        let script = match format {
            "aria" => {
                r#"
                (function() {
                    function buildAriaTree(element, depth) {
                        depth = depth || 0;
                        if (!element) return '';
                        const tag = element.tagName?.toLowerCase() || '';
                        const role = element.getAttribute('role') || '';
                        const name = element.getAttribute('aria-label') || element.getAttribute('name') || element.textContent?.slice(0, 50) || '';
                        const indent = '  '.repeat(depth);
                        if (role) {
                            return indent + '[' + tag + ' role="' + role + '" name="' + name.trim() + '"]' + '\n' +
                                Array.from(element.children).map(c => buildAriaTree(c, depth + 1)).join('');
                        }
                        return Array.from(element.children).map(c => buildAriaTree(c, depth)).join('');
                    }
                    return buildAriaTree(document.body);
                })();
            "#
            }
            "html" => "document.documentElement.outerHTML;",
            "text" => "document.body.innerText;",
            _ => return Err(format!("Unknown snapshot format: {}", format)),
        };

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                let request_id = Uuid::new_v4().to_string();

                log_webview_debug(
                    "WebViewInstance::snapshot",
                    &format!("Executing script, request_id={}", request_id),
                );
                webview
                    .ExecuteScript(
                        &HSTRING::from(script),
                        &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                            request_id: request_id.clone(),
                            hwnd: self.get_hwnd(),
                        }),
                    )
                    .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;

                // Wait for result
                let start = std::time::Instant::now();
                match self.wait_for_script_result(&request_id) {
                    Ok(result) => {
                        log_webview_success(
                            "WebViewInstance::snapshot",
                            Some(start.elapsed().as_millis()),
                        );
                        Ok(result)
                    }
                    Err(e) => {
                        log_webview_error("WebViewInstance::snapshot", &e);
                        Err(e)
                    }
                }
            }
        } else {
            log_webview_error("WebViewInstance::snapshot", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    /// Get all cookies via CDP (Network.getCookies)
    /// Returns all cookies including HttpOnly, Secure, SameSite attributes
    pub fn get_cookies(&self) -> WinResult<Vec<CookieInfo>> {
        log_webview_start("WebViewInstance::get_cookies", "CDP");
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                let request_id = Uuid::new_v4().to_string();
                
                // Call CDP Network.getCookies
                let params = "{}"; // Empty params gets all cookies for current URL
                
                log_webview_debug(
                    "WebViewInstance::get_cookies",
                    &format!("CDP Network.getCookies, request_id={}", request_id),
                );
                
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Network.getCookies"),
                    &HSTRING::from(params),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(
                        CdpCookieHandler {
                            request_id: request_id.clone(),
                        }
                    ),
                )?;
                
                // Wait for CDP result
                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(10);
                
                loop {
                    if start.elapsed() > timeout {
                        log_webview_error("WebViewInstance::get_cookies", "CDP timeout");
                        return Err(Error::new(
                            windows::core::HRESULT(0x80070057u32 as i32),
                            HSTRING::from("CDP timeout"),
                        ));
                    }
                    
                    // Check for result
                    let result = PENDING_CDP_COOKIES.with(|map| {
                        map.borrow_mut().remove(&request_id)
                    });
                    
                    if let Some(res) = result {
                        match res {
                            Ok(json_str) => {
                                // Parse CDP response: {"cookies": [...]}
                                let parsed: serde_json::Value = serde_json::from_str(&json_str)
                                    .map_err(|e| Error::new(
                                        windows::core::HRESULT(0x80070057u32 as i32),
                                        HSTRING::from(format!("Failed to parse CDP response: {}", e)),
                                    ))?;
                                
                                if let Some(cookies_arr) = parsed.get("cookies") {
                                    let cookies: Vec<CookieInfo> = serde_json::from_value(cookies_arr.clone())
                                        .map_err(|e| Error::new(
                                            windows::core::HRESULT(0x80070057u32 as i32),
                                            HSTRING::from(format!("Failed to parse cookies: {}", e)),
                                        ))?;
                                    
                                    log_webview_success(
                                        "WebViewInstance::get_cookies",
                                        Some(start.elapsed().as_millis()),
                                    );
                                    tracing::info!("[get_cookies] CDP returned {} cookies (including HttpOnly)", cookies.len());
                                    return Ok(cookies);
                                } else {
                                    log_webview_error("WebViewInstance::get_cookies", "No cookies in response");
                                    return Ok(vec![]);
                                }
                            }
                            Err(e) => {
                                log_webview_error("WebViewInstance::get_cookies", &e);
                                return Err(Error::new(
                                    windows::core::HRESULT(0x80070057u32 as i32),
                                    HSTRING::from(e),
                                ));
                            }
                        }
                    }
                    
                    // Process message pump
                    use windows::Win32::UI::WindowsAndMessaging::{MSG, PeekMessageW, TranslateMessage, DispatchMessageW, PM_REMOVE};
                    let mut msg = MSG::default();
                    while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        } else {
            log_webview_error("WebViewInstance::get_cookies", "WebView not ready");
            Err(Error::new(
                windows::core::HRESULT(0x80070057u32 as i32),
                HSTRING::from("WebView not ready"),
            ))
        }
    }

    /// Set a cookie via CDP (Network.setCookie)
    /// Supports HttpOnly, Secure, SameSite attributes
    pub fn set_cookie(&self, name: &str, value: &str, domain: &str) -> Result<(), String> {
        self.set_cookie_full(CookieInfo {
            name: name.to_string(),
            value: value.to_string(),
            domain: domain.to_string(),
            path: "/".to_string(),
            expires: None,
            size: None,
            http_only: false,
            secure: false,
            session: true,
            same_site: None,
            priority: None,
        })
    }
    
    /// Set a cookie with full attributes via CDP (Network.setCookie)
    pub fn set_cookie_full(&self, cookie: CookieInfo) -> Result<(), String> {
        log_webview_start(
            "WebViewInstance::set_cookie",
            &format!("name={}, domain={}, httpOnly={}", cookie.name, cookie.domain, cookie.http_only),
        );
        
        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                
                let request_id = Uuid::new_v4().to_string();
                
                // Build CDP Network.setCookie params
                let mut params = serde_json::json!({
                    "name": cookie.name,
                    "value": cookie.value,
                    "domain": cookie.domain,
                    "path": cookie.path,
                });
                
                if let Some(expires) = cookie.expires {
                    params["expires"] = serde_json::json!(expires);
                }
                if cookie.http_only {
                    params["httpOnly"] = serde_json::json!(true);
                }
                if cookie.secure {
                    params["secure"] = serde_json::json!(true);
                }
                if let Some(ref same_site) = cookie.same_site {
                    params["sameSite"] = serde_json::json!(same_site);
                }
                
                let params_str = params.to_string();
                
                webview.CallDevToolsProtocolMethod(
                    &HSTRING::from("Network.setCookie"),
                    &HSTRING::from(&params_str),
                    &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(
                        CdpCookieHandler {
                            request_id: request_id.clone(),
                        }
                    ),
                ).map_err(|e| format!("CDP call failed: {:?}", e))?;
                
                // Wait for CDP result  
                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(5);
                
                loop {
                    if start.elapsed() > timeout {
                        log_webview_error("WebViewInstance::set_cookie", "CDP timeout");
                        return Err("CDP timeout".to_string());
                    }
                    
                    let result = PENDING_CDP_COOKIES.with(|map| {
                        map.borrow_mut().remove(&request_id)
                    });
                    
                    if let Some(res) = result {
                        match res {
                            Ok(json_str) => {
                                // Check CDP response: {"success": true/false}
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                                    if parsed.get("success").and_then(|v| v.as_bool()) == Some(false) {
                                        log_webview_error("WebViewInstance::set_cookie", "Cookie was not set (success=false)");
                                        return Err("Cookie was not set".to_string());
                                    }
                                }
                                log_webview_success("WebViewInstance::set_cookie", Some(start.elapsed().as_millis()));
                                return Ok(());
                            }
                            Err(e) => {
                                log_webview_error("WebViewInstance::set_cookie", &e);
                                return Err(e);
                            }
                        }
                    }
                    
                    // Process message pump
                    use windows::Win32::UI::WindowsAndMessaging::{MSG, PeekMessageW, TranslateMessage, DispatchMessageW, PM_REMOVE};
                    let mut msg = MSG::default();
                    while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        } else {
            log_webview_error("WebViewInstance::set_cookie", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    /// Take a screenshot and return as base64-encoded PNG
    /// Wrapper that calls capture_screenshot_cdp() for backward compatibility
    pub fn screenshot(&self) -> Result<String, String> {
        log_webview_start("WebViewInstance::screenshot", "CDP wrapper");
        
        // Use CDP implementation (viewport only, PNG format)
        match self.capture_screenshot_cdp(false, "png", None) {
            Ok(png_data) => {
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                let base64_data = STANDARD.encode(&png_data);
                log_webview_success("WebViewInstance::screenshot", None);
                Ok(base64_data)
            }
            Err(e) => {
                log_webview_error("WebViewInstance::screenshot", &e);
                Err(e)
            }
        }
    }

    /// Get cookies as JSON string via CDP
    /// Returns all cookies including HttpOnly
    pub fn get_cookies_json(&self) -> Result<String, String> {
        log_webview_start("WebViewInstance::get_cookies_json", "CDP");
        
        match self.get_cookies() {
            Ok(cookies) => {
                let json = serde_json::to_string(&cookies)
                    .map_err(|e| format!("Failed to serialize cookies: {}", e))?;
                log_webview_success("WebViewInstance::get_cookies_json", None);
                Ok(json)
            }
            Err(e) => {
                log_webview_error("WebViewInstance::get_cookies_json", &format!("{:?}", e));
                Err(format!("Failed to get cookies: {:?}", e))
            }
        }
    }

    /// Set cookies from JSON string via CDP
    /// Supports HttpOnly, Secure, SameSite attributes
    pub fn set_cookies_json(&self, cookies_json: &str) -> Result<(), String> {
        log_webview_start("WebViewInstance::set_cookies_json", "CDP");
        
        // Parse JSON as array of CookieInfo
        let cookies: Vec<CookieInfo> = serde_json::from_str(cookies_json)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        
        let count = cookies.len();
        for cookie in cookies {
            self.set_cookie_full(cookie)?;
        }
        
        tracing::info!("[set_cookies_json] Set {} cookies via CDP", count);
        log_webview_success("WebViewInstance::set_cookies_json", None);
        Ok(())
    }

    /// Capture screenshot using CDP (backward compatibility wrapper)
    /// Previously used native CapturePreview API, now uses CDP Page.captureScreenshot
    pub fn capture_preview_native(&self) -> Result<Vec<u8>, String> {
        log_webview_start("WebViewInstance::capture_preview_native", "CDP wrapper");
        
        // Delegate to CDP implementation (viewport only, PNG format)
        self.capture_screenshot_cdp(false, "png", None)
    }

    /// Wait for a selector to appear in the DOM
    pub fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<bool, String> {
        log_webview_start("WebViewInstance::wait_for_selector", &format!("selector={}, timeout={}ms", selector, timeout_ms));
        
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let escaped_selector = selector.replace("'", "\\'");
        
        while start.elapsed() < timeout {
            // Simple synchronous check script
            let script = format!(
                "(function() {{ return document.querySelector('{}') !== null; }})()",
                escaped_selector
            );
            
            if let Some(controller) = &self.controller {
                unsafe {
                    let webview = controller
                        .CoreWebView2()
                        .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                    let request_id = Uuid::new_v4().to_string();

                    webview
                        .ExecuteScript(
                            &HSTRING::from(&script),
                            &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                                request_id: request_id.clone(),
                                hwnd: self.get_hwnd(),
                            }),
                        )
                        .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;

                    // Wait for result with short timeout
                    let poll_start = std::time::Instant::now();
                    let poll_timeout = std::time::Duration::from_secs(2);
                    
                    loop {
                        // Pump messages
                        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                        while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                            &mut msg,
                            HWND::default(),
                            0,
                            0,
                            windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                        )
                        .as_bool()
                        {
                            windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                        }

                        if let Some(result) = PENDING_SCRIPT_RESULTS.with(|map| map.borrow_mut().remove(&request_id)) {
                            match result {
                                Ok(val) => {
                                    if val == "true" {
                                        log_webview_success("WebViewInstance::wait_for_selector", Some(start.elapsed().as_millis()));
                                        return Ok(true);
                                    }
                                    // Element not found yet, continue polling
                                    break;
                                }
                                Err(_) => break, // Try again
                            }
                        }

                        if poll_start.elapsed() > poll_timeout {
                            break;
                        }

                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            } else {
                log_webview_error("WebViewInstance::wait_for_selector", "WebView not ready");
                return Err("WebView not ready".to_string());
            }
            
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        // Final check
        log_webview_success("WebViewInstance::wait_for_selector", Some(start.elapsed().as_millis()));
        Ok(false)
    }

    /// Extract data from DOM elements
    pub fn extract(&self, selector: &str, attribute: &str, extract_all: bool) -> Result<String, String> {
        log_webview_start("WebViewInstance::extract", &format!("selector={}, attr={}, all={}", selector, attribute, extract_all));
        
        let script = if extract_all {
            format!(r#"
                (function() {{
                    const elements = document.querySelectorAll('{}');
                    return Array.from(elements).map(el => {{
                        switch ('{}') {{
                            case 'text': return el.innerText || el.textContent;
                            case 'html': return el.innerHTML;
                            case 'outerHtml': return el.outerHTML;
                            case 'href': return el.href || el.getAttribute('href');
                            case 'src': return el.src || el.getAttribute('src');
                            case 'value': return el.value;
                            default: return el.getAttribute('{}') || el.innerText;
                        }}
                    }});
                }})();
            "#, selector.replace("'", "\\'"), attribute, attribute)
        } else {
            format!(r#"
                (function() {{
                    const el = document.querySelector('{}');
                    if (!el) return null;
                    switch ('{}') {{
                        case 'text': return el.innerText || el.textContent;
                        case 'html': return el.innerHTML;
                        case 'outerHtml': return el.outerHTML;
                        case 'href': return el.href || el.getAttribute('href');
                        case 'src': return el.src || el.getAttribute('src');
                        case 'value': return el.value;
                        default: return el.getAttribute('{}') || el.innerText;
                    }}
                }})();
            "#, selector.replace("'", "\\'"), attribute, attribute)
        };

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                let request_id = Uuid::new_v4().to_string();

                webview
                    .ExecuteScript(
                        &HSTRING::from(&script),
                        &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                            request_id: request_id.clone(),
                            hwnd: self.get_hwnd(),
                        }),
                    )
                    .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;

                match self.wait_for_script_result(&request_id) {
                    Ok(result) => {
                        log_webview_success("WebViewInstance::extract", None);
                        Ok(result)
                    }
                    Err(e) => {
                        log_webview_error("WebViewInstance::extract", &e);
                        Err(e)
                    }
                }
            }
        } else {
            log_webview_error("WebViewInstance::extract", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }
    pub fn manage_network(&self, action: crate::core::NetworkAction) -> Result<String, String> {
        log_webview_start("WebViewInstance::manage_network", &format!("action={:?}", action));

        match action {
            crate::core::NetworkAction::Enable { max_logs } => {
                let max = max_logs.unwrap_or(100);
                MAX_NETWORK_LOGS.with(|m| *m.borrow_mut() = max);
                NETWORK_MONITORING_ENABLED.with(|e| *e.borrow_mut() = true);
                
                if let Some(controller) = &self.controller {
                    unsafe {
                        let webview = controller.CoreWebView2().map_err(|e| format!("{:?}", e))?;
                        
                        // Register event handlers via GetDevToolsProtocolEventReceiver
                        let req_receiver = webview.GetDevToolsProtocolEventReceiver(
                            &HSTRING::from("Network.requestWillBeSent"),
                        ).map_err(|e| format!("{:?}", e))?;
                        let mut token_req: windows::Win32::System::WinRT::EventRegistrationToken = Default::default();
                        req_receiver.add_DevToolsProtocolEventReceived(
                            &ICoreWebView2DevToolsProtocolEventReceivedEventHandler::from(NetworkRequestReceivedHandler),
                            &mut token_req,
                        ).map_err(|e| format!("{:?}", e))?;
                        
                        let res_receiver = webview.GetDevToolsProtocolEventReceiver(
                            &HSTRING::from("Network.responseReceived"),
                        ).map_err(|e| format!("{:?}", e))?;
                        let mut token_res: windows::Win32::System::WinRT::EventRegistrationToken = Default::default();
                        res_receiver.add_DevToolsProtocolEventReceived(
                            &ICoreWebView2DevToolsProtocolEventReceivedEventHandler::from(NetworkResponseReceivedHandler),
                            &mut token_res,
                        ).map_err(|e| format!("{:?}", e))?;
                        
                        NETWORK_EVENT_TOKENS.with(|map| {
                            let mut m = map.borrow_mut();
                            m.insert("requestWillBeSent".to_string(), token_req);
                            m.insert("responseReceived".to_string(), token_res);
                        });
                        
                        // Enable Network domain
                         webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Network.enable"),
                            &HSTRING::from("{}"),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        ).map_err(|e| format!("{:?}", e))?;
                    }
                }
                Ok("Network monitoring enabled".to_string())
            },
             crate::core::NetworkAction::Disable => {
                 NETWORK_MONITORING_ENABLED.with(|e| *e.borrow_mut() = false);
                 
                  if let Some(controller) = &self.controller {
                    unsafe {
                        let webview = controller.CoreWebView2().map_err(|e| format!("{:?}", e))?;
                        
                         // Disable Network domain
                         webview.CallDevToolsProtocolMethod(
                            &HSTRING::from("Network.disable"),
                            &HSTRING::from("{}"),
                            &ICoreWebView2CallDevToolsProtocolMethodCompletedHandler::from(CdpCompletedHandler),
                        ).map_err(|e| format!("{:?}", e))?;
                        
                        // Unregister event handlers via GetDevToolsProtocolEventReceiver
                        NETWORK_EVENT_TOKENS.with(|map| {
                            let mut m = map.borrow_mut();
                            if let Some(token) = m.remove("requestWillBeSent") {
                                if let Ok(receiver) = webview.GetDevToolsProtocolEventReceiver(&HSTRING::from("Network.requestWillBeSent")) {
                                    let _ = receiver.remove_DevToolsProtocolEventReceived(token);
                                }
                            }
                            if let Some(token) = m.remove("responseReceived") {
                                if let Ok(receiver) = webview.GetDevToolsProtocolEventReceiver(&HSTRING::from("Network.responseReceived")) {
                                    let _ = receiver.remove_DevToolsProtocolEventReceived(token);
                                }
                            }
                        });
                    }
                }
                 Ok("Network monitoring disabled".to_string())
             },
             crate::core::NetworkAction::GetLogs { filter } => {
                let logs = NETWORK_LOGS_MAP.with(|map| {
                    let m = map.borrow();
                    let mut vec: Vec<crate::core::NetworkLogEntry> = m.values().cloned().collect();
                    // Filters
                    if let Some(f) = filter {
                        vec.retain(|entry| entry.request.url.contains(&f));
                    }
                    // Sort by timestamp
                    vec.sort_by(|a, b| a.request.timestamp.partial_cmp(&b.request.timestamp).unwrap_or(std::cmp::Ordering::Equal));
                    vec
                });
                serde_json::to_string(&logs).map_err(|e| format!("{:?}", e))
             },
             crate::core::NetworkAction::ClearLogs => {
                 NETWORK_LOGS_MAP.with(|map| map.borrow_mut().clear());
                 Ok("Logs cleared".to_string())
             }
        }
    }
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Cookie information (CDP Network.Cookie format)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default)]
    pub expires: Option<f64>,
    #[serde(default)]
    pub size: Option<u32>,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub session: bool,
    #[serde(default)]
    pub same_site: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

fn default_path() -> String {
    "/".to_string()
}
