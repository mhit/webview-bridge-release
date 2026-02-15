use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const RELEASES_URL: &str =
    "https://api.github.com/repos/mhit/webview-bridge-release/releases/latest";
const CACHE_TTL_SECS: i64 = 86400; // 24 hours
const MAX_ASSET_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
const CHECK_TIMEOUT_SECS: u64 = 5;
const DOWNLOAD_TIMEOUT_SECS: u64 = 120;
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
// Populated by build.rs when cli feature is active
const TARGET: &str = env!("TARGET");

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub asset_url: String,
    pub asset_name: String,
}

#[derive(Serialize, Deserialize)]
struct UpdateCache {
    last_check: i64,
    latest_version: String,
    asset_url: String,
    asset_name: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    NoAsset(String),
    Io(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(m) => write!(f, "Network error: {m}"),
            Self::NoAsset(m) => write!(f, "No matching asset: {m}"),
            Self::Io(m) => write!(f, "IO error: {m}"),
        }
    }
}

/// Fetch latest release info from GitHub API
pub fn check_latest() -> Result<ReleaseInfo, UpdateError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(CHECK_TIMEOUT_SECS))
        .build();

    let resp = agent
        .get(RELEASES_URL)
        .set("User-Agent", &format!("wb-cli/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;

    let release: GhRelease = resp
        .into_json()
        .map_err(|e| UpdateError::Network(format!("Invalid JSON: {e}")))?;

    let version = release.tag_name.trim_start_matches('v').to_string();

    // Find matching asset for current platform
    let asset = find_matching_asset(&release.assets, &version)?;

    Ok(ReleaseInfo {
        version,
        asset_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
    })
}

/// Check with 24h cache — returns Some(info) if newer version available
pub fn check_with_cache() -> Result<Option<ReleaseInfo>, UpdateError> {
    let cache_path = cache_file_path();

    // Try reading cache
    if let Some(cache) = read_cache(&cache_path) {
        let now = chrono::Utc::now().timestamp();
        if now - cache.last_check < CACHE_TTL_SECS {
            // Cache is fresh — compare versions
            if is_newer(&cache.latest_version, CURRENT_VERSION) {
                return Ok(Some(ReleaseInfo {
                    version: cache.latest_version,
                    asset_url: cache.asset_url,
                    asset_name: cache.asset_name,
                }));
            }
            return Ok(None);
        }
    }

    // Cache expired or missing — fetch from API
    let release = check_latest()?;
    write_cache(&cache_path, &release);

    if is_newer(&release.version, CURRENT_VERSION) {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

/// Download and replace current binary
pub fn perform_update(release: &ReleaseInfo) -> Result<(), UpdateError> {
    // Validate URL origin
    if !release.asset_url.starts_with("https://github.com/mhit/webview-bridge-release/") {
        return Err(UpdateError::Network("Unexpected download origin".into()));
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .redirects(10)
        .build();

    let resp = agent
        .get(&release.asset_url)
        .set("User-Agent", &format!("wb-cli/{CURRENT_VERSION}"))
        .call()
        .map_err(|e| UpdateError::Network(format!("Download failed: {e}")))?;

    // Check Content-Length
    if let Some(len_str) = resp.header("Content-Length") {
        if let Ok(len) = len_str.parse::<u64>() {
            if len > MAX_ASSET_SIZE {
                return Err(UpdateError::Io(format!("Asset too large: {len} bytes")));
            }
        }
    }

    // Read body
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(MAX_ASSET_SIZE)
        .read_to_end(&mut bytes)
        .map_err(|e| UpdateError::Io(format!("Read failed: {e}")))?;

    // Validate magic bytes
    validate_binary(&bytes)?;

    // Replace current binary
    replace_binary(&bytes)
}

/// Background check — returns message if update available
pub fn background_check() -> Option<String> {
    match check_with_cache() {
        Ok(Some(info)) => Some(format!(
            "[wb] Update available: v{} (run `wb update`)",
            info.version
        )),
        _ => None,
    }
}

// --- Internal helpers ---

fn find_matching_asset<'a>(
    assets: &'a [GhAsset],
    version: &str,
) -> Result<&'a GhAsset, UpdateError> {
    // Expected pattern: wb-v{version}-{target} or wb-v{version}-{target}.exe
    let pattern = format!("wb-v{version}-{TARGET}");
    assets
        .iter()
        .find(|a| a.name.starts_with(&pattern) && a.size <= MAX_ASSET_SIZE)
        .ok_or_else(|| UpdateError::NoAsset(format!("No asset matching {pattern} for {TARGET}")))
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

fn cache_file_path() -> PathBuf {
    let dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::data_dir().map(|d| d.join("webview-bridge")))
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("update-check.json")
}

fn read_cache(path: &PathBuf) -> Option<UpdateCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(path: &PathBuf, release: &ReleaseInfo) {
    let cache = UpdateCache {
        last_check: chrono::Utc::now().timestamp(),
        latest_version: release.version.clone(),
        asset_url: release.asset_url.clone(),
        asset_name: release.asset_name.clone(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string(&cache) {
        std::fs::write(path, json).ok();
    }
}

fn validate_binary(bytes: &[u8]) -> Result<(), UpdateError> {
    if bytes.len() < 4 {
        return Err(UpdateError::Io("Downloaded file too small".into()));
    }
    let valid = match &bytes[..4] {
        [0x7f, b'E', b'L', b'F'] => true,     // ELF (Linux)
        [b'M', b'Z', ..] => true,              // PE (Windows)
        [0xcf, 0xfa, 0xed, 0xfe] => true,      // Mach-O 64-bit (macOS)
        [0xfe, 0xed, 0xfa, 0xcf] => true,      // Mach-O 64-bit BE
        [0xca, 0xfe, 0xba, 0xbe] => true,      // Universal binary (macOS)
        _ => false,
    };
    if !valid {
        return Err(UpdateError::Io(
            "Downloaded file is not a valid executable".into(),
        ));
    }
    Ok(())
}

fn replace_binary(new_bytes: &[u8]) -> Result<(), UpdateError> {
    let current_exe =
        std::env::current_exe().map_err(|e| UpdateError::Io(format!("Cannot find self: {e}")))?;

    // Resolve symlinks to get the actual file
    let actual_path = std::fs::canonicalize(&current_exe)
        .map_err(|e| UpdateError::Io(format!("Cannot resolve path: {e}")))?;

    let backup_path = actual_path.with_extension(if cfg!(windows) {
        "old.exe"
    } else {
        "old"
    });

    // Rename current → backup
    std::fs::rename(&actual_path, &backup_path)
        .map_err(|e| UpdateError::Io(format!("Cannot rename current binary: {e}")))?;

    // Write new binary
    if let Err(e) = std::fs::write(&actual_path, new_bytes) {
        // Rollback: restore backup
        std::fs::rename(&backup_path, &actual_path).ok();
        return Err(UpdateError::Io(format!("Cannot write new binary: {e}")));
    }

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&actual_path, perms).ok();
    }

    // Remove backup (on Windows this may fail if the old binary is still loaded,
    // which is fine — it will be cleaned up on next launch)
    std::fs::remove_file(&backup_path).ok();

    Ok(())
}

/// Clean up leftover .old backup from a previous Windows update
pub fn cleanup_old_binary() {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(actual) = std::fs::canonicalize(&exe) {
            let old = actual.with_extension(if cfg!(windows) {
                "old.exe"
            } else {
                "old"
            });
            if old.exists() {
                std::fs::remove_file(&old).ok();
            }
        }
    }
}
