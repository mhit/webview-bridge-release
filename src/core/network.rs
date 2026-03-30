use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Network Monitoring Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkAction {
    Enable { max_logs: Option<usize> },
    Disable,
    GetLogs { filter: Option<String> },
    ClearLogs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub timestamp: f64,
    pub post_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResponse {
    pub url: String,
    pub status: i32,
    pub headers: HashMap<String, String>,
    pub mime_type: String,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLogEntry {
    pub request_id: String,
    pub request: NetworkRequest,
    pub response: Option<NetworkResponse>,
}
