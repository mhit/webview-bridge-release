use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    file_url: &str,
    selector: &str,
    frame: Option<&str>,
) -> Result<(), WbError> {
    let body = serde_json::json!({
        "session": session,
        "selector": selector,
        "file": file_url,
        "frame": frame,
    });

    let resp = client.post("/form/inject-file", &body)?;
    resp.check_success("File injection failed")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    output::print_result(opts, &format!(
        "File injected into '{}' [{} ms]",
        selector, resp.elapsed_ms
    ));
    Ok(())
}
