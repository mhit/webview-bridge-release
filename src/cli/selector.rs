/// Extended CSS selector support for wb CLI commands.
///
/// Supported extensions (mirrors webview_instance::selector_to_js_expr):
///   js:<expr>           — raw JS expression, must return an Element or null
///   <base>:has-text('T')   — first element matching base whose textContent includes T
///   <base>:text-is('T')    — first element matching base whose textContent.trim() === T
///   <base>:has-text-i('T') — case-insensitive :has-text
///
/// Standard CSS selectors pass through unchanged.

/// Convert a (possibly extended) selector to a JS expression that returns one Element or null.
pub fn to_single(selector: &str) -> String {
    if let Some(expr) = selector.strip_prefix("js:") {
        return format!("({})", expr);
    }
    if let Some((base, text)) = split_pseudo(selector, ":has-text-i(") {
        let base_json = js_str(&base);
        let text_lower = text.to_lowercase();
        let text_json = js_str(&text_lower);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).find(\
             el => el.textContent.toLowerCase().includes({text_json})) || null"
        );
    }
    if let Some((base, text)) = split_pseudo(selector, ":has-text(") {
        let base_json = js_str(&base);
        let text_json = js_str(&text);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).find(\
             el => el.textContent.includes({text_json})) || null"
        );
    }
    if let Some((base, text)) = split_pseudo(selector, ":text-is(") {
        let base_json = js_str(&base);
        let text_json = js_str(&text);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).find(\
             el => el.textContent.trim() === {text_json}) || null"
        );
    }
    // Standard CSS
    let sel_json = js_str(selector);
    format!("document.querySelector({sel_json})")
}

/// Convert a (possibly extended) selector to a JS expression that returns a NodeList/Array.
pub fn to_all(selector: &str) -> String {
    if let Some(expr) = selector.strip_prefix("js:") {
        return format!("(function(){{var r=({expr});return r instanceof Array?r:(r?[r]:[])}}())");
    }
    if let Some((base, text)) = split_pseudo(selector, ":has-text-i(") {
        let base_json = js_str(&base);
        let text_lower = text.to_lowercase();
        let text_json = js_str(&text_lower);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).filter(\
             el => el.textContent.toLowerCase().includes({text_json}))"
        );
    }
    if let Some((base, text)) = split_pseudo(selector, ":has-text(") {
        let base_json = js_str(&base);
        let text_json = js_str(&text);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).filter(\
             el => el.textContent.includes({text_json}))"
        );
    }
    if let Some((base, text)) = split_pseudo(selector, ":text-is(") {
        let base_json = js_str(&base);
        let text_json = js_str(&text);
        return format!(
            "Array.from(document.querySelectorAll({base_json})).filter(\
             el => el.textContent.trim() === {text_json})"
        );
    }
    // Standard CSS
    let sel_json = js_str(selector);
    format!("document.querySelectorAll({sel_json})")
}

/// Returns true if selector uses extensions (needs special JS, not plain querySelectorAll).
#[allow(dead_code)]
pub fn is_extended(selector: &str) -> bool {
    selector.starts_with("js:")
        || selector.contains(":has-text(")
        || selector.contains(":text-is(")
        || selector.contains(":has-text-i(")
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn split_pseudo(selector: &str, pseudo: &str) -> Option<(String, String)> {
    let pos = selector.find(pseudo)?;
    let base_raw = selector[..pos].trim();
    let base = if base_raw.is_empty() {
        "*".to_string()
    } else {
        base_raw.to_string()
    };
    let rest = &selector[pos + pseudo.len()..];
    let arg = extract_arg(rest);
    // strip surrounding quotes if present
    let text = arg
        .trim()
        .trim_start_matches('\'')
        .trim_end_matches('\'')
        .trim_start_matches('"')
        .trim_end_matches('"')
        .to_string();
    Some((base, text))
}

fn extract_arg(s: &str) -> &str {
    // find closing ')' (simple, handles one nesting level)
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return &s[..i];
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    s
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("{s:?}"))
}
