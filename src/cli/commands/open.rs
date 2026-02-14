use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(client: &WbClient, opts: &OutputOpts, session: &str, url: &str) -> Result<(), WbError> {
    let body = serde_json::json!({
        "session": session,
        "url": url,
    });
    let resp = client.post("/navigate", &body)?;

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
