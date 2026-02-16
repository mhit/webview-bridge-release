use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};
use crate::refs;

pub fn run(client: &WbClient, opts: &OutputOpts, session: &str, target: &str, text: &str, clear: bool, frame: Option<&str>) -> Result<(), WbError> {
    let selector = refs::resolve_target(target);
    let mut body = serde_json::json!({
        "session": session,
        "selector": selector,
        "text": text,
        "frame": frame,
    });
    if clear {
        body["clear_first"] = serde_json::Value::Bool(true);
    }
    let resp = client.post("/type", &body)?;
    resp.check_success("Type failed")?;

    if opts.json {
        output::print_json(&resp.body);
    } else {
        let chars = text.len();
        output::print_result(opts, &format!("Typed {chars} chars into '{target}' [{} ms]", resp.elapsed_ms));
    }
    Ok(())
}
