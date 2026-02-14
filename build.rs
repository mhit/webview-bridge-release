fn main() {
    // Only embed Windows resources for the server binary
    if std::env::var("CARGO_FEATURE_SERVER").is_ok()
        && std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows"
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("docs/img/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
