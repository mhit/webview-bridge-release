use serde_json::Value;
use std::fmt;
use std::time::Instant;

pub struct WbClient {
    base_url: String,
    token: Option<String>,
}

#[derive(Debug)]
pub struct WbError {
    pub message: String,
    code: u8,
}

impl WbError {
    pub fn connection(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), code: 2 }
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), code: 3 }
    }

    pub fn session(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), code: 4 }
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), code: 5 }
    }

    pub fn general(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), code: 1 }
    }

    pub fn exit_code(&self) -> u8 {
        self.code
    }
}

impl fmt::Display for WbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub struct WbResponse {
    pub body: Value,
    pub elapsed_ms: u64,
}

impl WbClient {
    pub fn new(base_url: &str, token: Option<&str>) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let token = token.map(|t| t.to_string()).or_else(|| Self::read_token_file());
        Self { base_url, token }
    }

    /// Read token from $DATA_DIR/cli-token file
    fn read_token_file() -> Option<String> {
        let data_dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
            .map(std::path::PathBuf::from)
            .ok()
            .or_else(|| dirs::data_dir().map(|d| d.join("webview-bridge")))?;
        let token_path = data_dir.join("cli-token");
        std::fs::read_to_string(token_path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn get(&self, path: &str) -> Result<WbResponse, WbError> {
        let url = format!("{}{}", self.base_url, path);
        let start = Instant::now();
        let mut req = ureq::get(&url);
        if let Some(ref token) = self.token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        let resp = req.call().map_err(|e| Self::map_ureq_error(e))?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let body: Value = resp.into_json().map_err(|e| WbError::general(format!("JSON parse error: {e}")))?;
        Ok(WbResponse { body, elapsed_ms })
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<WbResponse, WbError> {
        let url = format!("{}{}", self.base_url, path);
        let start = Instant::now();
        let mut req = ureq::post(&url);
        if let Some(ref token) = self.token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        let resp = req.send_json(body.clone()).map_err(|e| Self::map_ureq_error(e))?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let body: Value = resp.into_json().map_err(|e| WbError::general(format!("JSON parse error: {e}")))?;
        Ok(WbResponse { body, elapsed_ms })
    }

    fn map_ureq_error(e: ureq::Error) -> WbError {
        match e {
            ureq::Error::Status(401, _) | ureq::Error::Status(403, _) => {
                WbError::auth("Authentication failed. Set WB_TOKEN or use --token.")
            }
            ureq::Error::Status(code, resp) => {
                let msg = resp.into_string().unwrap_or_default();
                if msg.contains("SESSION_NOT_FOUND") {
                    WbError::session(format!("Session not found. Run: wb session acquire"))
                } else {
                    WbError::general(format!("HTTP {code}: {msg}"))
                }
            }
            ureq::Error::Transport(t) => {
                if t.to_string().contains("timed out") {
                    WbError::timeout("Request timed out")
                } else {
                    WbError::connection(format!("Cannot connect to server: {t}"))
                }
            }
        }
    }
}
