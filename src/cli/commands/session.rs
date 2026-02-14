use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn acquire(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let body = serde_json::json!({ "name": name });
    let resp = client.post("/session/acquire", &body)?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let is_new = resp.body.get("is_new").and_then(|v| v.as_bool()).unwrap_or(false);
        let status = if is_new { "created" } else { "reused" };
        output::print_result(opts, &format!("Session '{name}' ready ({status}) [{} ms]", resp.elapsed_ms));
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

    // Server returns {"sessions": [...]} — extract the array
    if let Some(arr) = resp.body.get("sessions").and_then(|v| v.as_array()) {
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
