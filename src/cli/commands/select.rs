use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use crate::refs;

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    target: &str,
    value: &str,
) -> Result<(), WbError> {
    let selector = refs::resolve_target(target);
    // Critical fix: return error instead of fallback
    let sel_json = serde_json::to_string(&selector)
        .map_err(|e| WbError::general(format!("Invalid selector: {e}")))?;
    let val_json = serde_json::to_string(value)
        .map_err(|e| WbError::general(format!("Invalid value: {e}")))?;

    let script = format!(
        r#"(() => {{
  const el = document.querySelector({sel_json});
  if (!el) return JSON.stringify({{ error: 'Element not found' }});
  if (el.tagName !== 'SELECT') return JSON.stringify({{ error: 'Not a <select> element' }});
  const opts = [...el.options].map(o => ({{ value: o.value, text: o.textContent.trim() }}));
  const val = {val_json};
  const match = opts.find(o => o.value === val || o.text === val);
  if (!match) return JSON.stringify({{ error: 'Option not found', available: opts }});
  el.value = match.value;
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return JSON.stringify({{ selected: match.value, text: match.text }});
}})()"#
    );

    let body = serde_json::json!({
        "session": session,
        "script": script,
    });
    let resp = client.post("/execute", &body)?;

    resp.check_success("Select execution failed")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    // Parse the JS result
    let result = resp.body.get("result").cloned().unwrap_or(serde_json::Value::Null);
    // Medium fix: return error instead of swallowing parse failure
    let result_obj = match &result {
        serde_json::Value::String(s) => serde_json::from_str::<serde_json::Value>(s)
            .map_err(|e| WbError::general(format!("Invalid response from select script: {e}")))?,
        other => other.clone(),
    };

    if let Some(err) = result_obj.get("error").and_then(|v| v.as_str()) {
        return Err(WbError::general(format!("Select failed: {err}")));
    }

    let selected = result_obj.get("text").and_then(|v| v.as_str()).unwrap_or(value);
    output::print_result(opts, &format!("Selected '{selected}' in '{target}' [{} ms]", resp.elapsed_ms));
    Ok(())
}
