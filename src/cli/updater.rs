use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const RELEASES_URL: &str =
    "https://api.github.com/repos/mhit/webview-bridge-release/releases/latest";
const EXPECTED_URL_PREFIX: &str =
    "https://github.com/mhit/webview-bridge-release/releases/download/";
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

// ---------------------------------------------------------------------------
// Version comparison (H4: single source, H5: pre-release safe)
// ---------------------------------------------------------------------------

/// Compare semver versions. Strips pre-release suffix (e.g. "3.8.0-rc.1" → "3.8.0").
/// Returns true if `latest` is strictly newer than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        // Strip pre-release/build metadata: "3.8.0-rc.1+build" → "3.8.0"
        let base = s.split(['-', '+']).next().unwrap_or(s);
        base.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    // Only compare if both have at least major.minor.patch
    if l.len() < 3 || c.len() < 3 {
        return false;
    }
    l > c
}

// ---------------------------------------------------------------------------
// GitHub API
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Download & replace binary
// ---------------------------------------------------------------------------

/// Download and replace current binary
pub fn perform_update(release: &ReleaseInfo) -> Result<(), UpdateError> {
    // C2: Validate URL origin (check the API-returned URL, not the redirect target)
    if !release.asset_url.starts_with(EXPECTED_URL_PREFIX) {
        return Err(UpdateError::Network(format!(
            "Unexpected download URL origin: {}",
            &release.asset_url[..release.asset_url.len().min(80)]
        )));
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

    // Fast-fail on Content-Length before reading body
    if let Some(len_str) = resp.header("Content-Length") {
        if let Ok(len) = len_str.parse::<u64>() {
            if len > MAX_ASSET_SIZE {
                return Err(UpdateError::Network(format!(
                    "Asset too large: {len} bytes (max {MAX_ASSET_SIZE})"
                )));
            }
        }
    }

    // Read body with hard cap
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(MAX_ASSET_SIZE)
        .read_to_end(&mut bytes)
        .map_err(|e| UpdateError::Io(format!("Read failed: {e}")))?;

    // Validate magic bytes
    validate_binary(&bytes)?;

    // C1: Atomic binary replacement
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cache operations (C3: 0600 permissions on Unix)
// ---------------------------------------------------------------------------

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
    let Ok(json) = serde_json::to_string(&cache) else {
        return;
    };

    // C3: Write cache with restricted permissions (0600 on Unix)
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
        {
            f.write_all(json.as_bytes()).ok();
        }
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, json).ok();
    }
}

// ---------------------------------------------------------------------------
// Binary validation
// ---------------------------------------------------------------------------

fn validate_binary(bytes: &[u8]) -> Result<(), UpdateError> {
    if bytes.len() < 4 {
        return Err(UpdateError::Io("Downloaded file too small".into()));
    }
    let valid = match &bytes[..4] {
        [0x7f, b'E', b'L', b'F'] => true, // ELF (Linux)
        [b'M', b'Z', ..] => true,         // PE (Windows)
        [0xcf, 0xfa, 0xed, 0xfe] => true, // Mach-O 64-bit (macOS)
        [0xfe, 0xed, 0xfa, 0xcf] => true, // Mach-O 64-bit BE
        [0xca, 0xfe, 0xba, 0xbe] => true, // Universal binary (macOS)
        _ => false,
    };
    if !valid {
        return Err(UpdateError::Io(
            "Downloaded file is not a valid executable".into(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// C1: Atomic binary replacement (write-tmp → rename)
// ---------------------------------------------------------------------------

fn replace_binary(new_bytes: &[u8]) -> Result<(), UpdateError> {
    let current_exe =
        std::env::current_exe().map_err(|e| UpdateError::Io(format!("Cannot find self: {e}")))?;

    // Resolve symlinks to get the actual file
    let actual_path = std::fs::canonicalize(&current_exe)
        .map_err(|e| UpdateError::Io(format!("Cannot resolve path: {e}")))?;

    // Write new binary to a temp file in the SAME directory (required for atomic rename)
    let tmp_path = actual_path.with_extension(if cfg!(windows) { "new.exe" } else { "new" });

    // Write new bytes to temp file
    std::fs::write(&tmp_path, new_bytes)
        .map_err(|e| UpdateError::Io(format!("Cannot write temp binary: {e}")))?;

    // Set executable permission on Unix before rename
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Preserve original permissions, fallback to 0o755
        let perms = std::fs::metadata(&actual_path)
            .map(|m| m.permissions())
            .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o755));
        std::fs::set_permissions(&tmp_path, perms).ok();
    }

    // On Unix: atomic rename replaces the target
    // On Windows: cannot rename over a running exe, so rename old away first
    #[cfg(unix)]
    {
        if let Err(e) = std::fs::rename(&tmp_path, &actual_path) {
            std::fs::remove_file(&tmp_path).ok();
            return Err(UpdateError::Io(format!("Cannot replace binary: {e}")));
        }
    }

    #[cfg(windows)]
    {
        let backup_path = actual_path.with_extension("old.exe");
        // Clean up any leftover .old from previous update
        std::fs::remove_file(&backup_path).ok();

        // Rename running exe → .old (Windows allows renaming a running exe)
        std::fs::rename(&actual_path, &backup_path).map_err(|e| {
            std::fs::remove_file(&tmp_path).ok();
            UpdateError::Io(format!("Cannot rename current binary: {e}"))
        })?;

        // Rename new → actual
        if let Err(e) = std::fs::rename(&tmp_path, &actual_path) {
            // Rollback: restore old binary
            std::fs::rename(&backup_path, &actual_path).ok();
            return Err(UpdateError::Io(format!("Cannot place new binary: {e}")));
        }
        // .old.exe will be cleaned up on next successful launch
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// H6: Cleanup leftover .old binary (called AFTER successful command, not at startup)
// ---------------------------------------------------------------------------

/// Clean up leftover .old backup from a previous update
pub fn cleanup_old_binary() {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(actual) = std::fs::canonicalize(&exe) {
            let old = actual.with_extension(if cfg!(windows) { "old.exe" } else { "old" });
            if old.exists() {
                std::fs::remove_file(&old).ok();
            }
            // Also clean up any leftover .new temp file from interrupted update
            let tmp = actual.with_extension(if cfg!(windows) { "new.exe" } else { "new" });
            if tmp.exists() {
                std::fs::remove_file(&tmp).ok();
            }
        }
    }
}
