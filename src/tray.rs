//! System tray icon module.
//!
//! Creates a notification area (system tray) icon with a right-click context
//! menu offering "Open Dashboard" and "Exit" actions.

use std::sync::mpsc::Sender;
use windows::{
    core::{w, PCWSTR},
    Win32::Foundation::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
};

const WM_TRAYICON: u32 = WM_USER + 1;
const IDM_DASHBOARD: u32 = 1001;
const IDM_EXIT: u32 = 1002;

/// Run the system tray icon on the current thread (blocking Win32 message loop).
///
/// * `port` – HTTP server port, used to build the dashboard URL.
/// * `shutdown_tx` – Sending side; dropping or sending signals the server to shut down.
pub fn run(port: u16, shutdown_tx: Sender<()>) {
    unsafe {
        let instance = GetModuleHandleW(None).expect("GetModuleHandleW");

        // Register a minimal window class for the invisible message-only window.
        let class_name = w!("WebViewBridgeTray");
        let wc = WNDCLASSW {
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(wnd_proc),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Create an invisible message-only window.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            w!("WebView Bridge Tray"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE, // message-only window
            None,
            instance,
            std::ptr::null(),
        );
        assert!(hwnd.0 != 0, "CreateWindowExW for tray failed");

        // Store port & shutdown_tx in window user data.
        let data = Box::new(TrayData {
            port,
            shutdown_tx: Some(shutdown_tx),
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(data) as isize);

        // Load the embedded application icon (resource ID 1 set by winres).
        let icon = LoadIconW(Some(instance.into()), PCWSTR(1 as *const u16))
            .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap());

        // Add tray icon via Shell_NotifyIconW.
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = icon;
        let tip = "WebView Bridge";
        let tip_wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let len = tip_wide.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        Shell_NotifyIconW(NIM_ADD, &nid);

        // Win32 message loop — runs until WM_QUIT.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup: remove the tray icon.
        Shell_NotifyIconW(NIM_DELETE, &nid);

        // Free TrayData.
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayData;
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr);
        }
    }
}

struct TrayData {
    port: u16,
    shutdown_tx: Option<Sender<()>>,
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAYICON => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    show_context_menu(hwnd);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u32;
                match id {
                    IDM_DASHBOARD => {
                        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayData;
                        if !ptr.is_null() {
                            let data = &*ptr;
                            let url: Vec<u16> = format!("http://localhost:{}", data.port)
                                .encode_utf16()
                                .chain(std::iter::once(0))
                                .collect();
                            windows::Win32::UI::Shell::ShellExecuteW(
                                None,
                                w!("open"),
                                PCWSTR(url.as_ptr()),
                                None,
                                None,
                                SW_SHOWNORMAL.0 as i32,
                            );
                        }
                    }
                    IDM_EXIT => {
                        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayData;
                        if !ptr.is_null() {
                            let data = &mut *ptr;
                            if let Some(tx) = data.shutdown_tx.take() {
                                let _ = tx.send(());
                            }
                        }
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn show_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().expect("CreatePopupMenu");

        let dashboard_label: Vec<u16> = "ダッシュボードを開く"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let exit_label: Vec<u16> = "終了"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        AppendMenuW(menu, MF_STRING, IDM_DASHBOARD as usize, PCWSTR(dashboard_label.as_ptr()))
            .expect("AppendMenuW dashboard");
        AppendMenuW(menu, MF_SEPARATOR, 0, None).expect("AppendMenuW separator");
        AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, PCWSTR(exit_label.as_ptr()))
            .expect("AppendMenuW exit");

        // Required: bring the window to foreground so the menu dismisses properly.
        let _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());

        let _ = DestroyMenu(menu);
    }
}
