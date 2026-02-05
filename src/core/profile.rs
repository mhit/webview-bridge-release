//! Profile management for WebView Bridge
//!
//! Profiles allow isolation of browser data (cookies, cache, etc.) between sessions.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

pub const PROFILES_DIR_NAME: &str = "profiles";

/// Error type for profile operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileError {
    pub code: String,
    pub message: String,
}

impl ProfileError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn profile_not_found(name: &str) -> Self {
        Self::new(
            "PROFILE_NOT_FOUND",
            format!("Profile '{}' does not exist", name),
        )
    }

    pub fn profile_exists(name: &str) -> Self {
        Self::new(
            "PROFILE_EXISTS",
            format!("Profile '{}' already exists", name),
        )
    }

    pub fn invalid_name(name: &str) -> Self {
        Self::new("INVALID_NAME", format!("Invalid profile name: '{}'", name))
    }
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ProfileError {}

impl From<io::Error> for ProfileError {
    fn from(err: io::Error) -> Self {
        Self::new("IO_ERROR", format!("FileSystem error: {}", err))
    }
}

/// Profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    pub path: String,
    pub created_at: Option<String>,
}

impl ProfileInfo {
    pub fn new(name: String, path: String) -> Self {
        let created_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string()),
        );
        Self {
            name,
            path,
            created_at,
        }
    }
}

/// Profile manager for handling browser profile directories
pub struct ProfileManager {
    base_dir: PathBuf,
}

impl ProfileManager {
    /// Create a new ProfileManager with the specified base directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a ProfileManager with the default base directory
    /// Default: ~/.webview-bridge/profiles
    pub fn default() -> Result<Self, ProfileError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            ProfileError::new("NO_HOME_DIR", "Could not determine home directory")
        })?;

        let base_dir = home_dir.join(".webview-bridge").join(PROFILES_DIR_NAME);

        // Ensure base directory exists
        fs::create_dir_all(&base_dir)?;

        Ok(Self { base_dir })
    }

    /// Get the base profiles directory
    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    /// List all available profiles
    pub fn list_profiles(&self) -> Result<Vec<ProfileInfo>, ProfileError> {
        let mut profiles = Vec::new();

        let entries = fs::read_dir(&self.base_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only directories are valid profiles
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        // Skip hidden directories
                        if !name_str.starts_with('.') {
                            profiles.push(ProfileInfo::new(
                                name_str.to_string(),
                                path.to_string_lossy().to_string(),
                            ));
                        }
                    }
                }
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Check if a profile exists
    pub fn profile_exists(&self, name: &str) -> bool {
        self.profile_path(name).exists()
    }

    /// Get the path for a specific profile
    pub fn profile_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    /// Get the user data folder for a specific profile
    /// This is where WebView2 will store cookies, cache, etc.
    pub fn user_data_folder(&self, name: &str) -> String {
        self.profile_path(name)
            .join("userdata")
            .to_string_lossy()
            .to_string()
    }

    /// Create a new profile
    pub fn create_profile(&self, name: &str) -> Result<ProfileInfo, ProfileError> {
        // Validate profile name
        Self::validate_name(name)?;

        // Check if profile already exists
        if self.profile_exists(name) {
            return Err(ProfileError::profile_exists(name));
        }

        let profile_path = self.profile_path(name);
        let user_data_path = profile_path.join("userdata");

        // Create profile directories
        fs::create_dir_all(&user_data_path)?;

        Ok(ProfileInfo::new(
            name.to_string(),
            profile_path.to_string_lossy().to_string(),
        ))
    }

    /// Delete a profile
    pub fn delete_profile(&self, name: &str) -> Result<(), ProfileError> {
        if !self.profile_exists(name) {
            return Err(ProfileError::profile_not_found(name));
        }

        let profile_path = self.profile_path(name);
        fs::remove_dir_all(&profile_path)?;

        Ok(())
    }

    /// Validate a profile name
    /// Rules: alphanumeric, underscore, hyphen only; 1-64 characters
    fn validate_name(name: &str) -> Result<(), ProfileError> {
        if name.is_empty() || name.len() > 64 {
            return Err(ProfileError::invalid_name(name));
        }

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ProfileError::invalid_name(name));
        }

        Ok(())
    }

    /// Ensure the "default" profile exists
    /// Creates it if it doesn't exist
    pub fn ensure_default_profile(&self) -> Result<ProfileInfo, ProfileError> {
        if !self.profile_exists("default") {
            self.create_profile("default")?;
        }

        Ok(ProfileInfo::new(
            "default".to_string(),
            self.profile_path("default").to_string_lossy().to_string(),
        ))
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::default().expect("Failed to create ProfileManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_manager() -> (TempDir, ProfileManager) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProfileManager::new(temp_dir.path().to_path_buf());
        (temp_dir, manager)
    }

    #[test]
    fn test_profile_manager_creation() {
        let (_temp, manager) = create_temp_manager();
        assert!(manager.base_dir().exists());
    }

    #[test]
    fn test_create_profile() {
        let (_temp, manager) = create_temp_manager();

        let result = manager.create_profile("test-profile");
        assert!(result.is_ok());

        let profile = result.unwrap();
        assert_eq!(profile.name, "test-profile");
        assert!(manager.profile_exists("test-profile"));
    }

    #[test]
    fn test_create_duplicate_profile() {
        let (_temp, manager) = create_temp_manager();

        manager.create_profile("duplicate").unwrap();
        let result = manager.create_profile("duplicate");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "PROFILE_EXISTS");
    }

    #[test]
    fn test_list_profiles() {
        let (_temp, manager) = create_temp_manager();

        manager.create_profile("alpha").unwrap();
        manager.create_profile("beta").unwrap();
        manager.create_profile("gamma").unwrap();

        let profiles = manager.list_profiles().unwrap();
        assert_eq!(profiles.len(), 3);

        // Should be sorted alphabetically
        assert_eq!(profiles[0].name, "alpha");
        assert_eq!(profiles[1].name, "beta");
        assert_eq!(profiles[2].name, "gamma");
    }

    #[test]
    fn test_delete_profile() {
        let (_temp, manager) = create_temp_manager();

        manager.create_profile("to-delete").unwrap();
        assert!(manager.profile_exists("to-delete"));

        let result = manager.delete_profile("to-delete");
        assert!(result.is_ok());
        assert!(!manager.profile_exists("to-delete"));
    }

    #[test]
    fn test_delete_nonexistent_profile() {
        let (_temp, manager) = create_temp_manager();

        let result = manager.delete_profile("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "PROFILE_NOT_FOUND");
    }

    #[test]
    fn test_validate_name_valid() {
        assert!(ProfileManager::validate_name("valid").is_ok());
        assert!(ProfileManager::validate_name("valid_name-123").is_ok());
        assert!(ProfileManager::validate_name("a").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        assert!(ProfileManager::validate_name("").is_err());
        assert!(ProfileManager::validate_name("invalid name").is_err());
        assert!(ProfileManager::validate_name("invalid@name").is_err());
        assert!(ProfileManager::validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn test_user_data_folder() {
        let (_temp, manager) = create_temp_manager();

        manager.create_profile("test").unwrap();
        let folder = manager.user_data_folder("test");

        assert!(folder.contains("test"));
        assert!(folder.contains("userdata"));
    }

    #[test]
    fn test_ensure_default_profile() {
        let (_temp, manager) = create_temp_manager();

        let result = manager.ensure_default_profile();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "default");
        assert!(manager.profile_exists("default"));

        // Calling again should not error
        assert!(manager.ensure_default_profile().is_ok());
    }
}
