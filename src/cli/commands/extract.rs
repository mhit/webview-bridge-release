use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use crate::selector;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    selector: &str,
    fields: Option<&str>,
    limit: usize,
    output_path: Option<&str>,
    frame: Option<&str>,
    scroll: bool,
    scroll_max: usize,
    scroll_dedup: Option<&str>,
    scroll_delay: u64,
    scroll_amount: u32,
) -> Result<(), WbError> {
    // Build field extraction map: "title=.title,price=.price" → [["title",".title"], ...]
    let field_pairs: Vec<(&str, &str)> = if let Some(f) = fields {
        f.split(',')
            .filter_map(|pair| {
                let parts: Vec<&str> = pair.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some((parts[0].trim(), parts[1].trim()))
                } else {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let sel_js_expr = selector::to_all(selector);

    let script = build_extract_script(&sel_js_expr, &field_pairs, limit)?;

    if scroll {
        return run_scroll_collect(
            client,
            opts,
            session,
            &sel_js_expr,
            &field_pairs,
            limit,
            output_path,
            frame,
            scroll_max,
            scroll_dedup,
            scroll_delay,
            scroll_amount,
        );
    }

    let body = serde_json::json!({
        "session": session,
        "script": script,
        "frame": frame,
    });
    let resp = client.post("/execute", &body)?;

    resp.check_success("Extract execution failed")?;

    // Parse the result
    let result_val = resp
        .body
        .get("result")
        .ok_or_else(|| WbError::general("No result from extract script"))?;
    let data: serde_json::Value = match result_val {
        serde_json::Value::String(s) => serde_json::from_str(s)
            .map_err(|e| WbError::general(format!("Extract parse error: {e}")))?,
        other => other.clone(),
    };

    if opts.json {
        output::print_json(&data);
        return Ok(());
    }

    let count = data.as_array().map(|a| a.len()).unwrap_or(0);
    let data_str = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "[]".to_string());

    if opts.no_file {
        print!("{data_str}");
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p).map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &data_str)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_text("extractions", "extract", session, &data_str)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(
        opts,
        &format!("{count} items extracted [{} ms]", resp.elapsed_ms),
    );
    output::print_saved(opts, &path);
    Ok(())
}

/// Scroll-and-collect: extract → scroll → deduplicate → repeat
fn run_scroll_collect(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    sel_js_expr: &str,
    field_pairs: &[(&str, &str)],
    limit: usize,
    output_path: Option<&str>,
    frame: Option<&str>,
    scroll_max: usize,
    scroll_dedup: Option<&str>,
    scroll_delay: u64,
    scroll_amount: u32,
) -> Result<(), WbError> {
    let mut accumulated: Vec<serde_json::Value> = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut no_new_streak: u32 = 0;
    let mut total_duplicates: usize = 0;
    let start = std::time::Instant::now();

    for i in 0..scroll_max {
        // Build extraction script (no limit per iteration — collect all visible)
        let script = build_extract_script(sel_js_expr, field_pairs, 500)?;

        let body = serde_json::json!({
            "session": session,
            "script": script,
            "frame": frame,
        });
        let resp = client.post("/execute", &body)?;
        resp.check_success("Extract execution failed")?;

        let result_val = resp
            .body
            .get("result")
            .ok_or_else(|| WbError::general("No result from extract script"))?;
        let batch: serde_json::Value = match result_val {
            serde_json::Value::String(s) => serde_json::from_str(s)
                .map_err(|e| WbError::general(format!("Extract parse error: {e}")))?,
            other => other.clone(),
        };

        let mut new_count = 0usize;
        if let Some(items) = batch.as_array() {
            for item in items {
                let key = if let Some(dk) = scroll_dedup {
                    item.get(dk)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    let s = serde_json::to_string(item).unwrap_or_default();
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    s.hash(&mut hasher);
                    format!("{:x}", hasher.finish())
                };

                if key.is_empty() {
                    continue;
                }

                if seen_keys.insert(key) {
                    accumulated.push(item.clone());
                    new_count += 1;
                } else {
                    total_duplicates += 1;
                }
            }
        }

        if !opts.json {
            eprint!(
                "\r  scroll {}/{}: +{} new, {} total",
                i + 1,
                scroll_max,
                new_count,
                accumulated.len()
            );
        }

        // Early exit: 3 consecutive rounds with 0 new items
        if new_count == 0 {
            no_new_streak += 1;
            if no_new_streak >= 3 {
                break;
            }
        } else {
            no_new_streak = 0;
        }

        // Check limit
        if accumulated.len() >= limit {
            accumulated.truncate(limit);
            break;
        }

        // Scroll down (skip after last iteration)
        if i + 1 < scroll_max {
            let scroll_script = format!(
                "window.scrollBy(0, {}); JSON.stringify({{ scrolled: true }});",
                scroll_amount
            );
            let scroll_body = serde_json::json!({
                "session": session,
                "script": scroll_script,
                "frame": frame,
            });
            let _ = client.post("/execute", &scroll_body);
            std::thread::sleep(std::time::Duration::from_millis(scroll_delay));
        }
    }

    if !opts.json {
        eprintln!(); // newline after progress
    }

    let elapsed = start.elapsed().as_millis();
    let count = accumulated.len();
    let data = serde_json::Value::Array(accumulated);

    if opts.json {
        output::print_json(&serde_json::json!({
            "count": count,
            "data": data,
            "scroll_mode": true,
            "duplicates_removed": total_duplicates,
        }));
        return Ok(());
    }

    let data_str = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "[]".to_string());

    if opts.no_file {
        print!("{data_str}");
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p).map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &data_str)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_text("extractions", "extract", session, &data_str)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(
        opts,
        &format!(
            "{count} items extracted (scroll mode, {total_duplicates} dupes removed) [{elapsed} ms]"
        ),
    );
    output::print_saved(opts, &path);
    Ok(())
}

fn build_extract_script(
    sel_js_expr: &str,
    field_pairs: &[(&str, &str)],
    limit: usize,
) -> Result<String, WbError> {
    if field_pairs.is_empty() {
        Ok(format!(
            r#"(() => {{
  const els = [...{sel_js_expr}].slice(0, {limit});
  return JSON.stringify(els.map((el, i) => ({{
    index: i, tag: el.tagName.toLowerCase(),
    text: (el.textContent || '').trim().slice(0, 200),
    href: el.href || null
  }})));
}})()"#
        ))
    } else {
        let field_js: String = field_pairs
            .iter()
            .map(|(name, sub_sel)| {
                let name_j = serde_json::to_string(name)
                    .map_err(|e| WbError::general(format!("Invalid field name '{name}': {e}")))?;
                let sub_js = selector::to_single(sub_sel);
                Ok(format!(
                    "    {name_j}: (() => {{ const s = {sub_js}; \
                     return s ? (s.textContent || '').trim() : null; }})()"
                ))
            })
            .collect::<Result<Vec<_>, WbError>>()?
            .join(",\n");

        Ok(format!(
            r#"(() => {{
  const els = [...{sel_js_expr}].slice(0, {limit});
  return JSON.stringify(els.map(el => ({{
{field_js}
  }})));
}})()"#
        ))
    }
}
