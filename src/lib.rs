//! WebView Bridge Library
//!
//! This library provides the core functionality for the WebView Bridge server.
//!
//! Protocol: WBP2 (WebView Bridge Protocol v2)

#[cfg(feature = "server")]
pub mod api_v2;
#[cfg(feature = "server")]
pub mod auto_login;
#[cfg(feature = "server")]
pub mod core;
#[cfg(feature = "server")]
pub mod mcp_v3;
#[cfg(feature = "server")]
pub mod tray;
#[cfg(feature = "server")]
pub mod webview;
