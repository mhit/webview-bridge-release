use base64::{Engine as _, engine::general_purpose::STANDARD};
use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    device: Option<&str>,
    output_path: Option<&str>,
) -> Result<(), WbError> {
    let mut body = serde_json::json!({ "session": session });
    if let Some(d) = device {
        body["device"] = serde_json::Value::String(d.to_string());
    }
    let resp = client.post("/screenshot", &body)?;

    // Server returns base64 image data
    let b64 = resp.body.get("image")
        .or_else(|| resp.body.get("data"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| WbError::general("No image data in response"))?;

    let image_data = base64_decode(b64)
        .map_err(|e| WbError::general(format!("Base64 decode error: {e}")))?;

    if opts.json {
        output::print_json(&serde_json::json!({
            "size": image_data.len(),
            "elapsed_ms": resp.elapsed_ms,
        }));
    }

    if opts.no_file {
        // Write raw PNG to stdout
        use std::io::Write;
        std::io::stdout().write_all(&image_data)
            .map_err(|e| WbError::general(format!("stdout write error: {e}")))?;
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p)
            .map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &image_data)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_binary("screenshots", "ss", session, "png", &image_data)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    let kb = image_data.len() / 1024;
    output::print_result(opts, &format!("Screenshot ({kb} KB) [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);

    Ok(())
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Strip data URI prefix if present (e.g., "data:image/png;base64,...")
    let data = if let Some(pos) = input.find(',') {
        &input[pos + 1..]
    } else {
        input
    };
    STANDARD.decode(data.trim()).map_err(|e| format!("Base64 decode error: {e}"))
}
