use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use crate::refs;

const VALID_CONDITIONS: &[&str] = &[
    "present", "visible", "stable", "text_contains", "text_matches",
    "attribute_equals", "clickable", "detached", "navigation_complete", "network_idle",
];

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    selector: &str,
    condition: &str,
    timeout_ms: u64,
    text: Option<&str>,
) -> Result<(), WbError> {
    if !VALID_CONDITIONS.contains(&condition) {
        return Err(WbError::general(format!(
            "Invalid condition '{}'. Valid: {}", condition, VALID_CONDITIONS.join(", ")
        )));
    }

    let resolved = refs::resolve_target(selector);

    let mut body = serde_json::json!({
        "session": session,
        "selector": resolved,
        "condition": condition,
        "timeout_ms": timeout_ms,
    });

    if let Some(t) = text {
        body["text"] = serde_json::Value::String(t.to_string());
    }

    let resp = client.post("/wait", &body)?;
    resp.check_success("Wait failed")?;

    // Server returns "found" field — true if condition met, false if timed out
    let found = resp.body.get("found").and_then(|v| v.as_bool()).unwrap_or(false);

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let status = if found { "found" } else { "timed out" };
        output::print_result(opts, &format!("Wait '{condition}' on '{selector}': {status} [{} ms]", resp.elapsed_ms));
    }

    if found {
        Ok(())
    } else {
        Err(WbError::timeout(format!("Wait '{condition}' on '{selector}' timed out after {timeout_ms}ms")))
    }
}
