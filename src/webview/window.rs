use std::cell::RefCell;
use std::collections::HashMap;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
use windows::{
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
    core::{Error, HSTRING, PCWSTR, Result, w},
};

// ============================================================================
// Thread-Local WebView Controller Registry
// ============================================================================

// Thread-local registry mapping HWND -> WebView2 Controller
// COM objects must stay on the same thread, so we use thread_local storage
thread_local! {
    static CONTROLLER_REGISTRY: RefCell<HashMap<isize, ICoreWebView2Controller>> = RefCell::new(HashMap::new());
}

/// Register a WebView controller for automatic resize handling
pub fn register_controller(hwnd: HWND, controller: ICoreWebView2Controller) {
    CONTROLLER_REGISTRY.with(|registry| {
        registry.borrow_mut().insert(hwnd.0 as isize, controller);
        tracing::debug!("Registered controller for HWND {:?}", hwnd);
    });
}

/// Unregister a WebView controller when the window is closed
pub fn unregister_controller(hwnd: HWND) {
    CONTROLLER_REGISTRY.with(|registry| {
        registry.borrow_mut().remove(&(hwnd.0 as isize));
        tracing::debug!("Unregistered controller for HWND {:?}", hwnd);
    });
}

/// Resize the WebView for a given HWND to match the client area
fn resize_webview(hwnd: HWND) {
    CONTROLLER_REGISTRY.with(|registry| {
        if let Some(controller) = registry.borrow().get(&(hwnd.0 as isize)) {
            unsafe {
                let mut rect = RECT::default();
                // GetClientRect returns BOOL - use as_bool() to check success
                if GetClientRect(hwnd, &mut rect).as_bool() {
                    let _ = controller.SetBounds(rect);
                    tracing::trace!("Resized WebView to {:?}", rect);
                }
            }
        }
    });
}

pub struct WebViewWindow {
    hwnd: HWND,
}

impl WebViewWindow {
    /// WebView2を表示するための親ウィンドウを作成する (Win32 API)
    pub fn create(title: &str, visible: bool) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let window_class = w!("WebViewBridgeWindow");

            let wc = WNDCLASSW {
                hInstance: instance.into(),
                lpszClassName: PCWSTR(window_class.as_ptr()),
                lpfnWndProc: Some(Self::wnd_proc),
                hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };

            if RegisterClassW(&wc) == 0 {
                let error = Error::from_win32();
                if error.code().0 as u32 != 0x80070582 && error.code().0 != 1410 {
                    return Err(error);
                }
            }

            let style = if visible {
                WS_OVERLAPPEDWINDOW
            } else {
                WS_OVERLAPPED
            };

            let (x, y) = if visible {
                (CW_USEDEFAULT, CW_USEDEFAULT)
            } else {
                (-32000, -32000)
            };

            let title_h = HSTRING::from(title);
            let hwnd = CreateWindowExW(
                if visible {
                    WINDOW_EX_STYLE::default()
                } else {
                    WS_EX_TOOLWINDOW
                },
                PCWSTR(window_class.as_ptr()),
                PCWSTR(title_h.as_ptr()),
                style,
                x,
                y,
                1024, // width
                768,  // height
                HWND::default(),
                HMENU::default(),
                instance,
                std::ptr::null_mut(),
            );

            if hwnd.0 == 0 {
                let err = Error::from_win32();
                tracing::error!("Failed to create window: {:?}", err);
                return Err(err);
            }

            tracing::info!(
                "Window created successfully: {:?}, visible={}",
                hwnd,
                visible
            );

            if visible {
                ShowWindow(hwnd, SW_SHOW);
                tracing::debug!("ShowWindow(SW_SHOW) called");
            } else {
                ShowWindow(hwnd, SW_HIDE);
                tracing::debug!("ShowWindow(SW_HIDE) called for headless mode");
            }

            Ok(Self { hwnd })
        }
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Show the window (make visible)
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
            tracing::debug!("Window shown: {:?}", self.hwnd);
        }
    }

    /// Hide the window (pseudo-headless)
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            tracing::debug!("Window hidden: {:?}", self.hwnd);
        }
    }

    /// Set window visibility
    pub fn set_visible(&self, visible: bool) {
        if visible {
            self.show();
        } else {
            self.hide();
        }
    }

    /// Check if window is visible
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }

    /// Bring window to front and focus
    pub fn bring_to_front(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
            SetForegroundWindow(self.hwnd);
            tracing::debug!("Window brought to front: {:?}", self.hwnd);
        }
    }

    pub fn close(&self) {
        // Unregister controller before destroying window
        unregister_controller(self.hwnd);
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    /// Check if the window handle is still valid (not destroyed)
    pub fn is_valid(&self) -> bool {
        unsafe { IsWindow(self.hwnd).as_bool() }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DESTROY => {
                // Unregister controller when window is destroyed
                unregister_controller(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                // Resize WebView to match new window size
                resize_webview(hwnd);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_RBUTTONUP => {
                tracing::info!("Right click detected! Capturing window diagnostic...");
                let mut rect = RECT::default();
                unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect) };
                tracing::info!("Window Rect: {:?}", rect);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
