use crate::client::{WbClient, WbError};
use crate::output::OutputOpts;
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const RELEASES_URL: &str =
    "https://api.github.com/repos/mhit/webview-bridge-release/releases/latest";
const EXPECTED_URL_PREFIX: &str =
    "https://github.com/mhit/webview-bridge-release/releases/download/";
const SERVER_ASSET_NAME: &str = "webview-bridge-rust.exe";
const MAX_SERVER_SIZE: u64 = 200 * 1024 * 1024; // 200 MB
const TASK_NAME: &str = "WebViewBridge Auto Update";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

// ---------------------------------------------------------------------------
// wb server update
// ---------------------------------------------------------------------------

pub fn update(client: &WbClient, _opts: &OutputOpts, force: bool) -> Result<(), WbError> {
    // 1. Fetch latest release info
    println!("Checking latest release...");
    let (latest_version, asset_url) = fetch_server_asset()
        .map_err(|e| WbError::general(format!("GitHub API error: {e}")))?;

    // 2. Compare with running server version
    let server_version = get_server_version(client);
    let display_version = server_version
        .as_deref()
        .unwrap_or(CURRENT_VERSION);

    println!("Server:  v{display_version}");
    println!("Latest:  v{latest_version}");

    if !force && !crate::updater::is_newer(&latest_version, display_version) {
        println!("Already up to date.");
        return Ok(());
    }

    // 3. Find server exe path
    let exe_path = find_server_exe()
        .ok_or_else(|| WbError::general("Cannot locate webview-bridge-rust.exe. Pass --exe-path or install via installer."))?;

    println!("Downloading {}...", SERVER_ASSET_NAME);
    let bytes = download_asset(&asset_url)
        .map_err(|e| WbError::general(format!("Download failed: {e}")))?;

    // 4. Stop server
    println!("Stopping server...");
    stop_server();

    // 5. Replace exe
    replace_server_exe(&exe_path, &bytes)
        .map_err(|e| WbError::general(format!("Replace failed: {e}")))?;

    // 6. Restart server
    println!("Starting server...");
    start_server(&exe_path);

    println!("Server updated to v{latest_version} successfully.");
    Ok(())
}

// ---------------------------------------------------------------------------
// wb server schedule
// ---------------------------------------------------------------------------

pub fn schedule(_opts: &OutputOpts, time: &str, exe_path_override: Option<&str>) -> Result<(), WbError> {
    #[cfg(not(windows))]
    {
        return Err(WbError::general("Task Scheduler is Windows-only."));
    }

    #[cfg(windows)]
    {
        let wb_exe = exe_path_override
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok())
            .ok_or_else(|| WbError::general("Cannot determine wb.exe path"))?;

        let wb_exe_str = wb_exe.to_string_lossy();
        let task_cmd = format!("\"{}\" server update", wb_exe_str);

        let status = std::process::Command::new("schtasks")
            .args([
                "/create",
                "/tn", TASK_NAME,
                "/tr", &task_cmd,
                "/sc", "daily",
                "/st", time,
                "/rl", "highest",
                "/f",
            ])
            .status()
            .map_err(|e| WbError::general(format!("schtasks failed: {e}")))?;

        if !status.success() {
            return Err(WbError::general(format!(
                "schtasks /create failed (exit {}). Try running as Administrator.",
                status.code().unwrap_or(-1)
            )));
        }

        println!("Scheduled: \"{TASK_NAME}\" — daily at {time}");
        println!("Command:   {task_cmd}");
        println!("Use `wb server unschedule` to remove.");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// wb server unschedule
// ---------------------------------------------------------------------------

pub fn unschedule(_opts: &OutputOpts) -> Result<(), WbError> {
    #[cfg(not(windows))]
    {
        return Err(WbError::general("Task Scheduler is Windows-only."));
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new("schtasks")
            .args(["/delete", "/tn", TASK_NAME, "/f"])
            .status()
            .map_err(|e| WbError::general(format!("schtasks failed: {e}")))?;

        if !status.success() {
            return Err(WbError::general(format!(
                "Task not found or delete failed (exit {}).",
                status.code().unwrap_or(-1)
            )));
        }

        println!("Removed scheduled task: \"{TASK_NAME}\"");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fetch_server_asset() -> Result<(String, String), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();

    let resp = agent
        .get(RELEASES_URL)
        .set("User-Agent", &format!("wb-cli/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;

    let release: GhRelease = resp.into_json().map_err(|e| e.to_string())?;
    let version = release.tag_name.trim_start_matches('v').to_string();

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == SERVER_ASSET_NAME && a.size <= MAX_SERVER_SIZE)
        .ok_or_else(|| format!("Asset '{SERVER_ASSET_NAME}' not found in release {version}"))?;

    Ok((version, asset.browser_download_url.clone()))
}

fn get_server_version(client: &WbClient) -> Option<String> {
    let resp = client.get("/health").ok()?;
    resp.body.get("version")?.as_str().map(|s| s.to_string())
}

fn find_server_exe() -> Option<PathBuf> {
    // 1. Same directory as wb.exe
    if let Ok(wb_exe) = std::env::current_exe() {
        if let Some(dir) = wb_exe.parent() {
            let candidate = dir.join(SERVER_ASSET_NAME);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. %APPDATA%\webview-bridge\ (data directory)
    let data_dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::data_dir().map(|d| d.join("webview-bridge")));

    if let Some(dir) = data_dir {
        let candidate = dir.join(SERVER_ASSET_NAME);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn download_asset(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with(EXPECTED_URL_PREFIX) {
        return Err(format!("Unexpected download URL: {}", &url[..url.len().min(80)]));
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(300))
        .redirects(10)
        .build();

    let resp = agent
        .get(url)
        .set("User-Agent", &format!("wb-cli/{CURRENT_VERSION}"))
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let mut bytes = Vec::new();
    resp.into_reader()
        .take(MAX_SERVER_SIZE)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Read error: {e}"))?;

    // Validate PE header (MZ)
    if bytes.len() < 2 || &bytes[..2] != b"MZ" {
        return Err("Downloaded file is not a valid Windows executable".into());
    }

    Ok(bytes)
}

fn stop_server() {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", SERVER_ASSET_NAME, "/F"])
            .status();
        // Give the process time to exit and release file handles
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn replace_server_exe(exe_path: &PathBuf, new_bytes: &[u8]) -> Result<(), String> {
    let tmp_path = exe_path.with_extension("new.exe");
    let backup_path = exe_path.with_extension("old.exe");

    std::fs::write(&tmp_path, new_bytes)
        .map_err(|e| format!("Cannot write temp file: {e}"))?;

    // Clean up leftover backup
    let _ = std::fs::remove_file(&backup_path);

    // Rename current → .old
    std::fs::rename(exe_path, &backup_path)
        .map_err(|e| format!("Cannot rename current exe: {e}"))?;

    // Rename new → current
    if let Err(e) = std::fs::rename(&tmp_path, exe_path) {
        // Rollback
        let _ = std::fs::rename(&backup_path, exe_path);
        return Err(format!("Cannot place new exe: {e}"));
    }

    Ok(())
}

fn start_server(exe_path: &PathBuf) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &exe_path.to_string_lossy()])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = exe_path;
    }
}
