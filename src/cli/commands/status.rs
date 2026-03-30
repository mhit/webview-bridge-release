use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(client: &WbClient, opts: &OutputOpts) -> Result<(), WbError> {
    let health = client.get("/health")?;
    let sessions = client.get("/session/list")?;

    if opts.json {
        let combined = serde_json::json!({
            "health": health.body,
            "sessions": sessions.body,
        });
        output::print_json(&combined);
        return Ok(());
    }

    // Server status
    let status = health
        .body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = health
        .body
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    output::print_result(
        opts,
        &format!("Server: {status} (v{version}) [{} ms]", health.elapsed_ms),
    );

    // Sessions
    if let Some(arr) = sessions.body.get("sessions").and_then(|v| v.as_array()) {
        if arr.is_empty() {
            output::print_result(opts, "Sessions: (none)");
        } else {
            output::print_result(opts, &format!("Sessions: {} active", arr.len()));
            for s in arr {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let url = s.get("current_url").and_then(|v| v.as_str()).unwrap_or("");
                let url_display = output::truncate_str(url, 60);
                output::print_result(opts, &format!("  {name}: {url_display}"));
            }
        }
    }

    Ok(())
}
