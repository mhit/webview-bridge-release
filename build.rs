fn main() {
    // Embed target triple for CLI update command
    if std::env::var("CARGO_FEATURE_CLI").is_ok()
        && let Ok(target) = std::env::var("TARGET")
    {
        println!("cargo:rustc-env=TARGET={target}");
    }

    // Only embed Windows resources for the server binary
    if std::env::var("CARGO_FEATURE_SERVER").is_ok()
        && std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows"
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("docs/img/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
