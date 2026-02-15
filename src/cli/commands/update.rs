use crate::client::WbError;
use crate::updater;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(check_only: bool) -> Result<(), WbError> {
    let release = updater::check_latest().map_err(|e| WbError::general(e.to_string()))?;

    let current = CURRENT_VERSION;
    let latest = &release.version;

    if !updater_is_newer(latest, current) {
        println!("Current: v{current}");
        println!("Already up to date.");
        return Ok(());
    }

    println!("Current: v{current} → Latest: v{latest}");

    if check_only {
        println!("Run `wb update` to install.");
        return Ok(());
    }

    println!("Downloading {}...", release.asset_name);

    updater::perform_update(&release).map_err(|e| WbError::general(e.to_string()))?;

    println!("Updated successfully! Restart to use v{latest}.");
    Ok(())
}

fn updater_is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    parse(latest) > parse(current)
}
