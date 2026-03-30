use std::path::PathBuf;
use uuid::Uuid;

/// Uploaded file metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct UploadedFile {
    pub upload_id: String,
    pub filename: String,
    pub stored_filename: String,
    pub size: u64,
    pub mime_type: String,
    pub session: String,
    pub file_path: PathBuf,
    pub url: String,
    pub uploaded_at: String,
}

/// Get uploads directory for a session
pub fn uploads_dir(session: &str) -> PathBuf {
    super::config::AppConfig::data_dir()
        .join("uploads")
        .join(session)
}

/// Sanitize a filename for safe storage.
/// Strips path separators, limits length, prepends UUID.
pub fn sanitize_and_store_filename(original: &str) -> String {
    // Extract just the filename part (strip any path components)
    let basename = original
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or("upload");

    // Remove dangerous characters, keep alphanumeric + .-_
    let clean: String = basename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Limit length and ensure non-empty
    let clean = if clean.is_empty() || clean == "." || clean == ".." {
        "upload".to_string()
    } else if clean.len() > 200 {
        clean[..200].to_string()
    } else {
        clean
    };

    // Prepend short UUID for collision prevention
    let short_id = &Uuid::new_v4().to_string()[..8];
    format!("{short_id}_{clean}")
}

/// Detect MIME type from filename extension
pub fn detect_mime_type(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Validate a filename for serving (prevent path traversal)
pub fn validate_filename(filename: &str) -> Result<(), String> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid filename: path traversal detected".to_string());
    }
    if filename.is_empty() {
        return Err("Empty filename".to_string());
    }
    Ok(())
}

/// Validate session name for directory use
pub fn validate_session_name(session: &str) -> Result<(), String> {
    if session.is_empty() {
        return Err("Empty session name".to_string());
    }
    if session.contains("..") || session.contains('/') || session.contains('\\') {
        return Err("Invalid session name".to_string());
    }
    Ok(())
}

/// Max upload size: 100MB
pub const MAX_UPLOAD_SIZE: u64 = 100 * 1024 * 1024;
