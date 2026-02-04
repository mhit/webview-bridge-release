pub mod api;
pub mod core;
pub mod webview;

use std::net::SocketAddr;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create command channel for main thread communication
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<core::AppCommand>();

    // Create API router
    let app = api::create_router(cmd_tx, 0); // TODO: Get actual main thread ID

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 9400));
    tracing::info!("listening on {}", addr);

    // Spawn command handler
    let cmd_handler = async move {
        while let Some(cmd) = cmd_rx.recv().await {
            // TODO: Implement command handling
            tracing::warn!("Received command but not implemented yet: {:?}", cmd);
        }
    };
    tokio::spawn(cmd_handler);

    // In a real Windows app, we'd need to handle the Win32 message loop here
    // or spawn this server in a background thread.
    axum::serve(tokio::net::TcpListener::bind(&addr).await.unwrap(), app)
        .await
        .unwrap();
}
