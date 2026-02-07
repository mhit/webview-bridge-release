use windows::{
    core::{w, Error, Result, HSTRING, PCWSTR},
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};

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
        unsafe {
            IsWindowVisible(self.hwnd).as_bool()
        }
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
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DESTROY => {
                // Do not quit the message loop here, as we support multiple windows/sessions.
                // PostQuitMessage(0);
                LRESULT(0)
            }
            WM_SIZE => {
                tracing::debug!("WM_SIZE received for HWND {:?}", hwnd);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_RBUTTONUP => {
                tracing::info!("Right click detected! Capturing window diagnostic...");
                let mut rect = RECT::default();
                windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
                tracing::info!("Window Rect: {:?}", rect);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
