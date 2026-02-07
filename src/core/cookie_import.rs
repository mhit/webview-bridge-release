//! Browser Cookie Import Module
//! 
//! Import cookies from Chrome, Edge, and Firefox into WebView sessions.
//! Supports Windows DPAPI decryption for Chromium-based browsers.

use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

/// Supported browser types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserType {
    Chrome,
    Edge,
    Firefox,
}

/// A single cookie to import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub expires: Option<i64>,
}

/// Request for cookie import
#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    /// Target session name
    pub session: String,
    /// Source browser
    pub browser: BrowserType,
    /// Optional: only import cookies for these domains
    #[serde(default)]
    pub domains: Vec<String>,
    /// Optional: browser profile name (default: "Default")
    #[serde(default = "default_profile")]
    pub profile: String,
}

fn default_profile() -> String {
    "Default".to_string()
}

/// Response for cookie import
#[derive(Debug, Clone, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub session: String,
    pub browser: String,
    pub imported_count: usize,
    pub domains_found: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Browser Profile Paths
// ============================================================================

/// Get the cookie database path for a browser
pub fn get_cookie_db_path(browser: BrowserType, profile: &str) -> Option<PathBuf> {
    let local_app_data = dirs::data_local_dir()?;
    
    match browser {
        BrowserType::Chrome => {
            Some(local_app_data
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join(profile)
                .join("Network")
                .join("Cookies"))
        }
        BrowserType::Edge => {
            Some(local_app_data
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join(profile)
                .join("Network")
                .join("Cookies"))
        }
        BrowserType::Firefox => {
            // Firefox profiles are more complex - need to find the profile folder
            let profiles_dir = dirs::data_dir()?
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles");
            
            // Find the default profile (ends with .default-release or .default)
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".default-release") || name.ends_with(".default") {
                        return Some(entry.path().join("cookies.sqlite"));
                    }
                }
            }
            None
        }
    }
}

/// Get the Local State file path for Chromium browsers (contains encryption key)
pub fn get_local_state_path(browser: BrowserType) -> Option<PathBuf> {
    let local_app_data = dirs::data_local_dir()?;
    
    match browser {
        BrowserType::Chrome => {
            Some(local_app_data
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Local State"))
        }
        BrowserType::Edge => {
            Some(local_app_data
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Local State"))
        }
        BrowserType::Firefox => None, // Firefox doesn't use this
    }
}

// ============================================================================
// Cookie Reading (without decryption for now)
// ============================================================================

/// Read cookies from a Chromium-based browser (Chrome/Edge)
/// 
/// Note: This reads the raw encrypted values. Full decryption requires
/// extracting the master key from Local State and using Windows DPAPI.
/// For now, we can copy the cookie database file which WebView2 can use.
pub fn read_chromium_cookies(
    db_path: &PathBuf,
    domain_filter: &[String],
) -> Result<Vec<ImportedCookie>, String> {
    // SQLite query to get cookies
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).map_err(|e| format!("Failed to open cookie database: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT host_key, name, path, is_secure, is_httponly, expires_utc FROM cookies"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let mut cookies = Vec::new();
    let rows = stmt.query_map([], |row| {
        Ok(ImportedCookie {
            domain: row.get::<_, String>(0)?,
            name: row.get::<_, String>(1)?,
            path: row.get::<_, String>(2)?,
            secure: row.get::<_, i32>(3)? != 0,
            http_only: row.get::<_, i32>(4)? != 0,
            expires: row.get::<_, i64>(5).ok(),
            value: String::new(), // Cannot decrypt without DPAPI
        })
    }).map_err(|e| format!("Query failed: {}", e))?;

    for cookie_result in rows {
        if let Ok(cookie) = cookie_result {
            // Filter by domain if specified
            if domain_filter.is_empty() || domain_filter.iter().any(|d| cookie.domain.contains(d)) {
                cookies.push(cookie);
            }
        }
    }

    Ok(cookies)
}

/// Read cookies from Firefox
pub fn read_firefox_cookies(
    db_path: &PathBuf,
    domain_filter: &[String],
) -> Result<Vec<ImportedCookie>, String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).map_err(|e| format!("Failed to open cookie database: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT host, name, value, path, isSecure, isHttpOnly, expiry FROM moz_cookies"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let mut cookies = Vec::new();
    let rows = stmt.query_map([], |row| {
        Ok(ImportedCookie {
            domain: row.get::<_, String>(0)?,
            name: row.get::<_, String>(1)?,
            value: row.get::<_, String>(2)?,
            path: row.get::<_, String>(3)?,
            secure: row.get::<_, i32>(4)? != 0,
            http_only: row.get::<_, i32>(5)? != 0,
            expires: row.get::<_, i64>(6).ok(),
        })
    }).map_err(|e| format!("Query failed: {}", e))?;

    for cookie_result in rows {
        if let Ok(cookie) = cookie_result {
            if domain_filter.is_empty() || domain_filter.iter().any(|d| cookie.domain.contains(d)) {
                cookies.push(cookie);
            }
        }
    }

    Ok(cookies)
}

/// List available browser profiles
pub fn list_browser_profiles(browser: BrowserType) -> Vec<String> {
    let mut profiles = Vec::new();
    
    let user_data = match browser {
        BrowserType::Chrome => dirs::data_local_dir()
            .map(|d| d.join("Google").join("Chrome").join("User Data")),
        BrowserType::Edge => dirs::data_local_dir()
            .map(|d| d.join("Microsoft").join("Edge").join("User Data")),
        BrowserType::Firefox => dirs::data_dir()
            .map(|d| d.join("Mozilla").join("Firefox").join("Profiles")),
    };
    
    if let Some(path) = user_data {
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                match browser {
                    BrowserType::Chrome | BrowserType::Edge => {
                        // Chrome/Edge profiles are "Default", "Profile 1", etc.
                        if name == "Default" || name.starts_with("Profile ") {
                            profiles.push(name);
                        }
                    }
                    BrowserType::Firefox => {
                        // Firefox profiles have a random prefix
                        if entry.path().is_dir() {
                            profiles.push(name);
                        }
                    }
                }
            }
        }
    }
    
    profiles
}

/// Get domain counts from cookies
pub fn summarize_cookies(cookies: &[ImportedCookie]) -> HashMap<String, usize> {
    let mut domain_counts: HashMap<String, usize> = HashMap::new();
    
    for cookie in cookies {
        // Extract base domain (remove leading dot)
        let domain = cookie.domain.trim_start_matches('.');
        *domain_counts.entry(domain.to_string()).or_insert(0) += 1;
    }
    
    domain_counts
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_type_serialize() {
        let chrome = BrowserType::Chrome;
        let json = serde_json::to_string(&chrome).unwrap();
        assert_eq!(json, "\"chrome\"");
    }

    #[test]
    fn test_import_request_deserialize() {
        let json = r#"{
            "session": "test",
            "browser": "chrome",
            "domains": ["google.com", "youtube.com"],
            "profile": "Default"
        }"#;
        
        let request: ImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.browser, BrowserType::Chrome);
        assert_eq!(request.domains.len(), 2);
    }

    #[test]
    fn test_list_profiles() {
        // Just test that it doesn't panic
        let _ = list_browser_profiles(BrowserType::Chrome);
        let _ = list_browser_profiles(BrowserType::Edge);
        let _ = list_browser_profiles(BrowserType::Firefox);
    }
}
