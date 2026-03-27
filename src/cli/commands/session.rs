use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn acquire(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let body = serde_json::json!({ "name": name });
    let resp = client.post("/session/acquire", &body)?;
    resp.check_success("Failed to acquire session")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let is_new = resp.body.get("is_new").and_then(|v| v.as_bool()).unwrap_or(false);
        let status = if is_new { "created" } else { "reused" };
        let url = resp.body.get("current_url").and_then(|v| v.as_str());
        let logged_in = resp.body
            .get("auth_status").and_then(|a| a.get("logged_in")).and_then(|v| v.as_bool());

        let login_note = match logged_in {
            Some(true) => " [logged in]",
            Some(false) => " [not logged in]",
            None => "",
        };
        let url_note = url.map(|u| format!(" — {}", output::truncate_str(u, 60))).unwrap_or_default();

        output::print_result(opts, &format!(
            "Session '{name}' ready ({status}){login_note}{url_note} [{} ms]",
            resp.elapsed_ms
        ));

        // Show contextual hints (skip in quiet mode)
        if !opts.quiet {
            if let Some(hints) = resp.body.get("hints").and_then(|v| v.as_array()) {
                for h in hints {
                    if let Some(s) = h.as_str() {
                        // Replace placeholder with actual session name
                        let msg = s.replace("<session>", name);
                        eprintln!("  hint: {msg}");
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn release(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let body = serde_json::json!({ "name": name });
    let resp = client.post("/session/release", &body)?;
    resp.check_success("Failed to release session")?;

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
                let url_display = output::truncate_str(url, 60);
                output::print_result(opts, &format!("{name}\t{url_display}"));
            }
        }
    }
    Ok(())
}
