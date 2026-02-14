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
        let path = std::path::PathBuf::from(p);
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

/// Simple base64 decoder (avoids pulling in the base64 crate for CLI)
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Strip data URI prefix if present
    let data = if let Some(pos) = input.find(",") {
        &input[pos + 1..]
    } else {
        input
    };

    // Simple base64 decode using lookup table
    let table: [u8; 128] = {
        let mut t = [255u8; 128];
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            t[alphabet[i] as usize] = i as u8;
            i += 1;
        }
        t
    };

    let clean: Vec<u8> = data.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);

    for chunk in clean.chunks(4) {
        let mut buf = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b >= 128 || table[b as usize] == 255 {
                return Err(format!("Invalid base64 character: {}", b as char));
            }
            buf[i] = table[b as usize];
        }
        let n = chunk.len();
        if n >= 2 { out.push((buf[0] << 2) | (buf[1] >> 4)); }
        if n >= 3 { out.push((buf[1] << 4) | (buf[2] >> 2)); }
        if n >= 4 { out.push((buf[2] << 6) | buf[3]); }
    }

    Ok(out)
}
