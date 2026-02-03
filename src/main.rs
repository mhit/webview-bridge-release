pub mod core;
use std::net::SocketAddr;
use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Build our application with a single route
    let app = Router::new().route("/health", get(health_check));

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 9400));
    tracing::info!("listening on {}", addr);
    
    // In a real Windows app, we'd need to handle the Win32 message loop here
    // or spawn this server in a background thread.
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
