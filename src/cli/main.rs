use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod client;
mod commands;
mod output;
mod refs;
mod selector;
mod updater;

#[derive(Parser)]
#[command(
    name = "wb",
    version,
    about = "WebView Bridge CLI - Token-efficient browser automation",
    after_help = "\
QUICK START:
  wb auth save <TOKEN>       Save server token (shown at server startup)
  wb session acquire mysite  Create/activate a browser session (sessions are PERSISTENT)
  wb login run mysite        Auto-login via configured credentials (1Password or plaintext)
  wb open https://example.com -s mysite
  wb snapshot -s mysite      View interactive elements (e1, e2... refs)
  wb click e3 -s mysite      Click element #3 from snapshot

SESSION PERSISTENCE (important for AI agents):
  Sessions survive server restarts — cookies and login state are preserved.
  Always run 'wb session acquire <name>' first; it tells you if the session is new or resumed.
  If resumed and previously logged in, skip login — just navigate or snapshot directly.
  Prefer long-lived named sessions over 'default' for multi-step automation.

LOGIN WORKFLOW:
  1. wb login config-set mysite config.json   Configure auto-login (1Password or plaintext)
  2. wb session acquire mysite                Acquire session (check hints in output)
  3. wb login status mysite                   Check login status
  4. wb login run mysite                      Perform auto-login (idempotent — safe to re-run)

EFFICIENT USAGE (for AI agents):
  wb snapshot -s S --all --within \".main\" --limit 30   Focused full-page capture
  wb snapshot -s S --limit 20                           Minimal interactive-only view
  wb extract \".items\" -F \"name=h3,price=.cost\" -s S    Structured data extraction
  wb execute \"document.title\" -s S                      Quick JS data retrieval
  wb wait \"#result\" -s S && wb snapshot -s S            Wait then capture
  Use --json for machine-readable output. Use --no-file to skip file saving.
  For autonomous multi-step tasks, use the MCP 'agent' tool or POST /goal API.

IFRAME SUPPORT:
  wb frames -s S                       List all iframes
  wb snapshot -s S --frame \"embed.co\"  Capture inside an iframe
  wb click e2 -s S --frame \"content\"   Interact inside an iframe

ENVIRONMENT VARIABLES:
  WB_HOST              Server address (default: http://127.0.0.1:9400)
  WB_TOKEN             Bearer token for authentication
  WB_NO_UPDATE_CHECK   Set to 1 to disable update checks
  WEBVIEW_BRIDGE_DATA_PATH  Override data directory (default: %APPDATA%/webview-bridge)"
)]
struct Cli {
    /// Server address [env: WB_HOST]
    #[arg(
        long,
        default_value = "http://127.0.0.1:9400",
        global = true,
        env = "WB_HOST"
    )]
    host: String,

    /// Bearer token for authentication [env: WB_TOKEN]
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

    /// Disable automatic update check at startup [env: WB_NO_UPDATE_CHECK]
    #[arg(long, global = true, env = "WB_NO_UPDATE_CHECK")]
    no_update_check: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show server status and active sessions
    Status,

    /// List all frames (iframes) in the page
    Frames {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,
    },

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

        /// Extra wait after page load (ms). Use for JS-heavy pages like charts/dashboards [default: 0]
        #[arg(long, default_value_t = 0)]
        wait: u64,
    },

    /// Capture interactive elements snapshot (token-efficient page view)
    Snapshot {
        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Snapshot format: "dom" (default, DOM injection) or "ax" (CDP Accessibility tree)
        #[arg(long, default_value = "dom", value_parser = ["dom", "ax"])]
        format: String,

        /// Include all elements, not just interactive ones (dom mode only)
        #[arg(long)]
        all: bool,

        /// Limit scope to elements within a CSS selector (dom mode only)
        #[arg(long)]
        within: Option<String>,

        /// Maximum number of elements (dom mode only)
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Target iframe (URL substring, frame name, or frame ID) (dom mode only)
        #[arg(short, long)]
        frame: Option<String>,
    },

    /// Click an element (accepts e1, e2... refs or CSS selector)
    Click {
        /// Target element: e1, e2... (from snapshot) or CSS selector
        target: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,
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

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,
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

        /// Target iframe (index, name, or URL substring)
        #[arg(long)]
        frame: Option<String>,
    },

    /// Execute JavaScript in the browser
    Execute {
        /// JavaScript code to execute
        script: Option<String>,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Read script from file
        #[arg(short = 'F', long)]
        file: Option<String>,

        /// Execution timeout in milliseconds
        #[arg(long, default_value = "30000")]
        timeout: u64,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(short, long)]
        frame: Option<String>,
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

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,
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

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,
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

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,
    },

    /// Extract data from page elements
    Extract {
        /// CSS selector for container elements
        selector: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Field mappings: "name=.sub-selector,price=.price"
        #[arg(short = 'F', long)]
        fields: Option<String>,

        /// Maximum number of items
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Target iframe (URL substring, frame name, or frame ID)
        #[arg(long)]
        frame: Option<String>,

        /// Enable scroll-and-collect mode (for virtual-scroll sites like X.com)
        #[arg(long)]
        scroll: bool,

        /// Max scroll iterations
        #[arg(long, default_value = "10")]
        scroll_max: usize,

        /// Field name for deduplication (omit for full-item hash)
        #[arg(long)]
        scroll_dedup: Option<String>,

        /// Delay between scrolls in ms
        #[arg(long, default_value = "500")]
        scroll_delay: u64,

        /// Pixels per scroll
        #[arg(long, default_value = "800")]
        scroll_amount: u32,
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

    /// Upload a file to session storage
    Upload {
        /// File path to upload
        file: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Override filename
        #[arg(long)]
        filename: Option<String>,
    },

    /// Inject uploaded file into a form file input via CDP
    InjectFile {
        /// File URL (from upload response) or absolute path
        file_url: String,

        /// CSS selector for file input
        #[arg(short = 'S', long, default_value = "input[type=file]")]
        selector: String,

        /// Session name
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Target iframe
        #[arg(long)]
        frame: Option<String>,
    },

    /// Check for and install CLI updates
    Update {
        /// Check only, don't download
        #[arg(long)]
        check: bool,
    },

    /// Auto-login a session using 1Password credentials
    Login {
        #[command(subcommand)]
        action: LoginAction,
    },
}

#[derive(Subcommand)]
enum LoginAction {
    /// Perform auto-login for a session (requires auto_login.toml or config.toml config)
    Run {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
        /// Force re-fetch credentials from 1Password, ignoring cache
        #[arg(long)]
        force: bool,
        /// Override 1Password item name/ID for this request
        #[arg(long)]
        op_item: Option<String>,
    },
    /// Show auto-login status and history for a session
    Status {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
    },
    /// List auto-login status for all sessions
    List,
    /// Get auto-login config for a session (as JSON)
    ConfigGet {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
    },
    /// Set auto-login config for a session from a JSON file or stdin (-)
    ConfigSet {
        /// Session name
        #[arg(default_value = "default")]
        name: String,
        /// JSON file path or - for stdin
        #[arg(default_value = "-")]
        file: String,
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
    // Set Windows console output to UTF-8 (code page 65001) so Japanese and other
    // multi-byte characters are not garbled when wb.exe is called from PowerShell or cmd.exe.
    // SetConsoleOutputCP fixes console display; _setmode fixes piped/redirected stdout/stderr
    // by disabling the CRT's CRLF/encoding translation layer (sets raw binary passthrough).
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
        }
        unsafe extern "C" {
            fn _setmode(fd: i32, mode: i32) -> i32;
        }
        unsafe {
            SetConsoleOutputCP(65001);
            _setmode(1, 0x8000); // stdout → _O_BINARY
            _setmode(2, 0x8000); // stderr → _O_BINARY
        }
    }

    let cli = Cli::parse();

    // H7: Background update check via channel (non-blocking, 1s timeout on exit)
    // cli.no_update_check is set by --no-update-check flag OR WB_NO_UPDATE_CHECK env var (via clap)
    let skip_check = cli.no_update_check;
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
        Command::Frames { session } => commands::frames::run(&client, &opts, &session),
        Command::Session { action } => match action {
            SessionAction::Acquire { name } => commands::session::acquire(&client, &opts, &name),
            SessionAction::Release { name } => commands::session::release(&client, &opts, &name),
            SessionAction::List => commands::session::list(&client, &opts),
        },
        Command::Open { url, session, wait } => {
            commands::open::run(&client, &opts, &session, &url, wait)
        }
        Command::Snapshot {
            session,
            format,
            all,
            within,
            limit,
            output,
            frame,
        } => commands::snapshot::run(
            &client,
            &opts,
            &session,
            &format,
            all,
            within.as_deref(),
            limit,
            output.as_deref(),
            frame.as_deref(),
        ),
        Command::Click {
            target,
            session,
            frame,
        } => commands::click::run(&client, &opts, &session, &target, frame.as_deref()),
        Command::Type {
            target,
            text,
            session,
            clear,
            frame,
        } => commands::type_cmd::run(
            &client,
            &opts,
            &session,
            &target,
            &text,
            clear,
            frame.as_deref(),
        ),
        Command::Screenshot {
            session,
            device,
            output,
            frame,
        } => commands::screenshot::run(
            &client,
            &opts,
            &session,
            device.as_deref(),
            output.as_deref(),
            frame.as_deref(),
        ),
        Command::Execute {
            script,
            session,
            file,
            timeout,
            output,
            frame,
        } => commands::execute::run(
            &client,
            &opts,
            &session,
            script.as_deref(),
            file.as_deref(),
            timeout,
            output.as_deref(),
            frame.as_deref(),
        ),
        Command::Scroll {
            direction,
            session,
            amount,
            target,
            frame,
        } => commands::scroll::run(
            &client,
            &opts,
            &session,
            &direction,
            amount,
            target.as_deref(),
            frame.as_deref(),
        ),
        Command::Wait {
            selector,
            session,
            condition,
            timeout,
            text,
            frame,
        } => commands::wait::run(
            &client,
            &opts,
            &session,
            &selector,
            &condition,
            timeout,
            text.as_deref(),
            frame.as_deref(),
        ),
        Command::Select {
            target,
            value,
            session,
            frame,
        } => commands::select::run(&client, &opts, &session, &target, &value, frame.as_deref()),
        Command::Extract {
            selector,
            session,
            fields,
            limit,
            output,
            frame,
            scroll,
            scroll_max,
            scroll_dedup,
            scroll_delay,
            scroll_amount,
        } => commands::extract::run(
            &client,
            &opts,
            &session,
            &selector,
            fields.as_deref(),
            limit,
            output.as_deref(),
            frame.as_deref(),
            scroll,
            scroll_max,
            scroll_dedup.as_deref(),
            scroll_delay,
            scroll_amount,
        ),
        Command::Cookies { action } => match action {
            CookieAction::Get { session, output } => {
                commands::cookies::get(&client, &opts, &session, output.as_deref())
            }
            CookieAction::Set { file, session } => {
                commands::cookies::set(&client, &opts, &session, &file)
            }
            CookieAction::Import {
                session,
                browser,
                profile,
                domains,
            } => commands::cookies::import(&client, &opts, &session, &browser, &profile, &domains),
        },
        Command::Auth { action } => match action {
            AuthAction::Save { token } => commands::auth::save(&opts, &token),
            AuthAction::Show => commands::auth::show(&opts),
            AuthAction::Clear => commands::auth::clear(&opts),
        },
        Command::Upload {
            file,
            session,
            filename,
        } => commands::upload::run(&client, &opts, &session, &file, filename.as_deref()),
        Command::InjectFile {
            file_url,
            selector,
            session,
            frame,
        } => commands::inject_file::run(
            &client,
            &opts,
            &session,
            &file_url,
            &selector,
            frame.as_deref(),
        ),
        Command::Update { check } => commands::update::run(check),
        Command::Login { action } => match action {
            LoginAction::Run {
                name,
                force,
                op_item,
            } => commands::login::run(&client, &opts, &name, force, op_item.as_deref()),
            LoginAction::Status { name } => commands::login::status(&client, &opts, &name),
            LoginAction::List => commands::login::list(&client, &opts),
            LoginAction::ConfigGet { name } => commands::login::config_get(&client, &opts, &name),
            LoginAction::ConfigSet { name, file } => {
                commands::login::config_set(&client, &opts, &name, &file)
            }
        },
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
