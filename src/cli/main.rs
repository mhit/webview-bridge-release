use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod client;
mod commands;
mod output;
mod refs;

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

fn main() -> ExitCode {
    let cli = Cli::parse();
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
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
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
