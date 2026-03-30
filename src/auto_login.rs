//! 1Password CLI integration for automatic session login
//!
//! Uses the `op` CLI binary to retrieve credentials and fill login forms.
//! Credentials are never logged or exposed in API responses — only usernames.

use serde_json::Value;
use std::process::Command;

/// Build an `op` Command with:
/// - `CREATE_NO_WINDOW` on Windows (no black console popup)
/// - `OP_SERVICE_ACCOUNT_TOKEN` env var when configured (no interactive auth prompts)
fn op_cmd(op_path: &str) -> Command {
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut c = Command::new(op_path);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = Command::new(op_path);

    // Inject service account token if configured — eliminates interactive auth prompts.
    if let Some(token) = crate::core::config::get_config()
        .session
        .op_service_account_token
        .as_deref()
    {
        if !token.is_empty() {
            cmd.env("OP_SERVICE_ACCOUNT_TOKEN", token);
        }
    }
    cmd
}

/// Legacy alias used by `find_op_binary` probe (no token needed for --version check).
fn no_window_cmd(program: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new(program);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

// ============================================================================
// Types
// ============================================================================

/// Credentials retrieved from 1Password
pub struct Credentials {
    /// 1Password item ID (used for caching in session DB)
    pub op_item_id: String,
    /// Username/email
    pub username: String,
    /// Password (not logged, not returned in API responses)
    pub password: String,
}

// ============================================================================
// Binary Detection
// ============================================================================

/// Find the 1Password CLI binary.
///
/// Resolution order:
/// 1. `config_path` from config.toml `[session] op_path`
/// 2. `op` in system PATH
/// 3. Well-known winget install path
pub fn find_op_binary(config_path: Option<&str>) -> Option<String> {
    // 1. Config-specified path
    if let Some(p) = config_path {
        if !p.is_empty() && std::path::Path::new(p).exists() {
            return Some(p.to_string());
        }
    }

    // 2. `op` in PATH
    let probe = if cfg!(windows) {
        no_window_cmd("op.exe").arg("--version").output()
    } else {
        no_window_cmd("op").arg("--version").output()
    };
    if probe.is_ok() {
        return Some(if cfg!(windows) { "op.exe" } else { "op" }.to_string());
    }

    // 3. Well-known winget install location (Windows)
    #[cfg(windows)]
    {
        if let Some(home) = dirs::home_dir() {
            let winget = home.join(
                r"AppData\Local\Microsoft\WinGet\Packages\AgileBits.1Password.CLI_Microsoft.Winget.Source_8wekyb3d8bbwe\op.exe",
            );
            if winget.exists() {
                return Some(winget.to_string_lossy().to_string());
            }
        }
        // 1Password CLI standard install path
        let standard = std::path::Path::new(r"C:\Program Files\1Password CLI\op.exe");
        if standard.exists() {
            return Some(standard.to_string_lossy().to_string());
        }
    }

    None
}

// ============================================================================
// Credential Retrieval
// ============================================================================

/// Fetch credentials (username + password) from a 1Password item.
///
/// `item_name` can be an item name, title, or UUID.
/// `vault` is required when using a service account token.
pub fn fetch_credentials(
    op_path: &str,
    item_name: &str,
    vault: Option<&str>,
) -> Result<Credentials, String> {
    let mut cmd = op_cmd(op_path);
    cmd.args(["item", "get", item_name, "--format", "json"]);
    if let Some(v) = vault {
        cmd.args(["--vault", v]);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run op binary: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("op item get failed: {}", stderr.trim()));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse op output: {}", e))?;

    let op_item_id = json["id"]
        .as_str()
        .ok_or("No 'id' field in op output")?
        .to_string();

    let fields = json["fields"]
        .as_array()
        .ok_or("No 'fields' array in op output")?;

    let username = fields
        .iter()
        .find(|f| f["purpose"].as_str() == Some("USERNAME"))
        .and_then(|f| f["value"].as_str())
        .ok_or("No USERNAME field in 1Password item")?
        .to_string();

    let password = fields
        .iter()
        .find(|f| f["purpose"].as_str() == Some("PASSWORD"))
        .and_then(|f| f["value"].as_str())
        .ok_or("No PASSWORD field in 1Password item")?
        .to_string();

    Ok(Credentials {
        op_item_id,
        username,
        password,
    })
}

/// Read a secret reference using `op read`.
///
/// Used for OTP/TOTP fields, e.g. `op://Personal/item/one-time password`
pub fn read_secret(op_path: &str, op_ref: &str) -> Result<String, String> {
    let output = op_cmd(op_path)
        .args(["read", op_ref])
        .output()
        .map_err(|e| format!("Failed to run op binary: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("op read failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Search 1Password items whose URL host matches the given URL.
///
/// Returns a list of `(item_id, item_title)` pairs.
pub fn search_items_by_url(op_path: &str, url: &str) -> Result<Vec<(String, String)>, String> {
    let output = op_cmd(op_path)
        .args(["item", "list", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to run op binary: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("op item list failed: {}", stderr.trim()));
    }

    let items: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Parse error: {}", e))?;

    let target_host = extract_host(url);

    let matches = items
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|item| {
            item["urls"]
                .as_array()
                .map(|urls| {
                    urls.iter().any(|u| {
                        u["href"]
                            .as_str()
                            .map(|href| extract_host(href) == target_host)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .map(|item| {
            (
                item["id"].as_str().unwrap_or("").to_string(),
                item["title"].as_str().unwrap_or("").to_string(),
            )
        })
        .filter(|(id, _)| !id.is_empty())
        .collect();

    Ok(matches)
}

fn extract_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url)
        .to_lowercase()
}
