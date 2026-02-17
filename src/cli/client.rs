use serde_json::Value;
use std::fmt;
use std::time::{Duration, Instant};

pub struct WbClient {
    base_url: String,
    token: Option<String>,
    agent: ureq::Agent,
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

impl WbResponse {
    /// Check if the server returned `success: false` and return an error if so.
    /// Many server endpoints return HTTP 200 with `success: false` for soft errors.
    pub fn check_success(&self, fallback_msg: &str) -> Result<(), WbError> {
        if self.body.get("success").and_then(|v| v.as_bool()) == Some(false) {
            let err_msg = self.body.get("error")
                .and_then(|v| {
                    // Error can be a string or an object with a "message" field
                    v.as_str().or_else(|| v.get("message").and_then(|m| m.as_str()))
                })
                .unwrap_or(fallback_msg);
            return Err(WbError::general(err_msg));
        }
        Ok(())
    }
}

impl WbClient {
    pub fn new(base_url: &str, token: Option<&str>) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let token = token.map(|t| t.to_string()).or_else(|| Self::read_token_file());
        // Generous HTTP timeout — command-level timeouts (wait, execute)
        // govern actual operations; this just prevents hung connections.
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(300))
            .build();
        Self { base_url, token, agent }
    }

    /// Read token from $DATA_DIR/cli-token file
    fn read_token_file() -> Option<String> {
        let data_dir = std::env::var("WEBVIEW_BRIDGE_DATA_PATH")
            .map(std::path::PathBuf::from)
            .ok()
            .or_else(|| dirs::data_dir().map(|d| d.join("webview-bridge")))?;
        let token_path = data_dir.join("cli-token");

        // On Unix, warn if token file is world-readable
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(&token_path) {
                if meta.mode() & 0o077 != 0 {
                    eprintln!("Warning: {} is accessible by other users. Run: chmod 600 {}",
                        token_path.display(), token_path.display());
                }
            }
        }

        std::fs::read_to_string(token_path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn get(&self, path: &str) -> Result<WbResponse, WbError> {
        self.request(self.agent.get(&self.url(path)), None)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<WbResponse, WbError> {
        self.request(self.agent.post(&self.url(path)), Some(body))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request(&self, mut req: ureq::Request, body: Option<&Value>) -> Result<WbResponse, WbError> {
        if let Some(ref token) = self.token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        let start = Instant::now();
        let resp = if let Some(b) = body {
            req.send_json(b.clone())
        } else {
            req.call()
        }.map_err(Self::map_ureq_error)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let body: Value = resp.into_json()
            .map_err(|e| WbError::general(format!("Invalid server response: {e}")))?;
        Ok(WbResponse { body, elapsed_ms })
    }

    fn map_ureq_error(e: ureq::Error) -> WbError {
        match e {
            ureq::Error::Status(401, _) | ureq::Error::Status(403, _) => {
                WbError::auth(
                    "Authentication failed. Run: wb auth save <TOKEN>\n\
                     The token is displayed when the server starts. You can also set WB_TOKEN env var."
                )
            }
            ureq::Error::Status(code, resp) => {
                let msg = resp.into_string().unwrap_or_default();
                if msg.contains("SESSION_NOT_FOUND") {
                    WbError::session("Session not found. Run: wb session acquire <name>")
                } else if msg.contains("Session is not acquired") {
                    WbError::session("Session exists but is not acquired. Run: wb session acquire <name>")
                } else if msg.contains("SESSION_LIMIT") {
                    WbError::general("Maximum session limit reached. Release unused sessions: wb session release <name>")
                } else {
                    WbError::general(format!("HTTP {code}: {msg}"))
                }
            }
            ureq::Error::Transport(t) => {
                let detail = t.to_string();
                if detail.contains("timed out") {
                    WbError::timeout("Request timed out. The server may be overloaded. Try again or increase timeout.")
                } else {
                    WbError::connection(format!(
                        "Cannot connect to server. Is the server running? ({})\n\
                         Check: 1) Server is started  2) Host/port is correct  3) Firewall allows access",
                        detail
                    ))
                }
            }
        }
    }
}
