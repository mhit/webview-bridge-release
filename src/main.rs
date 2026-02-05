pub mod api;
pub mod core;
pub mod mcp;
pub mod webdriver;
pub mod webview;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

use crate::core::{AppCommand, SessionManager};

pub const WM_CHECK_QUEUE: u32 = WM_USER + 200;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    tracing::info!("WebView Bridge Server starting...");

    // Create SessionManager
    let manager = Arc::new(SessionManager::new(10));

    // Create command channels
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<AppCommand>();

    // Spawn command processor task
    let manager_clone = manager.clone();
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            process_command_async(cmd, &manager_clone).await;
        }
    });

    // Create API router
    let app = api::create_router(cmd_tx, 0);

    // Run server
    let addr = SocketAddr::from(([127, 0, 0, 1], 9400));
    tracing::info!("listening on {}", addr);

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
