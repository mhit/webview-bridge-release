use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn get(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    output_path: Option<&str>,
) -> Result<(), WbError> {
    let encoded_session = urlencoding::encode(session);
    let resp = client.get(&format!("/session/{encoded_session}/cookies"))?;

    resp.check_success("Failed to get cookies")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let cookies = resp
        .body
        .get("cookies")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let count = cookies.as_array().map(|a| a.len()).unwrap_or(0);
    let data_str = serde_json::to_string_pretty(&cookies).unwrap_or_else(|_| "[]".to_string());

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
        output::save_text("cookies", "cookies", session, &data_str)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    output::print_result(opts, &format!("{count} cookies [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);
    Ok(())
}

pub fn set(client: &WbClient, opts: &OutputOpts, session: &str, file: &str) -> Result<(), WbError> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| WbError::general(format!("Cannot read cookie file '{}': {}", file, e)))?;
    let cookies: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| WbError::general(format!("Invalid JSON in cookie file: {e}")))?;

    // Medium fix: validate cookie data is an array
    if !cookies.is_array() {
        return Err(WbError::general("Cookie file must contain a JSON array"));
    }

    let body = serde_json::json!({ "cookies": cookies });
    let encoded_session = urlencoding::encode(session);
    let resp = client.post(&format!("/session/{encoded_session}/cookies"), &body)?;

    resp.check_success("Failed to set cookies")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let count = resp
            .body
            .get("set_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        output::print_result(
            opts,
            &format!(
                "{count} cookies set on '{session}' [{} ms]",
                resp.elapsed_ms
            ),
        );
    }
    Ok(())
}

pub fn import(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    browser: &str,
    profile: &str,
    domains: &[String],
) -> Result<(), WbError> {
    let mut body = serde_json::json!({
        "session": session,
        "browser": browser,
        "profile": profile,
    });
    if !domains.is_empty() {
        body["domains"] = serde_json::json!(domains);
    }

    let resp = client.post("/session/import", &body)?;

    resp.check_success("Cookie import failed")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        // Medium fix: server returns "imported_count" at root level (verified in api_v2.rs:1343)
        let count = resp
            .body
            .get("imported_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        output::print_result(
            opts,
            &format!(
                "{count} cookies imported from {browser} [{} ms]",
                resp.elapsed_ms
            ),
        );
    }
    Ok(())
}
