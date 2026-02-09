//! WBP2 WebSocket Module - Real-time event notification
//!
//! WebSocket server for DOM changes, navigation, and error events.
//! See: Plans.md Phase 8.3

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

// ============================================================================
// Event Types
// ============================================================================

/// Event types that can be sent over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WbpEvent {
    /// DOM mutation detected
    DomChange {
        session: String,
        selector: Option<String>,
        change_type: String, // added, removed, modified
        timestamp: String,
    },
    /// Navigation completed
    Navigation {
        session: String,
        url: String,
        status: String, // started, completed, failed
        timestamp: String,
    },
    /// Session state changed
    SessionState {
        session: String,
        state: String, // ready, busy, error, closed
        timestamp: String,
    },
    /// Error occurred
    Error {
        session: String,
        code: String,
        message: String,
        timestamp: String,
    },
    /// Wait condition met
    WaitComplete {
        session: String,
        selector: String,
        condition: String,
        success: bool,
        elapsed_ms: u64,
        timestamp: String,
    },
    /// Connection established (sent on connect)
    Connected {
        client_id: String,
        timestamp: String,
    },
    /// Heartbeat/ping
    Ping {
        timestamp: String,
    },
}

impl WbpEvent {
    pub fn timestamp() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:03}Z", now.as_secs(), now.subsec_millis())
    }
}

// ============================================================================
// Event Hub (Pub/Sub)
// ============================================================================

/// Manages event subscriptions per session
pub struct EventHub {
    /// Broadcast channel for all events
    global_tx: broadcast::Sender<WbpEvent>,
    /// Per-session channels
    session_channels: Arc<RwLock<HashMap<String, broadcast::Sender<WbpEvent>>>>,
    /// Connected clients
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id: String,
    pub subscribed_sessions: Vec<String>,
    pub connected_at: String,
}

impl EventHub {
    pub fn new() -> Self {
        let (global_tx, _) = broadcast::channel(1000);
        Self {
            global_tx,
            session_channels: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Subscribe to global events
    pub fn subscribe_global(&self) -> broadcast::Receiver<WbpEvent> {
        self.global_tx.subscribe()
    }
    
    /// Subscribe to session-specific events
    pub async fn subscribe_session(&self, session: &str) -> broadcast::Receiver<WbpEvent> {
        let mut channels = self.session_channels.write().await;
        
        if let Some(tx) = channels.get(session) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(100);
            channels.insert(session.to_string(), tx);
            rx
        }
    }
    
    /// Publish event globally
    pub fn publish(&self, event: WbpEvent) {
        let _ = self.global_tx.send(event);
    }
    
    /// Publish event to specific session
    pub async fn publish_to_session(&self, session: &str, event: WbpEvent) {
        // Clone for global publish
        let event_clone = event.clone();
        
        let channels = self.session_channels.read().await;
        if let Some(tx) = channels.get(session) {
            let _ = tx.send(event);
        }
        // Also send to global
        self.publish(event_clone);
    }
    
    /// Register a client
    pub async fn register_client(&self, client_id: &str, sessions: Vec<String>) {
        let mut clients = self.clients.write().await;
        clients.insert(client_id.to_string(), ClientInfo {
            id: client_id.to_string(),
            subscribed_sessions: sessions,
            connected_at: WbpEvent::timestamp(),
        });
    }
    
    /// Unregister a client
    pub async fn unregister_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(client_id);
    }
    
    /// Get client count
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WebSocket State
// ============================================================================

#[derive(Clone)]
pub struct WsState {
    pub event_hub: Arc<EventHub>,
}

// ============================================================================
// WebSocket Handler
// ============================================================================

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsState) {
    let client_id = uuid::Uuid::new_v4().to_string();
    
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to global events
    let mut event_rx = state.event_hub.subscribe_global();
    
    // Register client
    state.event_hub.register_client(&client_id, vec![]).await;
    
    // Send connected event
    let connected = WbpEvent::Connected {
        client_id: client_id.clone(),
        timestamp: WbpEvent::timestamp(),
    };
    
    if let Ok(msg) = serde_json::to_string(&connected) {
        let _ = sender.send(Message::Text(msg)).await;
    }
    
    // Spawn task to forward events to client
    let client_id_clone = client_id.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(msg) = serde_json::to_string(&event) {
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Handle incoming messages
    let event_hub = state.event_hub.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Handle subscription requests
                    if let Ok(cmd) = serde_json::from_str::<WsCommand>(&text) {
                        match cmd {
                            WsCommand::Subscribe { sessions } => {
                                // Update subscription
                                event_hub.register_client(&client_id_clone, sessions).await;
                            }
                            WsCommand::Ping => {
                                // Ping is handled by sending back via event channel
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
    
    // Cleanup
    state.event_hub.unregister_client(&client_id).await;
}

/// Commands that can be sent by the client
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum WsCommand {
    Subscribe { sessions: Vec<String> },
    Ping,
}

// ============================================================================
// WebSocket Router
// ============================================================================

/// Create WebSocket router
pub fn create_ws_router(event_hub: Arc<EventHub>) -> Router {
    let state = WsState { event_hub };
    
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_event_hub_subscribe() {
        let hub = EventHub::new();
        let mut rx = hub.subscribe_global();
        
        hub.publish(WbpEvent::Ping {
            timestamp: WbpEvent::timestamp(),
        });
        
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, WbpEvent::Ping { .. }));
    }
    
    #[tokio::test]
    async fn test_client_registration() {
        let hub = EventHub::new();
        
        hub.register_client("client1", vec!["session1".to_string()]).await;
        assert_eq!(hub.client_count().await, 1);
        
        hub.unregister_client("client1").await;
        assert_eq!(hub.client_count().await, 0);
    }
    
    #[test]
    fn test_event_hub_default() {
        let hub = EventHub::default();
        // Verify it doesn't panic
        let _ = hub.subscribe_global();
    }
    
    #[tokio::test]
    async fn test_session_subscribe() {
        let hub = EventHub::new();
        let mut rx = hub.subscribe_session("test_session").await;
        
        hub.publish_to_session("test_session", WbpEvent::Ping {
            timestamp: WbpEvent::timestamp(),
        }).await;
        
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, WbpEvent::Ping { .. }));
    }
    
    #[test]
    fn test_wbp_event_timestamp() {
        let ts = WbpEvent::timestamp();
        assert!(!ts.is_empty());
        // Format is like "1234567890.123Z"
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('.'));
    }
    
    #[test]
    fn test_wbp_event_dom_change() {
        let event = WbpEvent::DomChange {
            session: "test".to_string(),
            selector: Some(".container".to_string()),
            change_type: "added".to_string(),
            timestamp: WbpEvent::timestamp(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("dom_change"));
        assert!(json.contains("test"));
    }
    
    #[test]
    fn test_wbp_event_navigation() {
        let event = WbpEvent::Navigation {
            session: "main".to_string(),
            url: "https://example.com".to_string(),
            status: "completed".to_string(),
            timestamp: WbpEvent::timestamp(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("navigation"));
        assert!(json.contains("example.com"));
    }
    
    #[test]
    fn test_wbp_event_error() {
        let event = WbpEvent::Error {
            session: "test".to_string(),
            message: "Something went wrong".to_string(),
            code: "ERR001".to_string(),
            timestamp: WbpEvent::timestamp(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("Something went wrong"));
    }
    
    #[test]
    fn test_wbp_event_connected() {
        let event = WbpEvent::Connected {
            client_id: "abc123".to_string(),
            timestamp: WbpEvent::timestamp(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("connected"));
        assert!(json.contains("abc123"));
    }
    
    #[tokio::test]
    async fn test_multiple_clients() {
        let hub = EventHub::new();
        
        hub.register_client("client1", vec!["s1".to_string()]).await;
        hub.register_client("client2", vec!["s2".to_string()]).await;
        hub.register_client("client3", vec!["s1".to_string(), "s2".to_string()]).await;
        
        assert_eq!(hub.client_count().await, 3);
        
        hub.unregister_client("client2").await;
        assert_eq!(hub.client_count().await, 2);
    }
}

