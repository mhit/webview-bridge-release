use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

/// JavaScript to inject via /execute that captures interactive elements
fn snapshot_script(all: bool, within: Option<&str>, limit: usize) -> String {
    // Escape the selector as a JSON string literal to prevent JS injection
    let scope_selector_json = serde_json::to_string(within.unwrap_or("body"))
        .unwrap_or_else(|_| "\"body\"".to_string());
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

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    all: bool,
    within: Option<&str>,
    limit: usize,
    output_path: Option<&str>,
) -> Result<(), WbError> {
    let script = snapshot_script(all, within, limit);
    let body = serde_json::json!({
        "session": session,
        "script": script,
    });
    let resp = client.post("/execute", &body)?;

    // Parse the JS result
    let result_str = resp.body.get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WbError::general("No result from snapshot script"))?;

    let snap: serde_json::Value = serde_json::from_str(result_str)
        .map_err(|e| WbError::general(format!("Snapshot parse error: {e}")))?;

    if opts.json {
        output::print_json(&snap);
        return Ok(());
    }

    // Build text output
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

            // Build element descriptor
            let mut desc = format!("[{ref_id}] ");

            // Tag with type qualifier
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

            // Label: prefer name > text > placeholder
            let label = name
                .filter(|s| !s.is_empty())
                .or_else(|| if !txt.is_empty() { Some(txt) } else { None })
                .or(placeholder);
            if let Some(l) = label {
                let l_display = output::truncate_str(l, 60);
                desc.push_str(&format!(" \"{l_display}\""));
            }

            // Value for inputs
            if let Some(v) = value {
                if !v.is_empty() {
                    let v_display = output::truncate_str(v, 40);
                    desc.push_str(&format!(" val=\"{v_display}\""));
                }
            }

            // Href for links
            if let Some(h) = href {
                if !h.is_empty() && !h.starts_with("javascript:") {
                    let h_display = output::truncate_str(h, 60);
                    desc.push_str(&format!(" href={h_display}"));
                }
            }

            // Flags
            if checked { desc.push_str(" [checked]"); }
            if disabled { desc.push_str(" [disabled]"); }

            text.push_str(&desc);
            text.push('\n');
        }
        els.len()
    } else {
        0
    };

    text.push_str(&format!("\n--- {count} interactive elements ---\n"));

    if opts.no_file {
        print!("{text}");
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p)
            .map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &text)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_text("snapshots", "snap", session, &text)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(opts, &format!("{count} elements [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);

    Ok(())
}
