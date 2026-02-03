use std::ptr;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
};
use webview2_com::*;
use crate::core::SessionStatus;

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
                lpszClassName: window_class,
                lpfnWndProc: Some(DefWindowProcW),
                ..Default::default()
            };

            RegisterClassW(&wc);

            let style = if visible {
                WS_OVERLAPPEDWINDOW
            } else {
                WS_OVERLAPPED // 擬似ヘッドレス用（非表示または画面外）
            };

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                window_class,
                HSTRING::from(title),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                instance,
                None,
            );

            if hwnd.0 == 0 {
                return Err(Error::from_win32());
            }

            if visible {
                ShowWindow(hwnd, SW_SHOW);
            }

            Ok(Self { hwnd })
        }
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }
}

// TODO: WebView2のCOM初期化ロジックを追加（これはWindows実機でのデバッグが必須）

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_creation() {
        // 注: このテストはWindows環境でのみ成功する
        if cfg!(windows) {
            let window = WebViewWindow::create("TDD Test Window", false);
            assert!(window.is_ok());
        }
    }
}
