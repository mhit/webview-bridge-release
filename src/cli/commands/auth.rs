use crate::client::WbError;
use crate::output::{self, OutputOpts};

fn token_file_path() -> Result<std::path::PathBuf, WbError> {
    let data_dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
        .map(std::path::PathBuf::from)
        .ok()
        .or_else(|| dirs::data_dir().map(|d| d.join("webview-bridge")))
        .ok_or_else(|| WbError::general("Cannot determine data directory"))?;
    Ok(data_dir.join("cli-token"))
}

pub fn save(opts: &OutputOpts, token: &str) -> Result<(), WbError> {
    let path = token_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WbError::general(format!("Cannot create directory: {e}")))?;
    }

    // On Unix, create file with mode 0600 atomically to prevent race condition
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| WbError::general(format!("Cannot write token file: {e}")))?;
        file.write_all(token.as_bytes())
            .map_err(|e| WbError::general(format!("Cannot write token file: {e}")))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, token)
            .map_err(|e| WbError::general(format!("Cannot write token file: {e}")))?;
    }

    output::print_result(opts, &format!("Token saved to {}", path.display()));
    Ok(())
}

pub fn show(opts: &OutputOpts) -> Result<(), WbError> {
    let path = token_file_path()?;
    match std::fs::read_to_string(&path) {
        Ok(token) => {
            let token = token.trim();
            if token.is_empty() {
                output::print_result(opts, "Token file is empty");
            } else if opts.json {
                output::print_json(&serde_json::json!({ "token": token, "path": path.display().to_string() }));
            } else {
                // Safe masking using char boundaries (handles multibyte)
                let chars: Vec<char> = token.chars().collect();
                let masked = if chars.len() <= 12 {
                    let prefix: String = chars.iter().take(3).collect();
                    format!("{prefix}***")
                } else {
                    let prefix: String = chars.iter().take(8).collect();
                    let suffix: String = chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                    format!("{prefix}...{suffix}")
                };
                output::print_result(opts, &format!("Token: {masked}"));
                output::print_result(opts, &format!("Path: {}", path.display()));
            }
        }
        Err(_) => {
            output::print_result(opts, "No token saved. Use: wb auth save <token>");
        }
    }
    Ok(())
}

pub fn clear(opts: &OutputOpts) -> Result<(), WbError> {
    let path = token_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| WbError::general(format!("Cannot remove token file: {e}")))?;
        output::print_result(opts, "Token removed");
    } else {
        output::print_result(opts, "No token file to remove");
    }
    Ok(())
}
