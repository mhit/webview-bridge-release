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
    pub static PENDING_SCREENSHOTS: RefCell<HashMap<String, Result<Vec<u8>, String>>> = RefCell::new(HashMap::new());
    static NAVIGATION_COMPLETION: RefCell<HashMap<usize, bool>> = RefCell::new(HashMap::new());
    static PENDING_SCRIPT_COUNT: RefCell<std::sync::atomic::AtomicUsize> = RefCell::new(std::sync::atomic::AtomicUsize::new(0));
}

pub struct WebViewInstance {
    window: WebViewWindow,
    controller: Option<ICoreWebView2Controller>,
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
    // New Methods: Snapshot, Cookie
    // ========================================================================

    /// Check if the WebView controller is ready
    pub fn is_ready(&self) -> bool {
        self.controller.is_some()
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

    /// Get cookies via JavaScript
    pub fn get_cookies(&self) -> WinResult<Vec<CookieInfo>> {
        log_webview_start("WebViewInstance::get_cookies", "");
        let script = r#"
            (function() {
                return document.cookie.split(';').map(function(c) {
                    var parts = c.trim().split('=');
                    return { name: parts[0], value: parts[1] || '' };
                });
            })();
        "#;

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller.CoreWebView2()?;
                let request_id = Uuid::new_v4().to_string();

                log_webview_debug(
                    "WebViewInstance::get_cookies",
                    &format!("Executing script, request_id={}", request_id),
                );
                webview.ExecuteScript(
                    &HSTRING::from(script),
                    &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                        request_id: request_id.clone(),
                        hwnd: self.get_hwnd(),
                    }),
                )?;

                // Wait for result
                let start = std::time::Instant::now();
                match self.wait_for_script_result(&request_id) {
                    Ok(json_str) => {
                        log_webview_success(
                            "WebViewInstance::get_cookies",
                            Some(start.elapsed().as_millis()),
                        );
                        let cookies: Vec<CookieInfo> =
                            serde_json::from_str(&json_str).map_err(|e| {
                                Error::new(
                                    windows::core::HRESULT(0x80070057u32 as i32),
                                    HSTRING::from(format!("Failed to parse cookies: {}", e)),
                                )
                            })?;
                        Ok(cookies)
                    }
                    Err(e) => {
                        log_webview_error("WebViewInstance::get_cookies", &e);
                        Err(Error::new(
                            windows::core::HRESULT(0x80070057u32 as i32),
                            HSTRING::from(e),
                        ))
                    }
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

    /// Set a cookie
    pub fn set_cookie(&self, name: &str, value: &str, domain: &str) -> Result<(), String> {
        log_webview_start(
            "WebViewInstance::set_cookie",
            &format!("name={}, value={}, domain={}", name, value, domain),
        );
        let script = format!(
            "document.cookie = '{}={}; domain={}; path=/'",
            name, value, domain
        );

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                webview
                    .ExecuteScript(
                        &HSTRING::from(&script),
                        &ICoreWebView2ExecuteScriptCompletedHandler::from(FireAndForgetHandler),
                    )
                    .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;
                log_webview_success("WebViewInstance::set_cookie", None);
                Ok(())
            }
        } else {
            log_webview_error("WebViewInstance::set_cookie", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    /// Take a screenshot and return as base64-encoded PNG
    /// Uses html2canvas library for DOM capture with fallback
    pub fn screenshot(&self) -> Result<String, String> {
        log_webview_start("WebViewInstance::screenshot", "");
        
        // JavaScript that dynamically loads html2canvas and captures the page
        // With improved error handling and fallback
        let script = r#"
            (async function() {
                try {
                    // Check if html2canvas is already loaded
                    if (typeof html2canvas === 'undefined') {
                        // Load html2canvas from CDN with timeout
                        const script = document.createElement('script');
                        script.src = 'https://cdnjs.cloudflare.com/ajax/libs/html2canvas/1.4.1/html2canvas.min.js';
                        script.crossOrigin = 'anonymous';
                        
                        const loadPromise = new Promise((resolve, reject) => {
                            script.onload = resolve;
                            script.onerror = () => reject(new Error('Failed to load html2canvas'));
                            document.head.appendChild(script);
                        });
                        
                        const timeoutPromise = new Promise((_, reject) => 
                            setTimeout(() => reject(new Error('html2canvas load timeout')), 10000)
                        );
                        
                        await Promise.race([loadPromise, timeoutPromise]);
                    }
                    
                    // Capture the page with more compatible options
                    const canvas = await html2canvas(document.body, {
                        useCORS: true,
                        allowTaint: true,
                        logging: false,
                        scale: 1,
                        backgroundColor: '#ffffff',
                        foreignObjectRendering: false,
                        removeContainer: true
                    });
                    
                    // Convert to base64 PNG (remove the data:image/png;base64, prefix for cleaner output)
                    const dataUrl = canvas.toDataURL('image/png');
                    return dataUrl.replace(/^data:image\/png;base64,/, '');
                } catch (error) {
                    // Fallback: try simple canvas approach for basic pages
                    try {
                        const canvas = document.createElement('canvas');
                        const rect = document.body.getBoundingClientRect();
                        canvas.width = Math.min(rect.width || window.innerWidth, 1920);
                        canvas.height = Math.min(rect.height || window.innerHeight, 1080);
                        const ctx = canvas.getContext('2d');
                        ctx.fillStyle = '#ffffff';
                        ctx.fillRect(0, 0, canvas.width, canvas.height);
                        ctx.fillStyle = '#000000';
                        ctx.font = '16px Arial';
                        ctx.fillText('Screenshot capture failed: ' + error.message, 20, 40);
                        ctx.fillText('URL: ' + window.location.href, 20, 70);
                        ctx.fillText('Title: ' + document.title, 20, 100);
                        return canvas.toDataURL('image/png').replace(/^data:image\/png;base64,/, '');
                    } catch (fallbackError) {
                        throw new Error('Screenshot failed: ' + error.message);
                    }
                }
            })();
        "#;

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                let request_id = Uuid::new_v4().to_string();

                log_webview_debug(
                    "WebViewInstance::screenshot",
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

                // Wait for result with longer timeout for screenshot
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
                    )
                    .as_bool()
                    {
                        windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }

                    if let Some(result) =
                        PENDING_SCRIPT_RESULTS.with(|map| map.borrow_mut().remove(&request_id))
                    {
                        match result {
                            Ok(data) => {
                                log_webview_success(
                                    "WebViewInstance::screenshot",
                                    Some(start.elapsed().as_millis()),
                                );
                                return Ok(data);
                            }
                            Err(e) => {
                                log_webview_error("WebViewInstance::screenshot", &e);
                                return Err(e);
                            }
                        }
                    }

                    if start.elapsed() > std::time::Duration::from_secs(60) {
                        return Err("Screenshot timeout".to_string());
                    }

                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        } else {
            log_webview_error("WebViewInstance::screenshot", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    /// Get cookies as JSON string
    pub fn get_cookies_json(&self) -> Result<String, String> {
        log_webview_start("WebViewInstance::get_cookies_json", "");
        let script = r#"
            (function() {
                return document.cookie.split(';').map(function(c) {
                    var parts = c.trim().split('=');
                    return { name: parts[0], value: parts.slice(1).join('=') || '' };
                }).filter(c => c.name);
            })();
        "#;

        if let Some(controller) = &self.controller {
            unsafe {
                let webview = controller
                    .CoreWebView2()
                    .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                let request_id = Uuid::new_v4().to_string();

                webview
                    .ExecuteScript(
                        &HSTRING::from(script),
                        &ICoreWebView2ExecuteScriptCompletedHandler::from(ExecuteScriptHandler {
                            request_id: request_id.clone(),
                            hwnd: self.get_hwnd(),
                        }),
                    )
                    .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;

                match self.wait_for_script_result(&request_id) {
                    Ok(result) => {
                        log_webview_success("WebViewInstance::get_cookies_json", None);
                        Ok(result)
                    }
                    Err(e) => {
                        log_webview_error("WebViewInstance::get_cookies_json", &e);
                        Err(e)
                    }
                }
            }
        } else {
            log_webview_error("WebViewInstance::get_cookies_json", "WebView not ready");
            Err("WebView not ready".to_string())
        }
    }

    /// Set cookies from JSON string
    pub fn set_cookies_json(&self, cookies_json: &str) -> Result<(), String> {
        log_webview_start("WebViewInstance::set_cookies_json", cookies_json);
        
        // Parse JSON and set each cookie
        let cookies: Vec<serde_json::Value> = serde_json::from_str(cookies_json)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        
        for cookie in cookies {
            let name = cookie["name"].as_str().unwrap_or("");
            let value = cookie["value"].as_str().unwrap_or("");
            let domain = cookie["domain"].as_str().unwrap_or("");
            let path = cookie["path"].as_str().unwrap_or("/");
            
            let script = format!(
                "document.cookie = '{}={}; domain={}; path={}'",
                name, value, domain, path
            );
            
            if let Some(controller) = &self.controller {
                unsafe {
                    let webview = controller
                        .CoreWebView2()
                        .map_err(|e| format!("CoreWebView2 error: {:?}", e))?;
                    webview
                        .ExecuteScript(
                            &HSTRING::from(&script),
                            &ICoreWebView2ExecuteScriptCompletedHandler::from(FireAndForgetHandler),
                        )
                        .map_err(|e| format!("ExecuteScript failed: {:?}", e))?;
                }
            }
        }
        
        log_webview_success("WebViewInstance::set_cookies_json", None);
        Ok(())
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
}

// ============================================================================
// Supporting Types
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
}
