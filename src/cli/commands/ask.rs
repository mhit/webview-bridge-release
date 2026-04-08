use crate::client::{WbClient, WbError};
use crate::output::{self, OutputOpts};

pub fn run(
    client: &WbClient,
    opts: &OutputOpts,
    session: &str,
    question: &str,
    context_chars: usize,
) -> Result<(), WbError> {
    let body = serde_json::json!({
        "session": session,
        "question": question,
        "context_chars": context_chars,
    });
    let resp = client.post("/ask", &body)?;
    resp.check_success("AI ask failed")?;

    if opts.json {
        output::print_json(&resp.body);
        return Ok(());
    }

    let answer = resp
        .body
        .get("answer")
        .and_then(|v| v.as_str())
        .unwrap_or("(no answer)");
    let model = resp
        .body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let context = resp
        .body
        .get("context_chars")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let page_title = resp
        .body
        .get("page")
        .and_then(|p| p.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !opts.quiet {
        if !page_title.is_empty() {
            eprintln!("  Page: {page_title}");
        }
        eprintln!(
            "  Model: {model} | {context} chars context [{} ms]",
            resp.elapsed_ms
        );
        eprintln!();
    }

    println!("{answer}");

    if !opts.quiet {
        eprintln!();
        eprintln!("  Next: wb ask \"<follow-up question>\" -s {session}");
        eprintln!("        wb extract \".selector\" -s {session}   (CSS-based, no AI)");
    }

    Ok(())
}
