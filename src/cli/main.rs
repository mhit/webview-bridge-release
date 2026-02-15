use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod client;
mod commands;
mod output;
mod refs;
mod updater;

#[derive(Parser)]
#[command(name = "wb", version, about = "WebView Bridge CLI - Token-efficient browser automation")]
struct Cli {
    /// Server address
    #[arg(long, default_value = "http://127.0.0.1:9400", global = true, env = "WB_HOST")]
    host: String,

    /// Bearer token for authentication
    #[arg(long, global = true, env = "WB_TOKEN")]
    token: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Quiet mode: output file paths only
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Output to stdout instead of saving to file
    #[arg(long, global = true)]
    no_file: bool,

    /// Disable automatic update check at startup
    #[arg(long, global = true)]
    no_update_check: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show server status and active sessions
    Status,

    /// Manage browser sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Navigate to a URL
    Open {
        /// URL to navigate to
        url: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,
    },

    /// Capture interactive elements snapshot (token-efficient page view)
    Snapshot {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Include all elements, not just interactive ones
        #[arg(long)]
        all: bool,

        /// Limit scope to elements within a CSS selector
        #[arg(long)]
        within: Option<String>,

        /// Maximum number of elements
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Click an element (accepts e1, e2... refs or CSS selector)
    Click {
        /// Target element: e1, e2... (from snapshot) or CSS selector
        target: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,
    },

    /// Type text into an element
    Type {
        /// Target element: e1, e2... (from snapshot) or CSS selector
        target: String,

        /// Text to type
        text: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Clear field before typing
        #[arg(long)]
        clear: bool,
    },

    /// Take a screenshot
    Screenshot {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Device preset name (e.g., "iPhone 15", "Pixel 8")
        #[arg(long)]
        device: Option<String>,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Execute JavaScript in the browser
    Execute {
        /// JavaScript code to execute
        script: Option<String>,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Read script from file
        #[arg(short, long)]
        file: Option<String>,

        /// Execution timeout in milliseconds
        #[arg(long, default_value = "30000")]
        timeout: u64,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Scroll the page or an element
    Scroll {
        /// Direction: up or down
        direction: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Scroll amount in pixels
        #[arg(long, default_value = "500")]
        amount: i64,

        /// CSS selector to scroll (default: window)
        #[arg(long)]
        target: Option<String>,
    },

    /// Wait for an element or condition
    Wait {
        /// Target: e1, e2... (from snapshot) or CSS selector
        selector: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Condition: present, visible, stable, text_contains, clickable, detached
        #[arg(short, long, default_value = "present")]
        condition: String,

        /// Timeout in milliseconds
        #[arg(long, default_value = "10000")]
        timeout: u64,

        /// Text to match (for text_contains/text_matches conditions)
        #[arg(long)]
        text: Option<String>,
    },

    /// Select an option in a <select> dropdown
    Select {
        /// Target element: e1, e2... (from snapshot) or CSS selector
        target: String,

        /// Value or visible text to select
        value: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,
    },

    /// Extract data from page elements
    Extract {
        /// CSS selector for container elements
        selector: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Field mappings: "name=.sub-selector,price=.price"
        #[arg(short, long)]
        fields: Option<String>,

        /// Maximum number of items
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Manage cookies for a session
    Cookies {
        #[command(subcommand)]
        action: CookieAction,
    },

    /// Manage authentication token
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Check for and install CLI updates
    Update {
        /// Check only, don't download
        #[arg(long)]
        check: bool,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Acquire (create/reuse) a browser session
    Acquire {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
    },

    /// Release a browser session (keeps data for reuse)
    Release {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
    },

    /// List all sessions
    List,
}

#[derive(Subcommand)]
enum CookieAction {
    /// Get cookies from a session
    Get {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Set cookies from a JSON file
    Set {
        /// Path to JSON file with cookies array
        file: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,
    },

    /// Import cookies from a browser (Chrome/Edge/Firefox)
    Import {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Browser to import from: chrome, edge, firefox
        #[arg(short, long, default_value = "chrome")]
        browser: String,

        /// Browser profile name
        #[arg(short, long, default_value = "Default")]
        profile: String,

        /// Filter by domains (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        domains: Vec<String>,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Save a token for CLI authentication
    Save {
        /// The bearer token to save
        token: String,
    },

    /// Show the saved token (masked)
    Show,

    /// Remove the saved token
    Clear,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // H7: Background update check via channel (non-blocking, 1s timeout on exit)
    let skip_check = cli.no_update_check
        || std::env::var("WB_NO_UPDATE_CHECK").is_ok_and(|v| !v.is_empty() && v != "0" && v != "false");
    let bg_rx = if !skip_check && !matches!(cli.command, Command::Update { .. }) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(updater::background_check());
        });
        Some(rx)
    } else {
        None
    };

    let client = client::WbClient::new(&cli.host, cli.token.as_deref());
    let opts = output::OutputOpts {
        json: cli.json,
        quiet: cli.quiet,
        no_file: cli.no_file,
    };

    let result = match cli.command {
        Command::Status => commands::status::run(&client, &opts),
        Command::Session { action } => match action {
            SessionAction::Acquire { name } => {
                commands::session::acquire(&client, &opts, &name)
            }
            SessionAction::Release { name } => {
                commands::session::release(&client, &opts, &name)
            }
            SessionAction::List => commands::session::list(&client, &opts),
        },
        Command::Open { url, session } => {
            commands::open::run(&client, &opts, &session, &url)
        }
        Command::Snapshot { session, all, within, limit, output } => {
            commands::snapshot::run(&client, &opts, &session, all, within.as_deref(), limit, output.as_deref())
        }
        Command::Click { target, session } => {
            commands::click::run(&client, &opts, &session, &target)
        }
        Command::Type { target, text, session, clear } => {
            commands::type_cmd::run(&client, &opts, &session, &target, &text, clear)
        }
        Command::Screenshot { session, device, output } => {
            commands::screenshot::run(&client, &opts, &session, device.as_deref(), output.as_deref())
        }
        Command::Execute { script, session, file, timeout, output } => {
            commands::execute::run(&client, &opts, &session, script.as_deref(), file.as_deref(), timeout, output.as_deref())
        }
        Command::Scroll { direction, session, amount, target } => {
            commands::scroll::run(&client, &opts, &session, &direction, amount, target.as_deref())
        }
        Command::Wait { selector, session, condition, timeout, text } => {
            commands::wait::run(&client, &opts, &session, &selector, &condition, timeout, text.as_deref())
        }
        Command::Select { target, value, session } => {
            commands::select::run(&client, &opts, &session, &target, &value)
        }
        Command::Extract { selector, session, fields, limit, output } => {
            commands::extract::run(&client, &opts, &session, &selector, fields.as_deref(), limit, output.as_deref())
        }
        Command::Cookies { action } => match action {
            CookieAction::Get { session, output } => {
                commands::cookies::get(&client, &opts, &session, output.as_deref())
            }
            CookieAction::Set { file, session } => {
                commands::cookies::set(&client, &opts, &session, &file)
            }
            CookieAction::Import { session, browser, profile, domains } => {
                commands::cookies::import(&client, &opts, &session, &browser, &profile, &domains)
            }
        },
        Command::Auth { action } => match action {
            AuthAction::Save { token } => commands::auth::save(&opts, &token),
            AuthAction::Show => commands::auth::show(&opts),
            AuthAction::Clear => commands::auth::clear(&opts),
        },
        Command::Update { check } => commands::update::run(check),
    };

    // H7: Collect background update check (non-blocking, 1s timeout)
    if let Some(rx) = bg_rx {
        if let Ok(Some(msg)) = rx.recv_timeout(std::time::Duration::from_secs(1)) {
            eprintln!("{msg}");
        }
    }

    match result {
        Ok(()) => {
            // H6: Clean up leftover .old binary only after successful command
            updater::cleanup_old_binary();
            ExitCode::SUCCESS
        }
        Err(e) => {
            if !cli.json {
                eprintln!("Error: {e}");
            } else {
                let err = serde_json::json!({ "error": e.to_string(), "code": e.exit_code() });
                eprintln!("{}", serde_json::to_string(&err).unwrap_or_default());
            }
            ExitCode::from(e.exit_code())
        }
    }
}
