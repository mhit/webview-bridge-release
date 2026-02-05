pub mod api;
pub mod api_v2;
pub mod cdp;
pub mod core;
pub mod mcp;
pub mod webdriver;
pub mod webview;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

use crate::core::{AppCommand, SessionManager, SessionHandle, SessionOptions};
use crate::api_v2::{create_v2_router, init_session_manager_v2, V2AppState};

pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

// Number of concurrent command processors
const COMMAND_PROCESSOR_COUNT: usize = 4;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    tracing::info!("WebView Bridge Server starting...");

    // Create SessionManager with higher capacity
    let manager = Arc::new(SessionManager::new(20));
    let manager_for_v2 = manager.clone();

    // Initialize v2 session manager
    let data_dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"));
    init_session_manager_v2(data_dir, 20);

    // Create command channels with bounded capacity for backpressure
    let (cmd_tx, cmd_rx) = mpsc::channel::<AppCommand>(1000);
    let cmd_rx = Arc::new(tokio::sync::Mutex::new(cmd_rx));

    // Spawn multiple command processor tasks for parallelism
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
                    Some(cmd) => {
                        process_command_async(cmd, &manager_clone).await;
                    }
                    None => break,
                }
            }
            tracing::debug!("Command processor {} stopped", i);
        });
    }

    // Convert to unbounded for API compatibility
    // (API uses unbounded, we convert to bounded internally)
    let (unbounded_tx, mut unbounded_rx) = mpsc::unbounded_channel::<AppCommand>();
    tokio::spawn(async move {
        while let Some(cmd) = unbounded_rx.recv().await {
            if cmd_tx.send(cmd).await.is_err() {
                tracing::error!("Failed to forward command to processor");
                break;
            }
        }
    });

    // Create v2 API state with session creation callback
    let v2_state = V2AppState {
        create_session_fn: Arc::new(move |options: SessionOptions| -> Result<(String, SessionHandle), String> {
            // Use the v1 session manager to create the actual session
            let _id = manager_for_v2.create_session(options.clone())?;
            // Return a dummy handle for now (the actual session runs in its own thread)
            // In production, we'd need to get the actual handle from the SessionManager
            Err("Session created but handle not yet available - use v1 API for operations".to_string())
        }),
    };

    // Create API router (v1 + v2)
    let v1_router = api::create_router(unbounded_tx, 0);
    let v2_router = create_v2_router(v2_state);
    
    let app = v1_router.nest("/v2", v2_router);

    // Run server
    let addr = SocketAddr::from(([127, 0, 0, 1], 9400));
    tracing::info!("listening on {} with {} command processors", addr, COMMAND_PROCESSOR_COUNT);
    tracing::info!("v1 API: http://{}/", addr);
    tracing::info!("v2 API: http://{}/v2/", addr);

    match axum::serve(
        tokio::net::TcpListener::bind(&addr).await.unwrap(),
        app,
    )
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
        AppCommand::ExecuteScript {
            id,
            script,
            request_id: _,
            resp_tx,
        } => {
            let result = manager.execute_script(&id, script).await;
            let _ = resp_tx.send(result.map(|s| serde_json::json!(s)));
        }
        AppCommand::CloseSession { id, resp_tx } => {
            let result = manager.remove_session(&id).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::Act {
            id,
            action,
            resp_tx,
        } => {
            let result = manager.act(&id, action).await;
            let _ = resp_tx.send(result);
        }
        AppCommand::Snapshot {
            id,
            format,
            resp_tx,
        } => {
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
        AppCommand::Extract { id, selector, attribute, extract_all, resp_tx } => {
            let result = manager.extract(&id, selector, attribute, extract_all).await;
            let _ = resp_tx.send(result);
        }
    }
}
