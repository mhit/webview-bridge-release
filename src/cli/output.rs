use std::fs;
use std::path::PathBuf;

pub struct OutputOpts {
    pub json: bool,
    pub quiet: bool,
    pub no_file: bool,
}

/// Get the wb output directory ($TMPDIR/wb/ or %TEMP%\wb\)
pub fn output_dir() -> PathBuf {
    let base = std::env::temp_dir().join("wb");
    fs::create_dir_all(&base).ok();
    base
}

/// Save text content to a file and return the path
pub fn save_text(subdir: &str, prefix: &str, session: &str, content: &str) -> std::io::Result<PathBuf> {
    let dir = output_dir().join(subdir);
    fs::create_dir_all(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let filename = format!("{prefix}-{session}-{ts}.txt");
    let path = dir.join(filename);
    fs::write(&path, content)?;
    Ok(path)
}

/// Save binary content to a file and return the path
pub fn save_binary(subdir: &str, prefix: &str, session: &str, ext: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    let dir = output_dir().join(subdir);
    fs::create_dir_all(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let filename = format!("{prefix}-{session}-{ts}.{ext}");
    let path = dir.join(filename);
    fs::write(&path, data)?;
    Ok(path)
}

/// Print a result line to stdout (respects quiet mode)
pub fn print_result(opts: &OutputOpts, message: &str) {
    if !opts.quiet {
        println!("{message}");
    }
}

/// Print a file save notification
pub fn print_saved(opts: &OutputOpts, path: &PathBuf) {
    if opts.quiet {
        println!("{}", path.display());
    } else {
        println!("Saved: {}", path.display());
    }
}

/// Print a JSON value to stdout
pub fn print_json(value: &serde_json::Value) {
    if let Ok(s) = serde_json::to_string_pretty(value) {
        println!("{s}");
    }
}

/// Validate and resolve a user-provided output path.
/// Rejects paths containing `..` components to prevent traversal attacks.
pub fn validate_output_path(p: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(p);
    // Reject path traversal components
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("Output path must not contain '..' components".to_string());
        }
    }
    Ok(path)
}

/// Truncate a string at a character boundary, safe for UTF-8/multibyte.
pub fn truncate_str(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        s
    } else {
        let end = s.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(s.len());
        &s[..end]
    }
}
