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

    let success = resp
        .body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if success {
        let username = resp
            .body
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let already = resp
            .body
            .get("already_logged_in")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let steps = resp
            .body
            .get("extra_steps_completed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if already {
            output::print_result(
                opts,
                &format!(
                    "Session '{name}' already logged in as {username} [{} ms]",
                    resp.elapsed_ms
                ),
            );
        } else {
            let steps_note = if steps > 0 {
                format!(" ({steps} extra steps)")
            } else {
                String::new()
            };
            output::print_result(
                opts,
                &format!(
                    "Session '{name}' logged in as {username}{steps_note} [{} ms]",
                    resp.elapsed_ms
                ),
            );
        }
    } else {
        let err_obj = resp.body.get("error");
        let msg = err_obj
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .or_else(|| resp.body.get("message").and_then(|v| v.as_str()))
            .unwrap_or("Login failed");
        let next = err_obj
            .and_then(|e| e.get("next_action"))
            .and_then(|v| v.as_str());
        let code = err_obj
            .and_then(|e| e.get("code"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        eprintln!("Login failed [{}]: {msg}", code);
        if let Some(action) = next {
            eprintln!("Hint: {action}");
        }
        // Print page_state diagnostics — critical for AI-driven TOML authoring
        if let Some(ps) = err_obj.and_then(|e| e.get("page_state")) {
            if let Some(url) = ps.get("url").and_then(|v| v.as_str()) {
                eprintln!("Page URL: {url}");
            }
            if let Some(title) = ps.get("title").and_then(|v| v.as_str()) {
                eprintln!("Page title: {title}");
            }
            if let Some(subs) = ps.get("submit_candidates").and_then(|v| v.as_array()) {
                if !subs.is_empty() {
                    eprintln!("Submit candidates:");
                    for s in subs {
                        let tag = s.get("tag").and_then(|v| v.as_str()).unwrap_or("?");
                        let typ = s.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let cls = s.get("class").and_then(|v| v.as_str()).unwrap_or("");
                        let text = s.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let disabled = s.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);
                        eprintln!(
                            "  <{tag}> type={typ} name={name:?} id={id:?} class={cls:?} text={text:?}{}",
                            if disabled { " [disabled]" } else { "" }
                        );
                    }
                }
            }
            if let Some(els) = ps.get("form_elements").and_then(|v| v.as_array()) {
                let visible: Vec<_> = els
                    .iter()
                    .filter(|e| e.get("visible").and_then(|v| v.as_bool()).unwrap_or(false))
                    .collect();
                if !visible.is_empty() {
                    eprintln!("Visible form elements ({}):", visible.len());
                    for e in visible.iter().take(8) {
                        let tag = e.get("tag").and_then(|v| v.as_str()).unwrap_or("?");
                        let typ = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let ph = e.get("placeholder").and_then(|v| v.as_str()).unwrap_or("");
                        let text = e.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        eprintln!(
                            "  <{tag}> type={typ} name={name:?} id={id:?} placeholder={ph:?} text={text:?}"
                        );
                    }
                }
            }
        }
        return Err(WbError::general(format!(
            "Auto-login failed for session '{name}'"
        )));
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
    let configured = b
        .get("configured")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let source = b
        .get("config_source")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let last_result = b.get("last_result").and_then(|v| v.as_str());
    let last_at = b.get("last_at").and_then(|v| v.as_str());
    let last_username = b.get("last_username").and_then(|v| v.as_str());
    let failures = b
        .get("consecutive_failures")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let attempts = b.get("attempt_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let next_action = b.get("next_action").and_then(|v| v.as_str()).unwrap_or("");

    println!("Session:    {name}");
    println!(
        "Configured: {} ({})",
        if configured { "yes" } else { "no" },
        source
    );

    if let Some(result) = last_result {
        let symbol = match result {
            "success" | "already_logged_in" => "✓",
            "failed" => "✗",
            _ => "?",
        };
        println!(
            "Last login: {symbol} {result}{}",
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

    println!(
        "{:<20} {:<12} {:<18} {:<20}",
        "Session", "Configured", "Last Result", "Username"
    );
    println!("{}", "-".repeat(72));

    for s in sessions {
        let name = s.get("session").and_then(|v| v.as_str()).unwrap_or("?");
        let configured = s
            .get("configured")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let last_result = s.get("last_result").and_then(|v| v.as_str()).unwrap_or("-");
        let username = s
            .get("last_username")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let failures = s
            .get("consecutive_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let result_display = if failures > 0 {
            format!("{last_result} ({failures}x fail)")
        } else {
            last_result.to_string()
        };

        println!(
            "{:<20} {:<12} {:<18} {:<20}",
            output::truncate_str(name, 20),
            if configured { "yes" } else { "no" },
            output::truncate_str(&result_display, 18),
            output::truncate_str(username, 20),
        );
    }

    println!();
    output::print_result(
        opts,
        &format!("{} session(s) [{} ms]", sessions.len(), resp.elapsed_ms),
    );
    Ok(())
}

/// GET /session/auto-login/config?name=<session> — show config
pub fn config_get(client: &WbClient, opts: &OutputOpts, name: &str) -> Result<(), WbError> {
    let resp = client.get(&format!(
        "/session/auto-login/config?name={}",
        urlencoding(name)
    ))?;

    if opts.json
        || resp
            .body
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        output::print_json(&resp.body);
        return Ok(());
    }

    let msg = resp
        .body
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("Not configured");
    eprintln!("Error: {msg}");
    Err(WbError::general(format!(
        "No auto-login config for session '{name}'"
    )))
}

/// PUT /session/auto-login/config — set config from file or stdin
pub fn config_set(
    client: &WbClient,
    opts: &OutputOpts,
    name: &str,
    file: &str,
) -> Result<(), WbError> {
    let raw = if file == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| WbError::general(format!("Failed to read stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(file)
            .map_err(|e| WbError::general(format!("Failed to read {file}: {e}")))?
    };

    // Parse JSON and inject session name
    let mut config: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| WbError::general(format!("Invalid JSON: {e}")))?;
    config["name"] = serde_json::Value::String(name.to_string());

    let resp = client.put("/session/auto-login/config", &config)?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let success = resp
        .body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if success {
        let path = resp
            .body
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        output::print_result(
            opts,
            &format!(
                "Config saved for session '{name}' → {path} [{} ms]",
                resp.elapsed_ms
            ),
        );
        Ok(())
    } else {
        let msg = resp
            .body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("Failed");
        eprintln!("Error: {msg}");
        Err(WbError::general(format!(
            "Failed to set config for session '{name}'"
        )))
    }
}

/// POST /login/setup — AI-powered: analyze login form + 1Password search → auto_login.toml
pub fn setup(
    client: &WbClient,
    opts: &OutputOpts,
    name: &str,
    url: Option<&str>,
    op_item: Option<&str>,
) -> Result<(), WbError> {
    let mut body = serde_json::json!({ "session": name });
    if let Some(u) = url {
        body["url"] = serde_json::Value::String(u.to_string());
    }
    if let Some(item) = op_item {
        body["op_item"] = serde_json::Value::String(item.to_string());
    }

    let resp = client.post("/login/setup", &body)?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let success = resp
        .body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let msg = resp
            .body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("Setup failed");
        eprintln!("Error: {msg}");
        return Err(WbError::general(format!(
            "login setup failed for session '{name}'"
        )));
    }

    let config_path = resp
        .body
        .get("config_path")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let ai_used = resp
        .body
        .get("ai_used")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let op_available = resp
        .body
        .get("op_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ai_notes = resp
        .body
        .get("ai_notes")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let cfg = &resp.body["config"];
    let login_url = cfg.get("login_url").and_then(|v| v.as_str()).unwrap_or("");
    let username_sel = cfg
        .get("username_selector")
        .and_then(|v| v.as_str())
        .unwrap_or("(not found)");
    let password_sel = cfg
        .get("password_selector")
        .and_then(|v| v.as_str())
        .unwrap_or("(not found)");
    let submit_sel = cfg
        .get("submit_selector")
        .and_then(|v| v.as_str())
        .unwrap_or("(not found)");
    let logged_in_sel = cfg
        .get("logged_in_selector")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let op_item_resolved = cfg.get("op_item").and_then(|v| v.as_str()).unwrap_or("");

    let mode = match (ai_used, op_available) {
        (true, true) => "AI + 1Password",
        (true, false) => "AI only (1Password not found)",
        (false, true) => "1Password only (AI disabled)",
        (false, false) => "manual (AI and 1Password both unavailable)",
    };

    println!("Auto-login config generated [{} ms]", resp.elapsed_ms);
    println!("  Session: {name}");
    println!("  Mode:    {mode}");
    if !login_url.is_empty() {
        println!("  Page:    {login_url}");
    }
    println!();
    let is_multi_step = cfg
        .get("multi_step")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    println!("Selectors (step 1 — username page):");
    println!("  username: {username_sel}");
    if !password_sel.is_empty() {
        println!("  password: {password_sel}");
    }
    println!("  submit:   {submit_sel}");
    if !logged_in_sel.is_empty() {
        println!("  logged-in check: {logged_in_sel}");
    } else {
        println!("  logged-in check: (not detected — add manually for reliability)");
    }

    // Show extra_steps for multi-step login
    if let Some(steps) = cfg.get("extra_steps").and_then(|v| v.as_array()) {
        if !steps.is_empty() {
            for (i, step) in steps.iter().enumerate() {
                let wait = step
                    .get("wait_url_contains")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let pwd = step
                    .get("password_selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let sub = step
                    .get("submit_selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let done = step
                    .get("done_selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!();
                println!("Selectors (step {} — password page):", i + 2);
                if !wait.is_empty() {
                    println!("  wait for URL containing: {wait}");
                }
                if !pwd.is_empty() {
                    println!("  password: {pwd}");
                }
                println!("  submit:   {sub}");
                if !done.is_empty() {
                    println!("  done check: {done}");
                }
            }
        } else if is_multi_step {
            println!();
            println!(
                "  (multi-step detected but extra_steps could not be generated — edit TOML manually)"
            );
        }
    }

    if !ai_notes.is_empty() {
        println!();
        println!("AI notes: {ai_notes}");
    }

    // Show 1Password candidates
    if let Some(candidates) = resp.body.get("op_candidates").and_then(|v| v.as_array()) {
        if !candidates.is_empty() {
            println!();
            println!("1Password candidates:");
            for c in candidates.iter().take(5) {
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let title = c.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                let marker = if id == op_item_resolved || title == op_item_resolved {
                    " ← selected"
                } else {
                    ""
                };
                println!("  {title} ({id}){marker}");
            }
        }
    }

    if !op_item_resolved.is_empty() {
        println!();
        println!("1Password item: {op_item_resolved}");
    }

    println!();
    println!("Config saved: {config_path}");
    println!();
    println!("Next steps:");
    println!("  wb login run {name}        # Test auto-login now");
    println!("  wb login status {name}     # Check login status");
    println!("  wb login config-get {name} # Review the full config");

    Ok(())
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}
