//! WBP2 Communication Module
//!
//! Webhook notifications, batch requests, and async job management.
//! See: Plans.md Phase 16

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Job Management (16.1)
// ============================================================================

/// Job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Job type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Download,
    Extract,
    AiAnalysis,
    Screenshot,
    Batch,
    Custom(String),
}

/// Job information
#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    /// Progress 0-100
    pub percent: Option<u8>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<u32>,
    /// Result data
    pub result: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Webhook config for this job
    pub webhook: Option<WebhookConfig>,
}

/// Response for GET /v2/jobs/:id
#[derive(Debug, Clone, Serialize)]
pub struct JobResponse {
    pub success: bool,
    pub job: Job,
}

/// Job manager
#[derive(Debug, Clone, Default)]
pub struct JobManager {
    pub jobs: HashMap<String, Job>,
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a new job
    pub fn create_job(&mut self, job_type: JobType, webhook: Option<WebhookConfig>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        
        self.jobs.insert(id.clone(), Job {
            id: id.clone(),
            job_type,
            status: JobStatus::Pending,
            created_at: chrono_now_iso8601(),
            started_at: None,
            completed_at: None,
            percent: Some(0),
            eta_seconds: None,
            result: None,
            error: None,
            webhook,
        });
        
        id
    }
    
    /// Start a job
    pub fn start_job(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Running;
            job.started_at = Some(chrono_now_iso8601());
        }
    }
    
    /// Update job progress
    pub fn update_progress(&mut self, id: &str, percent: u8, eta_seconds: Option<u32>) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.percent = Some(percent);
            job.eta_seconds = eta_seconds;
        }
    }
    
    /// Complete a job
    pub fn complete_job(&mut self, id: &str, result: serde_json::Value) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.completed_at = Some(chrono_now_iso8601());
            job.percent = Some(100);
            job.eta_seconds = None;
            job.result = Some(result);
        }
    }
    
    /// Fail a job
    pub fn fail_job(&mut self, id: &str, error: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Failed;
            job.completed_at = Some(chrono_now_iso8601());
            job.error = Some(error.to_string());
        }
    }
    
    /// Cancel a job
    pub fn cancel_job(&mut self, id: &str) -> bool {
        if let Some(job) = self.jobs.get_mut(id) {
            if job.status == JobStatus::Pending || job.status == JobStatus::Running {
                job.status = JobStatus::Cancelled;
                job.completed_at = Some(chrono_now_iso8601());
                return true;
            }
        }
        false
    }
    
    /// Get a job
    pub fn get_job(&self, id: &str) -> Option<&Job> {
        self.jobs.get(id)
    }
    
    /// List jobs with optional status filter
    pub fn list_jobs(&self, status: Option<JobStatus>) -> Vec<&Job> {
        self.jobs.values()
            .filter(|j| status.as_ref().map(|s| &j.status == s).unwrap_or(true))
            .collect()
    }
}

// ============================================================================
// Webhook Notifications (16.2)
// ============================================================================

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Target URL
    pub url: String,
    
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    
    /// Events to notify
    #[serde(default)]
    pub events: Vec<WebhookEvent>,
    
    /// Secret for signature
    #[serde(default)]
    pub secret: Option<String>,
    
    /// Retry configuration
    #[serde(default)]
    pub retry: WebhookRetry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRetry {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,
}

impl Default for WebhookRetry {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            backoff_ms: default_backoff_ms(),
        }
    }
}

fn default_max_attempts() -> u32 {
    3
}

fn default_backoff_ms() -> u64 {
    1000
}

/// Webhook events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    JobCompleted,
    JobFailed,
    DownloadCompleted,
    DownloadFailed,
    SessionCreated,
    SessionDestroyed,
    All,
}

/// Webhook payload
#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    pub event: String,
    pub timestamp: String,
    pub data: serde_json::Value,
    /// HMAC-SHA256 signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl WebhookPayload {
    pub fn new(event: &str, data: serde_json::Value) -> Self {
        Self {
            event: event.to_string(),
            timestamp: chrono_now_iso8601(),
            data,
            signature: None,
        }
    }
    
    /// Add signature using secret
    pub fn sign(&mut self, secret: &str) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        format!("{}{}{}", self.event, self.timestamp, secret).hash(&mut hasher);
        self.signature = Some(format!("sha256={:x}", hasher.finish()));
    }
}

// ============================================================================
// Batch Requests (16.3)
// ============================================================================

/// Request for POST /v2/batch
#[derive(Debug, Clone, Deserialize)]
pub struct BatchRequest {
    /// List of operations
    pub operations: Vec<BatchOperation>,
    
    /// Stop on first error
    #[serde(default)]
    pub stop_on_error: bool,
    
    /// Execute in parallel
    #[serde(default)]
    pub parallel: bool,
    
    /// Webhook for batch completion
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchOperation {
    /// Operation ID (for depends_on)
    pub id: String,
    
    /// HTTP method
    pub method: String,
    
    /// Path (e.g., "/v2/screenshot")
    pub path: String,
    
    /// Request body
    #[serde(default)]
    pub body: serde_json::Value,
    
    /// Dependencies (operation IDs)
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Response for batch request
#[derive(Debug, Clone, Serialize)]
pub struct BatchResponse {
    pub success: bool,
    pub job_id: String,
    pub results: Vec<BatchOperationResult>,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationResult {
    pub id: String,
    pub success: bool,
    pub status_code: u16,
    pub response: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// WebSocket Events (16.4)
// ============================================================================

/// WebSocket subscription request
#[derive(Debug, Clone, Deserialize)]
pub struct EventSubscription {
    /// Action: subscribe/unsubscribe
    pub action: String,
    
    /// Event patterns to subscribe to
    #[serde(default)]
    pub events: Vec<String>,
    
    /// Filter by session
    #[serde(default)]
    pub session_filter: Option<String>,
}

/// WebSocket event message
#[derive(Debug, Clone, Serialize)]
pub struct EventMessage {
    #[serde(rename = "type")]
    pub event_type: String,
    pub session: Option<String>,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

impl EventMessage {
    pub fn new(event_type: &str, session: Option<String>, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            session,
            timestamp: chrono_now_iso8601(),
            payload,
        }
    }
}

/// Event types
pub mod event_types {
    pub const DOWNLOAD_STARTED: &str = "download_started";
    pub const DOWNLOAD_PROGRESS: &str = "download_progress";
    pub const DOWNLOAD_COMPLETED: &str = "download_completed";
    pub const DOWNLOAD_FAILED: &str = "download_failed";
    
    pub const JOB_STARTED: &str = "job_started";
    pub const JOB_PROGRESS: &str = "job_progress";
    pub const JOB_COMPLETED: &str = "job_completed";
    pub const JOB_FAILED: &str = "job_failed";
    pub const JOB_CANCELLED: &str = "job_cancelled";
    
    pub const SESSION_ACQUIRED: &str = "session_acquired";
    pub const SESSION_RELEASED: &str = "session_released";
    pub const SESSION_DESTROYED: &str = "session_destroyed";
    
    pub const DOM_CHANGE: &str = "dom_change";
    pub const NAVIGATION: &str = "navigation";
    pub const PAGE_LOAD: &str = "page_load";
    pub const ERROR: &str = "error";
}

// ============================================================================
// Error Codes (16.5)
// ============================================================================

/// WBP2 Error code registry
pub mod error_codes {
    // Session errors (001-009)
    pub const SESSION_NOT_FOUND: &str = "WBP2_001";
    pub const SESSION_BUSY: &str = "WBP2_002";
    pub const SESSION_TIMEOUT: &str = "WBP2_003";
    pub const SESSION_LIMIT: &str = "WBP2_004";
    
    // Element errors (010-019)
    pub const ELEMENT_NOT_FOUND: &str = "WBP2_010";
    pub const WAIT_TIMEOUT: &str = "WBP2_011";
    pub const ELEMENT_NOT_VISIBLE: &str = "WBP2_012";
    pub const ELEMENT_NOT_INTERACTABLE: &str = "WBP2_013";
    
    // Script errors (020-029)
    pub const SCRIPT_ERROR: &str = "WBP2_020";
    pub const SCRIPT_TIMEOUT: &str = "WBP2_021";
    
    // Navigation errors (030-039)
    pub const NAVIGATION_FAILED: &str = "WBP2_030";
    pub const PAGE_LOAD_TIMEOUT: &str = "WBP2_031";
    
    // Macro errors (080-089)
    pub const MACRO_NOT_FOUND: &str = "WBP2_080";
    pub const MACRO_EXECUTION_FAILED: &str = "WBP2_081";
    
    // Request errors (090-099)
    pub const INVALID_REQUEST: &str = "WBP2_090";
    pub const VALIDATION_ERROR: &str = "WBP2_091";
    pub const INTERNAL_ERROR: &str = "WBP2_099";
    
    // AI errors (100-109)
    pub const AI_NOT_AVAILABLE: &str = "WBP2_100";
    pub const AI_BUDGET_EXCEEDED: &str = "WBP2_101";
    pub const AI_REQUEST_FAILED: &str = "WBP2_102";
    
    // Download errors (110-119)
    pub const DOWNLOAD_NOT_FOUND: &str = "WBP2_110";
    pub const FILE_REF_NOT_FOUND: &str = "WBP2_111";
    pub const DOWNLOAD_FAILED: &str = "WBP2_112";
    pub const STORAGE_FULL: &str = "WBP2_113";
    
    // Job errors (120-129)
    pub const JOB_NOT_FOUND: &str = "WBP2_120";
    pub const JOB_ALREADY_COMPLETED: &str = "WBP2_121";
    pub const JOB_CANCELLED: &str = "WBP2_122";
    
    // Webhook errors (130-139)
    pub const WEBHOOK_FAILED: &str = "WBP2_130";
    pub const WEBHOOK_TIMEOUT: &str = "WBP2_131";
    
    // Batch errors (140-149)
    pub const BATCH_FAILED: &str = "WBP2_140";
    pub const DEPENDENCY_FAILED: &str = "WBP2_141";
}

// ============================================================================
// Helper Functions
// ============================================================================

fn chrono_now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_job_lifecycle() {
        let mut manager = JobManager::new();
        
        let id = manager.create_job(JobType::Download, None);
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Pending);
        
        manager.start_job(&id);
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Running);
        
        manager.update_progress(&id, 50, Some(30));
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.percent, Some(50));
        
        manager.complete_job(&id, serde_json::json!({"file": "test.zip"}));
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.percent, Some(100));
    }
    
    #[test]
    fn test_job_fail() {
        let mut manager = JobManager::new();
        
        let id = manager.create_job(JobType::AiAnalysis, None);
        manager.start_job(&id);
        manager.fail_job(&id, "API error");
        
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error, Some("API error".to_string()));
    }
    
    #[test]
    fn test_job_cancel() {
        let mut manager = JobManager::new();
        
        let id = manager.create_job(JobType::Extract, None);
        manager.start_job(&id);
        
        assert!(manager.cancel_job(&id));
        let job = manager.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }
    
    #[test]
    fn test_job_cancel_completed_fails() {
        let mut manager = JobManager::new();
        
        let id = manager.create_job(JobType::Screenshot, None);
        manager.start_job(&id);
        manager.complete_job(&id, serde_json::json!({}));
        
        // Cannot cancel completed job
        assert!(!manager.cancel_job(&id));
    }
    
    #[test]
    fn test_job_list_all() {
        let mut manager = JobManager::new();
        
        manager.create_job(JobType::Download, None);
        manager.create_job(JobType::Extract, None);
        manager.create_job(JobType::AiAnalysis, None);
        
        let all_jobs = manager.list_jobs(None);
        assert_eq!(all_jobs.len(), 3);
    }
    
    #[test]
    fn test_job_list_with_filter() {
        let mut manager = JobManager::new();
        
        let id1 = manager.create_job(JobType::Download, None);
        let id2 = manager.create_job(JobType::Extract, None);
        manager.create_job(JobType::AiAnalysis, None);
        
        manager.start_job(&id1);
        manager.start_job(&id2);
        manager.complete_job(&id1, serde_json::json!({}));
        
        let running = manager.list_jobs(Some(JobStatus::Running));
        assert_eq!(running.len(), 1);
        
        let completed = manager.list_jobs(Some(JobStatus::Completed));
        assert_eq!(completed.len(), 1);
        
        let pending = manager.list_jobs(Some(JobStatus::Pending));
        assert_eq!(pending.len(), 1);
    }
    
    #[test]
    fn test_job_get_not_found() {
        let manager = JobManager::new();
        assert!(manager.get_job("nonexistent").is_none());
    }
    
    #[test]
    fn test_job_type_serialization() {
        let job_type = JobType::Download;
        let json = serde_json::to_string(&job_type).unwrap();
        assert!(json.contains("download"));
        
        let custom = JobType::Custom("my_job".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        assert!(json.contains("my_job"));
    }
    
    #[test]
    fn test_webhook_config_defaults() {
        let config: WebhookConfig = serde_json::from_str(r#"{"url": "https://example.com/hook"}"#).unwrap();
        assert_eq!(config.url, "https://example.com/hook");
        assert!(config.headers.is_empty());
        assert!(config.events.is_empty());
        assert!(config.secret.is_none());
        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.retry.backoff_ms, 1000);
    }
    
    #[test]
    fn test_webhook_payload() {
        let mut payload = WebhookPayload::new(
            "job_completed",
            serde_json::json!({"job_id": "123"})
        );
        
        payload.sign("my_secret");
        assert!(payload.signature.is_some());
        assert!(payload.signature.unwrap().starts_with("sha256="));
    }
    
    #[test]
    fn test_webhook_payload_consistent_signature() {
        let mut payload1 = WebhookPayload {
            event: "test".to_string(),
            timestamp: "12345".to_string(),
            data: serde_json::json!({}),
            signature: None,
        };
        
        let mut payload2 = WebhookPayload {
            event: "test".to_string(),
            timestamp: "12345".to_string(),
            data: serde_json::json!({}),
            signature: None,
        };
        
        payload1.sign("secret");
        payload2.sign("secret");
        
        assert_eq!(payload1.signature, payload2.signature);
    }
    
    #[test]
    fn test_event_message() {
        let msg = EventMessage::new(
            event_types::DOWNLOAD_COMPLETED,
            Some("main".to_string()),
            serde_json::json!({"file": "test.zip"})
        );
        
        assert_eq!(msg.event_type, "download_completed");
        assert_eq!(msg.session, Some("main".to_string()));
    }
    
    #[test]
    fn test_event_message_no_session() {
        let msg = EventMessage::new(
            event_types::ERROR,
            None,
            serde_json::json!({"message": "test error"})
        );
        
        assert_eq!(msg.event_type, "error");
        assert!(msg.session.is_none());
    }
    
    #[test]
    fn test_event_types() {
        assert_eq!(event_types::DOWNLOAD_STARTED, "download_started");
        assert_eq!(event_types::JOB_COMPLETED, "job_completed");
        assert_eq!(event_types::SESSION_ACQUIRED, "session_acquired");
        assert_eq!(event_types::DOM_CHANGE, "dom_change");
    }
    
    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::SESSION_NOT_FOUND, "WBP2_001");
        assert_eq!(error_codes::AI_NOT_AVAILABLE, "WBP2_100");
        assert_eq!(error_codes::DOWNLOAD_NOT_FOUND, "WBP2_110");
        assert_eq!(error_codes::JOB_NOT_FOUND, "WBP2_120");
        assert_eq!(error_codes::INTERNAL_ERROR, "WBP2_099");
    }
    
    #[test]
    fn test_batch_operation() {
        let batch = BatchRequest {
            operations: vec![
                BatchOperation {
                    id: "op1".to_string(),
                    method: "POST".to_string(),
                    path: "/v2/screenshot".to_string(),
                    body: serde_json::json!({"session": "main"}),
                    depends_on: vec![],
                },
                BatchOperation {
                    id: "op2".to_string(),
                    method: "POST".to_string(),
                    path: "/v2/download/trigger".to_string(),
                    body: serde_json::json!({"session": "main", "url": "https://example.com"}),
                    depends_on: vec!["op1".to_string()],
                },
            ],
            stop_on_error: true,
            parallel: false,
            webhook: None,
        };
        
        assert_eq!(batch.operations.len(), 2);
        assert_eq!(batch.operations[1].depends_on, vec!["op1"]);
    }
    
    #[test]
    fn test_batch_operation_result() {
        let result = BatchOperationResult {
            id: "op1".to_string(),
            success: true,
            status_code: 200,
            response: serde_json::json!({"data": "test"}),
            error: None,
        };
        
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("op1"));
        assert!(json.contains("200"));
        assert!(!json.contains("error"));  // None should be skipped
    }
    
    #[test]
    fn test_event_subscription() {
        let sub: EventSubscription = serde_json::from_str(r#"{
            "action": "subscribe",
            "events": ["download_completed", "job_failed"],
            "session_filter": "main"
        }"#).unwrap();
        
        assert_eq!(sub.action, "subscribe");
        assert_eq!(sub.events.len(), 2);
        assert_eq!(sub.session_filter, Some("main".to_string()));
    }
    
    #[test]
    fn test_webhook_event_serialization() {
        let event = WebhookEvent::JobCompleted;
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("job_completed"));
        
        let event = WebhookEvent::All;
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("all"));
    }
}
