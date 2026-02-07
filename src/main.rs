pub mod api_v2;
pub mod core;
pub mod mcp;
pub mod mcp_stdio;
pub mod webview;

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

use crate::core::{AppCommand, SessionManager, SessionOptions};
use crate::api_v2::{create_v2_router, init_session_manager_v2, V2AppState};

pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

const COMMAND_PROCESSOR_COUNT: usize = 4;

/// Command line configuration
struct Config {
    bind: IpAddr,
    port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".parse().unwrap(),
            port: 9400,
        }
    }
}

fn parse_args() -> Option<Config> {
    let args: Vec<String> = std::env::args().collect();
    let mut config = Config::default();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("WebView Bridge Server v2");
                println!();
                println!("Usage: webview-bridge-rust.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --bind <IP>       Bind address (default: 0.0.0.0)");
                println!("  --port <PORT>     Port number (default: 9400)");
                println!("  --mcp-stdio       Run in MCP stdio proxy mode");
                println!("  --help, -h        Show this help message");
                return None;
            }
            "--bind" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.bind = args[i].parse().unwrap_or_else(|_| {
                        eprintln!("Invalid bind address: {}", args[i]);
                        std::process::exit(1);
                    });
                }
            }
            "--port" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.port = args[i].parse().unwrap_or_else(|_| {
                        eprintln!("Invalid port number: {}", args[i]);
                        std::process::exit(1);
                    });
                }
            }
            "--mcp-stdio" => {
                // Handled separately
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
    // Check for MCP stdio mode BEFORE tokio runtime
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--mcp-stdio".to_string()) {
        // MCP stdio mode: synchronous, connects to HTTP server
        eprintln!("WebView Bridge MCP proxy starting (connecting to http://127.0.0.1:9400)...");
        mcp_stdio::run_mcp_stdio_proxy();
        return;
    }
    
    // Parse command line arguments
    let config = match parse_args() {
        Some(c) => c,
        None => return, // --help was shown
    };
    
    // Normal HTTP server mode with tokio
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_http_server(config));
}

async fn run_http_server(config: Config) {
    // Initialize config file system first
    let app_config = crate::core::config::init_config();
    {
        let cfg = app_config.read().unwrap();
        eprintln!("Config loaded: AI enabled={}, provider={}, model={}", 
            cfg.ai.enabled, cfg.ai.provider, cfg.ai.model);
        if cfg.ai.api_key.is_some() {
            eprintln!("AI API key: configured from file");
        }
    }
    
    // Initialize logging
    tracing_subscriber::fmt::init();
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

    // Create routers
    let event_hub = Arc::new(crate::core::websocket::EventHub::new());
    let ws_router = crate::core::websocket::create_ws_router(event_hub.clone());
    let v2_router = create_v2_router(v2_state);
    let app = v2_router.nest("/ws", ws_router);

    // Run server
    let addr = SocketAddr::new(config.bind, config.port);
    tracing::info!("listening on {} with {} command processors", addr, COMMAND_PROCESSOR_COUNT);
    tracing::info!("API: http://{}/", addr);
    tracing::info!("MCP: webview-bridge-rust.exe --mcp-stdio (requires server running)");

    match axum::serve(
        tokio::net::TcpListener::bind(&addr).await.unwrap(),
        app,
    ).await {
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
        AppCommand::WaitForSelector { id, selector, timeout_ms, resp_tx } => {
            let result = manager.wait_for_selector(&id, selector, timeout_ms).await;
            let _ = resp_tx.send(result);
        }
    }
}
