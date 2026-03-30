use crate::client::{WbClient, WbError};
use crate::output::OutputOpts;

pub fn run(client: &WbClient, opts: &OutputOpts, session: &str) -> Result<(), WbError> {
    let resp = client.get(&format!("/frames?session={}", session))?;
    resp.check_success("Failed to list frames")?;

    if opts.json {
        crate::output::print_json(&resp.body);
        return Ok(());
    }

    let frames = resp.body.get("frames").and_then(|v| v.as_array());

    if let Some(frames) = frames {
        if frames.is_empty() {
            println!("No frames found (single-page, no iframes)");
            return Ok(());
        }
        for frame in frames {
            let id = frame.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let url = frame.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let name = frame.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let parent = frame.get("parent_id").and_then(|v| v.as_str());

            let indent = if parent.is_some() { "  " } else { "" };
            let name_part = if !name.is_empty() {
                format!(" name=\"{}\"", name)
            } else {
                String::new()
            };

            println!("{}{}{}", indent, url, name_part);
            if !opts.quiet {
                println!("{}  id: {}", indent, id);
            }
        }
        println!("\n--- {} frame(s) ---", frames.len());
    } else {
        println!("No frame data returned");
    }

    Ok(())
}
