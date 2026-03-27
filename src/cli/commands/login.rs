use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

/// POST /session/auto-login — perform auto-login via 1Password
pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    name: &str,
    force: bool,
    op_item: Option<&str>,
) -> Result<(), WbError> {
    let mut body = serde_json::json!({ "name": name, "force": force });
    if let Some(item) = op_item {
        body["op_item"] = serde_json::Value::String(item.to_string());
    }

    let resp = client.post("/session/auto-login", &body)?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let success = resp.body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if success {
        let username = resp.body.get("username").and_then(|v| v.as_str()).unwrap_or("?");
        let already = resp.body.get("already_logged_in").and_then(|v| v.as_bool()).unwrap_or(false);
        let steps = resp.body.get("extra_steps_completed").and_then(|v| v.as_u64()).unwrap_or(0);
        if already {
            output::print_result(opts, &format!("Session '{name}' already logged in as {username} [{} ms]", resp.elapsed_ms));
        } else {
            let steps_note = if steps > 0 { format!(" ({steps} extra steps)") } else { String::new() };
            output::print_result(opts, &format!("Session '{name}' logged in as {username}{steps_note} [{} ms]", resp.elapsed_ms));
        }
    } else {
        let msg = resp.body
            .get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str())
            .or_else(|| resp.body.get("message").and_then(|v| v.as_str()))
            .unwrap_or("Login failed");
        let next = resp.body
            .get("error").and_then(|e| e.get("next_action")).and_then(|v| v.as_str());
        eprintln!("Login failed: {msg}");
        if let Some(action) = next {
            eprintln!("Next action: {action}");
        }
        return Err(WbError::general(format!("Auto-login failed for session '{name}'")));
    }

    Ok(())
}

/// GET /session/auto-login?name=<session> — show login status
pub fn status(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let resp = client.get(&format!("/session/auto-login?name={}", urlencoding(name)))?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let b = &resp.body;
    let configured = b.get("configured").and_then(|v| v.as_bool()).unwrap_or(false);
    let source = b.get("config_source").and_then(|v| v.as_str()).unwrap_or("none");
    let last_result = b.get("last_result").and_then(|v| v.as_str());
    let last_at = b.get("last_at").and_then(|v| v.as_str());
    let last_username = b.get("last_username").and_then(|v| v.as_str());
    let failures = b.get("consecutive_failures").and_then(|v| v.as_u64()).unwrap_or(0);
    let attempts = b.get("attempt_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let next_action = b.get("next_action").and_then(|v| v.as_str()).unwrap_or("");

    println!("Session:    {name}");
    println!("Configured: {} ({})", if configured { "yes" } else { "no" }, source);

    if let Some(result) = last_result {
        let symbol = match result {
            "success" | "already_logged_in" => "✓",
            "failed" => "✗",
            _ => "?",
        };
        println!("Last login: {symbol} {result}{}",
            last_at.map(|t| format!(" at {t}")).unwrap_or_default()
        );
        if let Some(u) = last_username {
            println!("Username:   {u}");
        }
        if failures > 0 {
            println!("Failures:   {failures} consecutive ({attempts} total)");
        } else {
            println!("Attempts:   {attempts} total");
        }
    } else {
        println!("Last login: (never attempted)");
    }

    if !next_action.is_empty() {
        println!("Next:       {next_action}");
    }

    Ok(())
}

/// GET /session/auto-login/list — list all sessions' login status
pub fn list(client: &WbClient, opts: &OutputOpts) -> Result<(), WbError> {
    let resp = client.get("/session/auto-login/list")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let sessions = resp.body.get("sessions").and_then(|v| v.as_array());
    let Some(sessions) = sessions else {
        output::print_result(opts, "No sessions found");
        return Ok(());
    };

    if sessions.is_empty() {
        output::print_result(opts, "No sessions found");
        return Ok(());
    }

    println!("{:<20} {:<12} {:<18} {:<20}", "Session", "Configured", "Last Result", "Username");
    println!("{}", "-".repeat(72));

    for s in sessions {
        let name = s.get("session").and_then(|v| v.as_str()).unwrap_or("?");
        let configured = s.get("configured").and_then(|v| v.as_bool()).unwrap_or(false);
        let last_result = s.get("last_result").and_then(|v| v.as_str()).unwrap_or("-");
        let username = s.get("last_username").and_then(|v| v.as_str()).unwrap_or("-");
        let failures = s.get("consecutive_failures").and_then(|v| v.as_u64()).unwrap_or(0);

        let result_display = if failures > 0 {
            format!("{last_result} ({failures}x fail)")
        } else {
            last_result.to_string()
        };

        println!("{:<20} {:<12} {:<18} {:<20}",
            output::truncate_str(name, 20),
            if configured { "yes" } else { "no" },
            output::truncate_str(&result_display, 18),
            output::truncate_str(username, 20),
        );
    }

    println!();
    output::print_result(opts, &format!("{} session(s) [{} ms]", sessions.len(), resp.elapsed_ms));
    Ok(())
}

/// GET /session/auto-login/config?name=<session> — show config
pub fn config_get(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let resp = client.get(&format!("/session/auto-login/config?name={}", urlencoding(name)))?;

    if opts.json || resp.body.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        output::print_json(&resp.body);
        return Ok(());
    }

    let msg = resp.body.get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str())
        .unwrap_or("Not configured");
    eprintln!("Error: {msg}");
    Err(WbError::general(format!("No auto-login config for session '{name}'")))
}

/// PUT /session/auto-login/config — set config from file or stdin
pub fn config_set(client: &WbClient, opts: &OutputOpts, name: &str, file: &str) -> Result<(), WbError> {
    let raw = if file == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)
            .map_err(|e| WbError::general(format!("Failed to read stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(file)
            .map_err(|e| WbError::general(format!("Failed to read {file}: {e}")))?
    };

    // Parse JSON and inject session name
    let mut config: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| WbError::general(format!("Invalid JSON: {e}")))?;
    config["name"] = serde_json::Value::String(name.to_string());

    let resp = client.put("/session/auto-login/config", &config)?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let success = resp.body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if success {
        let path = resp.body.get("path").and_then(|v| v.as_str()).unwrap_or("?");
        output::print_result(opts, &format!("Config saved for session '{name}' → {path} [{} ms]", resp.elapsed_ms));
        Ok(())
    } else {
        let msg = resp.body.get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str())
            .unwrap_or("Failed");
        eprintln!("Error: {msg}");
        Err(WbError::general(format!("Failed to set config for session '{name}'")))
    }
}

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c.to_string() }
        else { format!("%{:02X}", c as u32) }
    }).collect()
}
