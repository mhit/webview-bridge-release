use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    selector: &str,
    fields: Option<&str>,
    limit: usize,
    output_path: Option<&str>,
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

    // Critical fix: return error instead of fallback on serialization failure
    let sel_json = serde_json::to_string(selector)
        .map_err(|e| WbError::general(format!("Invalid selector: {e}")))?;

    let script = if field_pairs.is_empty() {
        // No fields specified — extract text content of each matched element
        format!(
            r#"(() => {{
  const els = [...document.querySelectorAll({sel_json})].slice(0, {limit});
  return JSON.stringify(els.map((el, i) => ({{
    index: i, tag: el.tagName.toLowerCase(),
    text: (el.textContent || '').trim().slice(0, 200),
    href: el.href || null
  }})));
}})()"#
        )
    } else {
        // Fields specified — for each container, extract sub-fields
        let field_js: String = field_pairs
            .iter()
            .map(|(name, sub_sel)| {
                // Critical fix: return error instead of manual format fallback
                let name_j = serde_json::to_string(name)
                    .map_err(|e| WbError::general(format!("Invalid field name '{name}': {e}")))?;
                let sub_j = serde_json::to_string(sub_sel)
                    .map_err(|e| WbError::general(format!("Invalid sub-selector '{sub_sel}': {e}")))?;
                Ok(format!(
                    "    {name_j}: (() => {{ const s = el.querySelector({sub_j}); \
                     return s ? (s.textContent || '').trim() : null; }})()"
                ))
            })
            .collect::<Result<Vec<_>, WbError>>()?
            .join(",\n");

        format!(
            r#"(() => {{
  const els = [...document.querySelectorAll({sel_json})].slice(0, {limit});
  return JSON.stringify(els.map(el => ({{
{field_js}
  }})));
}})()"#
        )
    };

    let body = serde_json::json!({
        "session": session,
        "script": script,
    });
    let resp = client.post("/execute", &body)?;

    resp.check_success("Extract execution failed")?;

    // Parse the result
    let result_val = resp.body.get("result")
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
        let path = output::validate_output_path(p)
            .map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &data_str)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        output::save_text("extractions", "extract", session, &data_str)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(opts, &format!("{count} items extracted [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);
    Ok(())
}
