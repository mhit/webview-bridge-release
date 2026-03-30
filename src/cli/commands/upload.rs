use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use base64::{Engine as _, engine::general_purpose::STANDARD};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    file_path: &str,
    filename_override: Option<&str>,
) -> Result<(), WbError> {
    // Read file from local filesystem
    let data = std::fs::read(file_path)
        .map_err(|e| WbError::general(format!("Failed to read file '{}': {e}", file_path)))?;

    let filename = filename_override.unwrap_or_else(|| {
        std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload")
    });

    let data_b64 = STANDARD.encode(&data);

    let body = serde_json::json!({
        "session": session,
        "filename": filename,
        "data": data_b64,
    });

    let resp = client.post("/upload/base64", &body)?;
    resp.check_success("Upload failed")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let stored = resp
        .body
        .get("stored_filename")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let size = resp.body.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    let url = resp.body.get("url").and_then(|v| v.as_str()).unwrap_or("?");

    output::print_result(
        opts,
        &format!(
            "Uploaded: {} ({}) [{} ms]\n  URL: {}",
            stored,
            format_bytes(size),
            resp.elapsed_ms,
            url
        ),
    );
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
