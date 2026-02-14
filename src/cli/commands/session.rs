use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn acquire(client: &WbClient, opts: &OutputOpts, name: &str, device: Option<&str>) -> Result<(), WbError> {
    let mut body = serde_json::json!({ "name": name });
    if let Some(d) = device {
        body["device"] = serde_json::Value::String(d.to_string());
    }
    let resp = client.post("/session/acquire", &body)?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        output::print_result(opts, &format!("Session '{name}' ready [{} ms]", resp.elapsed_ms));
        if let Some(d) = device {
            output::print_result(opts, &format!("  Device: {d}"));
        }
    }
    Ok(())
}

pub fn release(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let body = serde_json::json!({ "name": name });
    let resp = client.post("/session/release", &body)?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        output::print_result(opts, &format!("Session '{name}' released [{} ms]", resp.elapsed_ms));
    }
    Ok(())
}

pub fn list(client: &WbClient, opts: &OutputOpts) -> Result<(), WbError> {
    let resp = client.get("/session/list")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    if let Some(arr) = resp.body.as_array() {
        if arr.is_empty() {
            output::print_result(opts, "No active sessions");
        } else {
            for s in arr {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let url = s.get("current_url").and_then(|v| v.as_str()).unwrap_or("about:blank");
                output::print_result(opts, &format!("{name}\t{url}"));
            }
        }
    }
    Ok(())
}
