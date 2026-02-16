use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    script: Option<&str>,
    file: Option<&str>,
    timeout_ms: u64,
    output_path: Option<&str>,
    frame: Option<&str>,
) -> Result<(), WbError> {
    let script_content = if let Some(path) = file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| WbError::general(format!("Cannot read script file '{}': {}", path, e)))?;
        if content.trim().is_empty() {
            return Err(WbError::general(format!("Script file '{}' is empty", path)));
        }
        content
    } else if let Some(s) = script {
        s.to_string()
    } else {
        return Err(WbError::general("Provide a script argument or --file <path>"));
    };

    // Medium fix: always send timeout_ms explicitly
    let body = serde_json::json!({
        "session": session,
        "script": script_content,
        "timeout_ms": timeout_ms,
        "frame": frame,
    });

    let resp = client.post("/execute", &body)?;

    resp.check_success("Script execution failed")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    // Extract result — may be any JSON type
    let result = resp.body.get("result").cloned().unwrap_or(serde_json::Value::Null);
    let result_str = match &result {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| "null".to_string()),
    };

    if opts.no_file {
        print!("{result_str}");
        return Ok(());
    }

    let path = if let Some(p) = output_path {
        let path = output::validate_output_path(p)
            .map_err(|e| WbError::general(e))?;
        std::fs::write(&path, &result_str)
            .map_err(|e| WbError::general(format!("File write error: {e}")))?;
        path
    } else {
        if result_str.len() <= 500 {
            // Small results: print directly (even in quiet mode — no file to show path for)
            println!("{result_str}");
            output::print_result(opts, &format!("[{} ms]", resp.elapsed_ms));
            return Ok(());
        }
        // Large results: save to file
        output::save_text("executions", "exec", session, &result_str)
            .map_err(|e| WbError::general(format!("File save error: {e}")))?
    };

    let preview = output::truncate_str(&result_str, 80);
    output::print_result(opts, &format!("{preview}... [{} ms]", resp.elapsed_ms));
    output::print_saved(opts, &path);

    Ok(())
}
