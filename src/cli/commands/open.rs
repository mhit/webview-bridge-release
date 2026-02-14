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
        let title = resp.body.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if title.is_empty() {
            output::print_result(opts, &format!("Navigated [{} ms]", resp.elapsed_ms));
        } else {
            output::print_result(opts, &format!("Navigated: {title} [{} ms]", resp.elapsed_ms));
        }
    }
    Ok(())
}
