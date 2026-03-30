use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    url: &str,
    post_load_wait_ms: u64,
) -> Result<(), WbError> {
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
        let elem_count = resp.body.get("element_count").and_then(|v| v.as_u64());
        let count_str = elem_count
            .map(|n| format!(", {n} elements"))
            .unwrap_or_default();
        output::print_result(
            opts,
            &format!("Navigated: {url_short} [{display_ms} ms{count_str}]"),
        );
        // Print snapshot elements if present (skipped in quiet mode)
        if !opts.quiet {
            if let Some(snap) = resp.body.get("snapshot") {
                if let Some(elements) = snap.get("elements").and_then(|e| e.as_array()) {
                    for el in elements {
                        let r = el.get("ref").and_then(|v| v.as_str()).unwrap_or("?");
                        let tag = el.get("tag").and_then(|v| v.as_str()).unwrap_or("");
                        let text = el.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let href = el.get("href").and_then(|v| v.as_str());
                        let t = el.get("type").and_then(|v| v.as_str());
                        let val = el.get("value").and_then(|v| v.as_str());
                        let name_attr = el.get("name").and_then(|v| v.as_str());
                        let detail = match (tag, t, href) {
                            ("a", _, Some(h)) if !text.is_empty() => format!(
                                "[{r}] a \"{}\" href={}",
                                output::truncate_str(text, 50),
                                output::truncate_str(h, 60)
                            ),
                            ("a", _, Some(h)) => {
                                format!("[{r}] a href={}", output::truncate_str(h, 60))
                            }
                            ("input", Some(t), _) => {
                                let n = name_attr.unwrap_or("");
                                let v = val.unwrap_or("");
                                if v.is_empty() {
                                    format!("[{r}] input[{t}] \"{n}\"")
                                } else {
                                    format!(
                                        "[{r}] input[{t}] \"{n}\" val=\"{}\"",
                                        output::truncate_str(v, 30)
                                    )
                                }
                            }
                            ("button", _, _) | ("select", _, _) | ("textarea", _, _) => {
                                if text.is_empty() {
                                    format!("[{r}] {tag}")
                                } else {
                                    format!("[{r}] {tag} \"{}\"", output::truncate_str(text, 50))
                                }
                            }
                            _ => {
                                if text.is_empty() {
                                    format!("[{r}] {tag}")
                                } else {
                                    format!("[{r}] {tag} \"{}\"", output::truncate_str(text, 50))
                                }
                            }
                        };
                        println!("{detail}");
                    }
                    println!("\n--- {} interactive elements ---", elements.len());
                }
            }
        }
    }
    Ok(())
}
