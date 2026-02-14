use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    direction: &str,
    amount: i64,
    selector: Option<&str>,
) -> Result<(), WbError> {
    let pixels = match direction {
        "down" => amount,
        "up" => -amount,
        _ => return Err(WbError::general("Direction must be 'up' or 'down'")),
    };

    let script = if let Some(sel) = selector {
        // Critical fix: return error instead of fallback
        let sel_json = serde_json::to_string(sel)
            .map_err(|e| WbError::general(format!("Invalid selector: {e}")))?;
        format!(
            "(() => {{ const el = document.querySelector({sel_json}); \
             if (!el) return 'Element not found'; \
             el.scrollBy(0, {pixels}); \
             return 'ok'; }})()"
        )
    } else {
        format!("(() => {{ window.scrollBy(0, {pixels}); return 'ok'; }})()")
    };

    let body = serde_json::json!({
        "session": session,
        "script": script,
    });
    let resp = client.post("/execute", &body)?;

    resp.check_success("Scroll execution failed")?;

    // High fix: check JS result for element-not-found error
    if let Some(result) = resp.body.get("result") {
        let result_str = result.as_str().unwrap_or("");
        if result_str == "Element not found" {
            return Err(WbError::general(format!(
                "Element not found: {}", selector.unwrap_or("?")
            )));
        }
    }

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let abs = amount.unsigned_abs();
        let target = selector.unwrap_or("page");
        output::print_result(opts, &format!("Scrolled {direction} {abs}px on '{target}' [{} ms]", resp.elapsed_ms));
    }
    Ok(())
}
