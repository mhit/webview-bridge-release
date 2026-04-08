use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

/// JavaScript to inject via /execute that captures interactive elements (dom mode)
fn snapshot_script(all: bool, within: Option<&str>, limit: usize) -> String {
    // Escape the selector as a JSON string literal to prevent JS injection
    let scope_selector_json = serde_json::to_string(within.unwrap_or("body"))
        .expect("serde_json::to_string cannot fail on valid UTF-8 str");
    let element_selectors = if all {
        r#"'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="radio"],[onclick],[tabindex]:not([tabindex="-1"]),h1,h2,h3,h4,h5,h6,p,li,img,table,th,td,label,span[class],div[class]'"#
    } else {
        r#"'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="radio"],[onclick],[tabindex]:not([tabindex="-1"])'"#
    };

    format!(
        r#"(function(){{
  document.querySelectorAll('[data-wb-ref]').forEach(el => el.removeAttribute('data-wb-ref'));
  const scope = document.querySelector({scope_selector_json}) || document.body;
  const sels = {element_selectors};
  const els = [...scope.querySelectorAll(sels)].filter(el => {{
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) return false;
    const s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden';
  }}).slice(0, {limit});
  let id = 1;
  const results = els.map(el => {{
    const ref = 'e' + id++;
    el.setAttribute('data-wb-ref', ref);
    return {{
      ref, tag: el.tagName.toLowerCase(),
      type: el.type || null,
      role: el.getAttribute('role'),
      text: (el.textContent||'').trim().slice(0,80),
      name: el.getAttribute('name') || el.getAttribute('aria-label'),
      href: el.href || null,
      value: el.value || null,
      placeholder: el.placeholder || null,
      checked: el.checked === true ? true : null,
      disabled: el.disabled === true ? true : null
    }};
  }});
  return JSON.stringify({{ title: document.title, url: location.href, elements: results }});
}})()"#
    )
}

/// Format AX elements for terminal output.
///
/// Output format (from Plans.md):
/// ```
/// - button  @e1  "ログイン"
/// - textbox @e2  "メールアドレス"  (required)
/// - link    @e4  "パスワードを忘れた方"
/// ```
fn format_ax_text(snap: &serde_json::Value) -> String {
    let title = snap.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
    let url = snap.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let elements = snap.get("elements").and_then(|v| v.as_array());

    let mut text = String::new();
    text.push_str(&format!("Page: {title}\n"));
    text.push_str(&format!("URL: {url}\n\n"));

    let count = if let Some(els) = elements {
        for el in els {
            let ref_id = el.get("ref").and_then(|v| v.as_str()).unwrap_or("?");
            let role = el.get("role").and_then(|v| v.as_str()).unwrap_or("?");
            let name = el.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = el.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let value = el.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let disabled = el.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let required = el.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
            let checked = el.get("checked").and_then(|v| v.as_bool());
            let expanded = el.get("expanded").and_then(|v| v.as_bool());
            let level = el.get("level").and_then(|v| v.as_u64());
            let cursor = el.get("cursorInteractive").and_then(|v| v.as_bool()).unwrap_or(false);
            let frame_id = el.get("frameId").and_then(|v| v.as_str()).unwrap_or("");

            // "- button  @e1  "ログイン""
            let role_display = if let Some(lvl) = level {
                format!("{role}{lvl}")
            } else {
                role.to_string()
            };

            // Pad role to 10 chars for alignment
            let role_padded = format!("{role_display:<10}");
            let ref_padded = format!("@{ref_id:<5}");

            // Primary label: name > value > description
            let label = if !name.is_empty() {
                name
            } else if !value.is_empty() {
                value
            } else if !description.is_empty() {
                description
            } else {
                "(unnamed)"
            };
            let label_display = output::truncate_str(label, 60);

            let mut line = format!("- {role_padded} {ref_padded} \"{label_display}\"");

            // Flags
            let mut flags: Vec<&str> = Vec::new();
            if disabled { flags.push("disabled"); }
            if required { flags.push("required"); }
            if checked == Some(true) { flags.push("checked"); }
            if expanded == Some(false) { flags.push("collapsed"); }
            if expanded == Some(true) { flags.push("expanded"); }
            if cursor { flags.push("cursor-interactive"); }

            if !flags.is_empty() {
                let f = flags.join(", ");
                line.push_str(&format!("  ({f})"));
            }

            // Iframe annotation
            if !frame_id.is_empty() {
                // Show just the last segment of a long frameId for readability
                let frame_short = if frame_id.len() > 20 {
                    &frame_id[frame_id.len() - 20..]
                } else {
                    frame_id
                };
                line.push_str(&format!("  [frame:{frame_short}]"));
            }

            text.push_str(&line);
            text.push('\n');
        }
        els.len()
    } else {
        0
    };

    text.push_str(&format!("\n--- {count} elements (ax mode) ---\n"));
    text
}

/// Format DOM elements for terminal output (existing dom mode).
fn format_dom_text(snap: &serde_json::Value) -> (String, usize) {
    let title = snap.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
    let url = snap.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let elements = snap.get("elements").and_then(|v| v.as_array());

    let mut text = String::new();
    text.push_str(&format!("Page: {title}\n"));
    text.push_str(&format!("URL: {url}\n\n"));

    let count = if let Some(els) = elements {
        for el in els {
            let ref_id = el.get("ref").and_then(|v| v.as_str()).unwrap_or("?");
            let tag = el.get("tag").and_then(|v| v.as_str()).unwrap_or("?");
            let role = el.get("role").and_then(|v| v.as_str());
            let el_type = el.get("type").and_then(|v| v.as_str());
            let name = el.get("name").and_then(|v| v.as_str());
            let txt = el.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let href = el.get("href").and_then(|v| v.as_str());
            let value = el.get("value").and_then(|v| v.as_str());
            let placeholder = el.get("placeholder").and_then(|v| v.as_str());
            let checked = el.get("checked").and_then(|v| v.as_bool()).unwrap_or(false);
            let disabled = el.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut desc = format!("[{ref_id}] ");

            let display_role = role.unwrap_or(tag);
            if let Some(t) = el_type {
                if t != "submit" && t != "button" {
                    desc.push_str(&format!("{display_role}[{t}]"));
                } else {
                    desc.push_str(display_role);
                }
            } else {
                desc.push_str(display_role);
            }

            let label = name
                .filter(|s| !s.is_empty())
                .or_else(|| if !txt.is_empty() { Some(txt) } else { None })
                .or(placeholder);
            if let Some(l) = label {
                let l_display = output::truncate_str(l, 60);
                desc.push_str(&format!(" \"{l_display}\""));
            }

            if let Some(v) = value {
                if !v.is_empty() {
                    let v_display = output::truncate_str(v, 40);
                    desc.push_str(&format!(" val=\"{v_display}\""));
                }
            }

            if let Some(h) = href {
                if !h.is_empty() && !h.starts_with("javascript:") {
                    let h_display = output::truncate_str(h, 60);
                    desc.push_str(&format!(" href={h_display}"));
                }
            }

            if checked { desc.push_str(" [checked]"); }
            if disabled { desc.push_str(" [disabled]"); }

            text.push_str(&desc);
            text.push('\n');
        }
        els.len()
    } else {
        0
    };

    (text, count)
}

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    format: &str,
    all: bool,
    within: Option<&str>,
    limit: usize,
    output_path: Option<&str>,
    frame: Option<&str>,
) -> Result<(), WbError> {
    // AX mode: POST /snapshot with format=ax
    if format == "ax" {
        return run_ax(client, opts, session, output_path);
    }

    // DOM mode (default, backward compatible): JS via /execute
    let script = snapshot_script(all, within, limit);
    let body = serde_json::json!({
        "session": session,
        "script": script,
        "frame": frame,
    });
    let resp = client.post("/execute", &body)?;

    resp.check_success("Snapshot execution failed")?;

    // Parse the JS result — server may return pre-parsed JSON object or a JSON string
    let result_val = resp
        .body
        .get("result")
        .ok_or_else(|| WbError::general("No result from snapshot script"))?;
    let snap: serde_json::Value = if let Some(s) = result_val.as_str() {
        serde_json::from_str(s)
            .map_err(|e| WbError::general(format!("Snapshot parse error: {e}")))?
    } else {
        result_val.clone()
    };

    if opts.json {
        output::print_json(&snap);
        return Ok(());
    }

    let (mut text, count) = format_dom_text(&snap);
    text.push_str(&format!("\n--- {count} interactive elements ---\n"));

    save_or_print(opts, output_path, session, &text, &resp, count)
}

/// AX mode: POST /snapshot with format=ax, then format the AX tree for terminal.
fn run_ax(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    output_path: Option<&str>,
) -> Result<(), WbError> {
    let body = serde_json::json!({
        "session": session,
        "format": "ax",
    });
    let resp = client.post("/snapshot", &body)?;
    resp.check_success("AX snapshot failed")?;

    let snap = resp
        .body
        .get("snapshot")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let text = format_ax_text(&snap);
    let elem_count = snap
        .get("elements")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    save_or_print(opts, output_path, session, &text, &resp, elem_count)
}

/// Common output: print to stdout or save to file.
fn save_or_print(
    opts: &OutputOpts,
    output_path: Option<&str>,
    session: &str,
    text: &str,
    resp: &crate::client::WbResponse,
    count: usize,
) -> Result<(), WbError> {
    if opts.no_file {
        print!("{text}");
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p).map_err(WbError::general)?;
        std::fs::write(&path, text)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_text("snapshots", "snap", session, text)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(opts, &format!("{count} elements [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);

    Ok(())
}
