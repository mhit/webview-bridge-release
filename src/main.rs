#![windows_subsystem = "windows"]

pub mod api_v2;
pub mod core;
pub mod mcp_v3;
pub mod webview;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

use crate::core::{AppCommand, SessionManager, SessionOptions};
use crate::api_v2::{create_v2_router, init_auth_token, init_session_manager_v2, V2AppState};
use rand::Rng;

pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

const COMMAND_PROCESSOR_COUNT: usize = 4;

/// Command line configuration (CLI args override config.toml)
struct Config {
    bind: Option<IpAddr>,
    port: Option<u16>,
}

fn parse_args() -> Option<Config> {
    let args: Vec<String> = std::env::args().collect();
    let mut config = Config { bind: None, port: None };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("WebView Bridge Server v2");
                println!();
                println!("Usage: webview-bridge-rust.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --bind <IP>       Bind address (default: from config.toml or 127.0.0.1)");
                println!("  --port <PORT>     Port number (default: from config.toml or 9400)");
                println!("  --help, -h        Show this help message");
                return None;
            }
            "--bind" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.bind = Some(args[i].parse().unwrap_or_else(|_| {
                        eprintln!("Invalid bind address: {}", args[i]);
                        std::process::exit(1);
                    }));
                }
            }
            "--port" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.port = Some(args[i].parse().unwrap_or_else(|_| {
                        eprintln!("Invalid port number: {}", args[i]);
                        std::process::exit(1);
                    }));
                }
            }
            _ => {
                // Ignore unknown args for now
            }
        }
        i += 1;
    }

    Some(config)
}

fn main() {
    // Parse command line arguments
    let cli = match parse_args() {
        Some(c) => c,
        None => return, // --help was shown
    };

    // Resolve bind/port: CLI args > config.toml > hardcoded defaults
    let toml_cfg = crate::core::config::AppConfig::load();
    let bind: IpAddr = cli.bind
        .unwrap_or_else(|| toml_cfg.server.bind.parse().unwrap_or([127, 0, 0, 1].into()));
    let port: u16 = cli.port
        .unwrap_or(toml_cfg.server.port);

    // Channel: tray "Exit" → server graceful shutdown
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

    // Background thread: Tokio runtime + HTTP server
    let addr = SocketAddr::new(bind, port);
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run_http_server(addr, shutdown_rx));
    });

    // Main thread: Win32 message loop (system tray icon)
    webview_bridge_rust::tray::run(port, shutdown_tx);
}

/// Generate a random 32-character alphanumeric Bearer token
fn generate_auth_token() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

async fn run_http_server(addr: SocketAddr, shutdown_rx: std::sync::mpsc::Receiver<()>) {
    // Initialize config file system first
    let app_config = crate::core::config::init_config();
    {
        let cfg = app_config.read().unwrap();
        eprintln!("Config loaded: AI enabled={}, provider={}, model={}",
            cfg.ai.enabled, cfg.ai.provider, cfg.ai.model);
        if cfg.ai.api_key.is_some() {
            eprintln!("AI API key: configured from file");
        }

        // Generate and register auth token (unless no_auth mode)
        if cfg.server.no_auth {
            eprintln!("[AUTH] Authentication disabled (no_auth=true in config)");
        } else {
            let token = generate_auth_token();
            eprintln!("[AUTH] Dashboard token: {}...{}", &token[..4], &token[token.len()-4..]);
            init_auth_token(token);
        }
    }
    
    // Initialize logging — write to file since windows_subsystem="windows" has no console
    let log_dir = crate::core::config::AppConfig::data_dir();
    let file_appender = tracing_appender::rolling::never(&log_dir, "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    // _guard must stay alive for the duration of the program
    let _log_guard = _guard;
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("RUST_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();
    tracing::info!("WebView Bridge Server v2 starting...");

    // Create SessionManager
    let manager = Arc::new(SessionManager::new(20));
    let manager_for_v2 = manager.clone();

    // Initialize v2 session manager using the unified data directory
    let data_dir = crate::core::config::AppConfig::data_dir();
    
    // Ensure data directory exists
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::warn!("Failed to create data directory {:?}: {}", data_dir, e);
    }
    tracing::info!("Data directory: {:?}", data_dir);
    
    init_session_manager_v2(data_dir, 20);
    crate::api_v2::set_core_session_manager(manager.clone());

    // Create command channels
    let (cmd_tx, cmd_rx) = mpsc::channel::<AppCommand>(1000);
    let cmd_rx = Arc::new(tokio::sync::Mutex::new(cmd_rx));

    // Spawn command processors
    for i in 0..COMMAND_PROCESSOR_COUNT {
        let manager_clone = manager.clone();
        let cmd_rx_clone = cmd_rx.clone();
        tokio::spawn(async move {
            tracing::debug!("Command processor {} started", i);
            loop {
                let cmd = {
                    let mut rx = cmd_rx_clone.lock().await;
                    rx.recv().await
                };
                match cmd {
                    Some(cmd) => process_command_async(cmd, &manager_clone).await,
                    None => break,
                }
            }
        });
    }

    // Convert to unbounded for API
    let (unbounded_tx, mut unbounded_rx) = mpsc::unbounded_channel::<AppCommand>();
    tokio::spawn(async move {
        while let Some(cmd) = unbounded_rx.recv().await {
            if cmd_tx.send(cmd).await.is_err() {
                tracing::error!("Failed to forward command");
                break;
            }
        }
    });

    // Create v2 API state
    let v2_state = V2AppState {
        create_session_fn: Arc::new(move |options: SessionOptions| -> Result<(String, crate::core::session_v2::SessionHandle), String> {
            let id = manager_for_v2.create_session(options.clone())?;
            let handle = crate::core::session_v2::SessionHandle { id: id.clone() };
            Ok((id, handle))
        }),
        cmd_tx: unbounded_tx,
    };

    // Clone cmd_tx before v2_state is moved into router
    let cmd_tx_cleanup = v2_state.cmd_tx.clone();

    // Create routers
    let event_hub = Arc::new(crate::core::websocket::EventHub::new());
    let ws_router = crate::core::websocket::create_ws_router(event_hub.clone());
    let v2_router = create_v2_router(v2_state);
    let app = v2_router.nest("/ws", ws_router);

    // Run server
    tracing::info!("listening on {} with {} command processors", addr, COMMAND_PROCESSOR_COUNT);
    tracing::info!("API: http://{}/", addr);
    tracing::info!("MCP: webview-bridge-rust.exe --mcp-stdio (requires server running)");

    // Start auto-cleanup timer (suspends idle sessions + cleans expired)
    let idle_timeout_secs = 300u64; // 5 minutes
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute
        loop {
            interval.tick().await;
            let manager = crate::api_v2::get_session_manager_v2();
            
            // 1. Suspend idle sessions and close their WebView windows
            let suspended = manager.auto_suspend_idle(idle_timeout_secs);
            for (session_name, session_id) in &suspended {
                // Close the actual WebView window
                let (tx, _rx) = tokio::sync::oneshot::channel();
                let _ = cmd_tx_cleanup.send(crate::core::AppCommand::CloseSession {
                    id: session_id.clone(),
                    resp_tx: tx,
                });
                tracing::info!("[AutoCleanup] Closed WebView for suspended session '{}' (id={})", session_name, session_id);
            }
            if !suspended.is_empty() {
                tracing::info!("[AutoCleanup] Suspended {} idle sessions", suspended.len());
            }
            
            // 2. Cleanup TTL-expired sessions
            if let Ok(expired_ids) = manager.cleanup_expired() {
                for session_id in &expired_ids {
                    // Close the WebView window
                    let (tx, _rx) = tokio::sync::oneshot::channel();
                    let _ = cmd_tx_cleanup.send(crate::core::AppCommand::CloseSession {
                        id: session_id.clone(),
                        resp_tx: tx,
                    });
                }
                if !expired_ids.is_empty() {
                    tracing::info!("[AutoCleanup] Cleaned up {} expired sessions", expired_ids.len());
                }
            }
        }
    });

    // Graceful shutdown: wait for the tray "Exit" signal.
    let shutdown_signal = async move {
        tokio::task::spawn_blocking(move || {
            let _ = shutdown_rx.recv(); // blocks until tray sends or channel drops
        })
        .await
        .ok();
    };

    match axum::serve(
        tokio::net::TcpListener::bind(&addr).await.unwrap(),
        app,
    )
    .with_graceful_shutdown(shutdown_signal)
    .await
    {
        Ok(_) => tracing::info!("Server shut down gracefully"),
        Err(e) => tracing::error!("Server error: {:?}", e),
    }
}

async fn process_command_async(cmd: AppCommand, manager: &Arc<SessionManager>) {
    match cmd {
        AppCommand::CreateSession { options, resp_tx } => {
            let result = manager.create_session(options);
            let _ = resp_tx.send(result);
        }
        AppCommand::Navigate { id, url, resp_tx } => {
            let result = manager.navigate(&id, url).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::GetStatus { id, resp_tx } => {
            let result = manager.get_info(&id);
            let _ = resp_tx.send(result);
        }
        AppCommand::ExecuteScript { id, script, resp_tx } => {
            let result = manager.execute_script(&id, script).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::CloseSession { id, resp_tx } => {
            let result = manager.remove_session(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::Snapshot { id, format, resp_tx } => {
            let result = manager.snapshot(&id, format).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::Screenshot { id, resp_tx } => {
            let result = manager.screenshot(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::GetCookies { id, resp_tx } => {
            let result = manager.get_cookies(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SetCookies { id, cookies, resp_tx } => {
            let result = manager.set_cookies(&id, cookies).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::WaitForSelector { id, selector, timeout_ms, frame, resp_tx } => {
            let result = manager.wait_for_selector(&id, selector, timeout_ms, frame).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SetVisibility { id, visible, resp_tx } => {
            let result = manager.set_visibility(&id, visible).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SimulateDevice { id, device_name, resp_tx } => {
            let result = manager.simulate_device(&id, device_name).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SetViewport { id, width, height, resp_tx } => {
            let result = manager.set_viewport(&id, width, height).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SetUserAgent { id, user_agent, resp_tx } => {
            let result = manager.set_user_agent(&id, user_agent).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::ScreenshotCdp { id, full_page, format, quality, frame, resp_tx } => {
            let result = manager.screenshot_cdp(&id, full_page, &format, quality, frame).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::ResetDeviceEmulation { id, resp_tx } => {
            let result = manager.reset_device_emulation(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::SetViewportCdp { id, width, height, device_scale_factor, is_mobile, resp_tx } => {
            let result = manager.set_viewport_cdp(&id, width, height, device_scale_factor, is_mobile).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::ClickCdp { id, selector, human_mode, resp_tx } => {
            let result = manager.click_cdp(&id, selector, human_mode).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::TypeCdp { id, text, char_delay_ms, human_mode, resp_tx } => {
            let result = manager.type_cdp(&id, text, char_delay_ms, human_mode).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::PressKeyCdp { id, key, resp_tx } => {
            let result = manager.press_key_cdp(&id, key).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::ManageNetwork { id, action, resp_tx } => {
            let result = manager.manage_network(&id, action).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::GetFrames { id, resp_tx } => {
            let result = manager.get_frames(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::ExecuteInFrame { id, script, frame, resp_tx } => {
            let result = manager.execute_in_frame(&id, &script, &frame).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::FormInjectFile { id, selector, file_paths, frame, resp_tx } => {
            let result = manager.form_inject_file(&id, &selector, &file_paths, frame).await;
            let _ = resp_tx.send(result);
        }
    }
}
