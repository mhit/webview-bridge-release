//! WBP2 Download & Storage Management Module
//!
//! Browser download handling, file storage, and lifecycle management.
//! See: Plans.md Phase 15

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Download Types (15.1)
// ============================================================================

/// Request for POST /v2/download/trigger
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadTriggerRequest {
    /// Session name
    pub session: String,
    
    /// URL to download
    pub url: String,
    
    /// Optional custom filename
    #[serde(default)]
    pub filename: Option<String>,
    
    /// Target directory (relative to storage root)
    #[serde(default = "default_download_dir")]
    pub directory: String,
    
    /// Wait for completion
    #[serde(default)]
    pub wait: bool,
    
    /// Timeout in ms (if wait=true)
    #[serde(default = "default_download_timeout")]
    pub timeout_ms: u64,
}

fn default_download_dir() -> String {
    "downloads".to_string()
}

fn default_download_timeout() -> u64 {
    300000 // 5 minutes
}

/// Response for download trigger
#[derive(Debug, Clone, Serialize)]
pub struct DownloadTriggerResponse {
    pub success: bool,
    pub download_id: String,
    pub status: DownloadStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

/// Download status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// Waiting to start
    Pending,
    /// Download in progress
    InProgress,
    /// Download completed
    Completed,
    /// Download failed
    Failed,
    /// Download cancelled
    Cancelled,
}

/// Download progress info
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub download_id: String,
    pub status: DownloadStatus,
    pub url: String,
    pub filename: Option<String>,
    /// Bytes downloaded
    pub bytes_received: u64,
    /// Total bytes (if known)
    pub total_bytes: Option<u64>,
    /// Progress percentage
    pub percent: Option<f32>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<u32>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Request for POST /v2/download/batch
#[derive(Debug, Clone, Deserialize)]
pub struct BatchDownloadRequest {
    /// Session name
    pub session: String,
    
    /// URLs to download
    pub urls: Vec<String>,
    
    /// Parallel downloads
    #[serde(default = "default_parallel")]
    pub parallel: u32,
    
    /// Target directory
    #[serde(default = "default_download_dir")]
    pub directory: String,
}

fn default_parallel() -> u32 {
    3
}

/// Response for batch download
#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadResponse {
    pub success: bool,
    pub batch_id: String,
    pub download_ids: Vec<String>,
    pub total: usize,
}

// ============================================================================
// Storage Types (15.3)
// ============================================================================

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Root data path
    #[serde(default = "default_data_path")]
    pub data_path: PathBuf,
    
    /// Maximum total storage in bytes
    #[serde(default = "default_max_storage")]
    pub max_storage_bytes: u64,
    
    /// Maximum file size in bytes
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
    
    /// Default TTL in seconds
    #[serde(default = "default_ttl")]
    pub default_ttl_seconds: u64,
    
    /// Warning threshold (0.0-1.0)
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f32,
    
    /// Critical threshold (0.0-1.0)
    #[serde(default = "default_critical_threshold")]
    pub critical_threshold: f32,
}

fn default_data_path() -> PathBuf {
    std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("webview-bridge")
        })
}

fn default_max_storage() -> u64 {
    10 * 1024 * 1024 * 1024 // 10 GB
}

fn default_max_file_size() -> u64 {
    1024 * 1024 * 1024 // 1 GB
}

fn default_ttl() -> u64 {
    24 * 60 * 60 // 24 hours
}

fn default_warning_threshold() -> f32 {
    0.80
}

fn default_critical_threshold() -> f32 {
    0.95
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_path: default_data_path(),
            max_storage_bytes: default_max_storage(),
            max_file_size_bytes: default_max_file_size(),
            default_ttl_seconds: default_ttl(),
            warning_threshold: default_warning_threshold(),
            critical_threshold: default_critical_threshold(),
        }
    }
}

/// Storage status response
#[derive(Debug, Clone, Serialize)]
pub struct StorageStatus {
    pub data_path: String,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub usage_percent: f32,
    pub alert_level: AlertLevel,
    pub directories: HashMap<String, DirectoryInfo>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryInfo {
    pub path: String,
    pub file_count: u32,
    pub size_bytes: u64,
}

/// Request for POST /v2/storage/cleanup
#[derive(Debug, Clone, Deserialize)]
pub struct CleanupRequest {
    /// Only cleanup expired files
    #[serde(default = "default_true")]
    pub expired_only: bool,
    
    /// Directories to clean
    #[serde(default)]
    pub directories: Vec<String>,
    
    /// Dry run (report only)
    #[serde(default)]
    pub dry_run: bool,
}

fn default_true() -> bool {
    true
}

/// Response for cleanup
#[derive(Debug, Clone, Serialize)]
pub struct CleanupResponse {
    pub success: bool,
    pub files_deleted: u32,
    pub bytes_freed: u64,
    pub dry_run: bool,
    pub details: Vec<CleanupDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupDetail {
    pub path: String,
    pub reason: String,
    pub size: u64,
}

// ============================================================================
// File Lifecycle (15.4)
// ============================================================================

/// File reference for grouping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub files: Vec<FileInfo>,
    pub total_size: u64,
    pub persistent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub mime_type: String,
    pub created_at: String,
}

/// Request for POST /v2/media/persist
#[derive(Debug, Clone, Deserialize)]
pub struct PersistRequest {
    /// File reference ID
    pub file_ref: String,
    
    /// Optional new location
    #[serde(default)]
    pub destination: Option<String>,
}

/// Request for POST /v2/media/extend
#[derive(Debug, Clone, Deserialize)]
pub struct ExtendTtlRequest {
    /// File reference ID
    pub file_ref: String,
    
    /// Additional TTL in seconds
    #[serde(default = "default_ttl")]
    pub additional_seconds: u64,
}

// ============================================================================
// Download Manager
// ============================================================================

/// Download manager for tracking downloads
#[derive(Debug, Clone, Default)]
pub struct DownloadManager {
    pub downloads: HashMap<String, DownloadProgress>,
    pub batches: HashMap<String, Vec<String>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Start tracking a download
    pub fn start_download(&mut self, url: &str, filename: Option<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        
        self.downloads.insert(id.clone(), DownloadProgress {
            download_id: id.clone(),
            status: DownloadStatus::Pending,
            url: url.to_string(),
            filename,
            bytes_received: 0,
            total_bytes: None,
            percent: None,
            eta_seconds: None,
            error: None,
        });
        
        id
    }
    
    /// Update download progress
    pub fn update_progress(&mut self, id: &str, bytes_received: u64, total_bytes: Option<u64>) {
        if let Some(download) = self.downloads.get_mut(id) {
            download.bytes_received = bytes_received;
            download.total_bytes = total_bytes;
            download.status = DownloadStatus::InProgress;
            
            if let Some(total) = total_bytes {
                if total > 0 {
                    download.percent = Some((bytes_received as f32 / total as f32) * 100.0);
                }
            }
        }
    }
    
    /// Mark download as completed
    pub fn complete_download(&mut self, id: &str, filename: Option<String>) {
        if let Some(download) = self.downloads.get_mut(id) {
            download.status = DownloadStatus::Completed;
            download.percent = Some(100.0);
            if filename.is_some() {
                download.filename = filename;
            }
        }
    }
    
    /// Mark download as failed
    pub fn fail_download(&mut self, id: &str, error: &str) {
        if let Some(download) = self.downloads.get_mut(id) {
            download.status = DownloadStatus::Failed;
            download.error = Some(error.to_string());
        }
    }
    
    /// Get download progress
    pub fn get_progress(&self, id: &str) -> Option<&DownloadProgress> {
        self.downloads.get(id)
    }
    
    /// Create a batch download
    pub fn create_batch(&mut self, urls: &[String]) -> (String, Vec<String>) {
        let batch_id = uuid::Uuid::new_v4().to_string();
        let mut download_ids = Vec::new();
        
        for url in urls {
            let id = self.start_download(url, None);
            download_ids.push(id);
        }
        
        self.batches.insert(batch_id.clone(), download_ids.clone());
        
        (batch_id, download_ids)
    }
}

// ============================================================================
// Storage Manager
// ============================================================================

/// Storage manager for file lifecycle
#[derive(Debug, Clone)]
pub struct StorageManager {
    pub config: StorageConfig,
    pub file_refs: HashMap<String, FileRef>,
}

impl StorageManager {
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            file_refs: HashMap::new(),
        }
    }
    
    /// Create a new file reference
    pub fn create_file_ref(&mut self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono_now_iso8601();
        let expires = chrono_add_seconds(&now, self.config.default_ttl_seconds);
        
        self.file_refs.insert(id.clone(), FileRef {
            id: id.clone(),
            created_at: now,
            expires_at: expires,
            files: Vec::new(),
            total_size: 0,
            persistent: false,
        });
        
        id
    }
    
    /// Add a file to a reference
    pub fn add_file(&mut self, ref_id: &str, file: FileInfo) -> Result<(), String> {
        let file_ref = self.file_refs.get_mut(ref_id)
            .ok_or_else(|| format!("File reference '{}' not found", ref_id))?;
        
        // Check file size limit
        if file.size > self.config.max_file_size_bytes {
            return Err(format!("File size {} exceeds maximum {}", 
                file.size, self.config.max_file_size_bytes));
        }
        
        file_ref.total_size += file.size;
        file_ref.files.push(file);
        
        Ok(())
    }
    
    /// Get file reference
    pub fn get_file_ref(&self, id: &str) -> Option<&FileRef> {
        self.file_refs.get(id)
    }
    
    /// Mark file reference as persistent
    pub fn persist(&mut self, id: &str) -> Result<(), String> {
        let file_ref = self.file_refs.get_mut(id)
            .ok_or_else(|| format!("File reference '{}' not found", id))?;
        
        file_ref.persistent = true;
        file_ref.expires_at = "never".to_string();
        
        Ok(())
    }
    
    /// Extend TTL for file reference
    pub fn extend_ttl(&mut self, id: &str, seconds: u64) -> Result<String, String> {
        let file_ref = self.file_refs.get_mut(id)
            .ok_or_else(|| format!("File reference '{}' not found", id))?;
        
        if file_ref.persistent {
            return Ok(file_ref.expires_at.clone());
        }
        
        let new_expires = chrono_add_seconds(&chrono_now_iso8601(), seconds);
        file_ref.expires_at = new_expires.clone();
        
        Ok(new_expires)
    }
    
    /// Get storage status
    pub fn get_status(&self) -> StorageStatus {
        let data_path = self.config.data_path.display().to_string();
        
        // Calculate used space (simplified - actual impl would scan directories)
        let used_bytes: u64 = self.file_refs.values()
            .map(|r| r.total_size)
            .sum();
        
        let usage_percent = used_bytes as f32 / self.config.max_storage_bytes as f32;
        
        let alert_level = if usage_percent >= self.config.critical_threshold {
            AlertLevel::Critical
        } else if usage_percent >= self.config.warning_threshold {
            AlertLevel::Warning
        } else {
            AlertLevel::Normal
        };
        
        StorageStatus {
            data_path,
            used_bytes,
            max_bytes: self.config.max_storage_bytes,
            usage_percent: usage_percent * 100.0,
            alert_level,
            directories: HashMap::new(), // Would scan actual directories
        }
    }
    
    /// Cleanup expired files
    pub fn cleanup_expired(&mut self, dry_run: bool) -> CleanupResponse {
        let now = chrono_now_iso8601();
        let mut files_deleted = 0;
        let mut bytes_freed = 0;
        let mut details = Vec::new();
        
        let expired_refs: Vec<String> = self.file_refs.iter()
            .filter(|(_, r)| !r.persistent && r.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();
        
        for ref_id in expired_refs {
            if let Some(file_ref) = self.file_refs.get(&ref_id) {
                for file in &file_ref.files {
                    details.push(CleanupDetail {
                        path: file.path.clone(),
                        reason: "expired".to_string(),
                        size: file.size,
                    });
                    bytes_freed += file.size;
                    files_deleted += 1;
                }
                
                if !dry_run {
                    self.file_refs.remove(&ref_id);
                }
            }
        }
        
        CleanupResponse {
            success: true,
            files_deleted,
            bytes_freed,
            dry_run,
            details,
        }
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new(StorageConfig::default())
    }
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

fn chrono_add_seconds(base: &str, seconds: u64) -> String {
    let base_secs: u64 = base.parse().unwrap_or(0);
    format!("{}", base_secs + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_download_manager() {
        let mut manager = DownloadManager::new();
        
        let id = manager.start_download("https://example.com/file.zip", None);
        assert!(manager.get_progress(&id).is_some());
        
        manager.update_progress(&id, 500, Some(1000));
        let progress = manager.get_progress(&id).unwrap();
        assert_eq!(progress.percent, Some(50.0));
        
        manager.complete_download(&id, Some("file.zip".to_string()));
        let progress = manager.get_progress(&id).unwrap();
        assert_eq!(progress.status, DownloadStatus::Completed);
    }
    
    #[test]
    fn test_storage_manager() {
        let mut manager = StorageManager::default();
        
        let ref_id = manager.create_file_ref();
        assert!(manager.get_file_ref(&ref_id).is_some());
        
        let file = FileInfo {
            filename: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            size: 1024,
            mime_type: "text/plain".to_string(),
            created_at: chrono_now_iso8601(),
        };
        
        manager.add_file(&ref_id, file).unwrap();
        
        let file_ref = manager.get_file_ref(&ref_id).unwrap();
        assert_eq!(file_ref.files.len(), 1);
        assert_eq!(file_ref.total_size, 1024);
    }
    
    #[test]
    fn test_storage_status() {
        let manager = StorageManager::default();
        let status = manager.get_status();
        
        assert_eq!(status.alert_level, AlertLevel::Normal);
    }
    
    #[test]
    fn test_batch_download() {
        let mut manager = DownloadManager::new();
        
        let urls = vec![
            "https://example.com/a.zip".to_string(),
            "https://example.com/b.zip".to_string(),
        ];
        
        let (batch_id, download_ids) = manager.create_batch(&urls);
        assert_eq!(download_ids.len(), 2);
        assert!(manager.batches.contains_key(&batch_id));
    }
}
