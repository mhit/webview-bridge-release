use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use crate::refs;

pub fn run(client: &WbClient, opts: &OutputOpts, session: &str, target: &str) -> Result<(), WbError> {
    let selector = refs::resolve_target(target);
    let body = serde_json::json!({
        "session": session,
        "selector": selector,
    });
    let resp = client.post("/click", &body)?;
    resp.check_success("Click failed")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        output::print_result(opts, &format!("Clicked '{target}' [{} ms]", resp.elapsed_ms));
    }
    Ok(())
}
