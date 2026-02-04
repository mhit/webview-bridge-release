pub mod api;
pub mod core;
pub mod webview;

use std::net::SocketAddr;
use tokio::sync::mpsc;
use core::SessionManager;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create SessionManager
    let manager = SessionManager::new(10);
    let manager = std::sync::Arc::new(manager);

    // Create command channel for main thread communication
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<core::AppCommand>();

    // Create API router
    let app = api::create_router(cmd_tx, 0); // TODO: Get actual main thread ID

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 9400));
    tracing::info!("listening on {}", addr);

    // Spawn command handler
    let manager_clone = manager.clone();
    let cmd_handler = async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                core::AppCommand::CreateSession { options, resp_tx } => {
                    let result = manager_clone.create_session(options);
                    let _ = resp_tx.send(result);
                }
                core::AppCommand::Navigate { id, url, resp_tx } => {
                    let result = manager_clone.navigate(&id, url);
                    let _ = resp_tx.send(result);
                }
                core::AppCommand::GetStatus { id, resp_tx } => {
                    let result = manager_clone.get_info(&id);
                    let _ = resp_tx.send(result);
                }
                core::AppCommand::ExecuteScript { id, script, request_id: _, resp_tx } => {
                    let result = manager_clone.execute_script(&id, script);
                    let _ = resp_tx.send(result.map(|s| serde_json::json!(s)));
                }
                core::AppCommand::CloseSession { id, resp_tx } => {
                    let result = manager_clone.remove_session(&id);
                    let _ = resp_tx.send(result);
                }
                core::AppCommand::Act { id, action, resp_tx } => {
                    let result = manager_clone.act(&id, action);
                    let _ = resp_tx.send(result);
                }
            }
        }
    };
    tokio::spawn(cmd_handler);

    // In a real Windows app, we'd need to handle the Win32 message loop here
    // or spawn this server in a background thread.
    axum::serve(tokio::net::TcpListener::bind(&addr).await.unwrap(), app)
        .await
        .unwrap();
}
