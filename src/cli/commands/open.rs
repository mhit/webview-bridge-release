use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(client: &WbClient, opts: &OutputOpts, session: &str, url: &str, post_load_wait_ms: u64) -> Result<(), WbError> {
    let mut body = serde_json::json!({
        "session": session,
        "url": url,
    });
    if post_load_wait_ms > 0 {
        body["post_load_wait_ms"] = serde_json::Value::Number(post_load_wait_ms.into());
    }
    let resp = client.post("/navigate", &body)?;
    resp.check_success("Navigation failed")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let load_ms = resp.body.get("load_time_ms").and_then(|v| v.as_u64());
        let display_ms = load_ms.unwrap_or(resp.elapsed_ms);
        let url_short = output::truncate_str(url, 60);
        output::print_result(opts, &format!("Navigated: {url_short} [{display_ms} ms]"));
    }
    Ok(())
}
