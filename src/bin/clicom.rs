use clap::{Parser, Subcommand};
use clicom::clicom_cli::cmd_run::BusyMode;

#[derive(Parser)]
#[command(name = "clicom", disable_help_subcommand = true, arg_required_else_help = false)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    Start {
        #[arg(long)] mouse: bool,
        #[arg(long)] nopty: bool,
        #[arg(long)] name: Option<String>,
        #[arg(last = true)] command: Vec<String>,
    },
    Status { partial: Option<String> },
    Run {
        partial: Option<String>,
        #[arg(short = 'f')] file: Option<String>,
        #[arg(long)] wait: bool,
        #[arg(long)] force: bool,
        #[arg(long)] timeout: Option<u64>,
        source: Option<String>,
    },
    Queue {
        partial: Option<String>,
        #[arg(short = 'f')] file: Option<String>,
        /// Inline script. Use "-" to read from stdin.
        source: Option<String>,
    },
    Clean {
        partial: Option<String>,
        id: Option<String>,
    },
    Help { topic: Option<String> },
    /// Type text. Default appends Enter (\r). --no-enter to suppress; --raw to disable \n→\r translation.
    Type {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] raw: bool,
        #[arg(long)] no_enter: bool,
        text: String,
    },
    /// Send a keyboard chord spec like "[Ctrl+C]" or "[Up][Up][Enter]" or "hi[Tab]bye[Enter]".
    Keys {
        #[arg(long)] partial: Option<String>,
        spec: String,
    },
    /// Print the wrapped agent's current visible screen.
    Screen {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
    },
    /// Print everything after the last occurrence of <marker>.
    ScreenAfter {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
        marker: String,
    },
    /// Print everything after the last regex match of <pattern>.
    ScreenAfterRe {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
        pattern: String,
    },
    /// Wait until the agent has been idle for <ms> ms.
    WaitIdle {
        #[arg(long)] partial: Option<String>,
        #[arg(default_value_t = 800)] ms: u64,
        #[arg(long)] timeout: Option<u64>,
    },
    /// Start a stdio MCP server exposing clicom's driver ops as tools.
    Mcp,
    /// Spawn a command as a detached process. Prints the new pid.
    /// Useful for launching wrappers from scripts/MCP without occupying the caller's terminal.
    ExecDetached {
        /// The command to spawn, after `--`.
        #[arg(last = true, required = true, num_args = 1..)]
        cmd: Vec<String>,
    },
    /// Identify which clicom-wrapped instance this process is running inside.
    /// Walks the parent-PID chain; prints dir_name + path + pid + name + state.
    Whoami {
        /// Emit JSON instead of human-readable text.
        #[arg(long)] json: bool,
    },
}

fn read_script_source(arg: Option<&str>, file: Option<&str>) -> anyhow::Result<String> {
    use std::io::Read;
    if let Some(p) = file { return Ok(std::fs::read_to_string(p)?); }
    match arg {
        Some("-") => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; Ok(s) }
        Some(s) => Ok(s.to_string()),
        None => anyhow::bail!("no script source given (positional, -f <file>, or -)"),
    }
}

fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    let cmd = match cli.cmd {
        Some(c) => c,
        None => std::process::exit(clicom::clicom_cli::cmd_help::quickstart()),
    };
    let code = match cmd {
        Cmd::Start { mouse, nopty, name, command } => {
            clicom::clicom_cli::cmd_start::run(&cwd, clicom::clicom_cli::cmd_start::StartArgs { mouse, nopty, name, command })?
        }
        Cmd::Status { partial } => clicom::clicom_cli::cmd_status::run(&cwd, partial.as_deref())?,
        Cmd::Run { partial, file, wait, force, timeout, source } => {
            let body = read_script_source(source.as_deref(), file.as_deref())?;
            let mode = if wait { BusyMode::Wait }
                       else if force { BusyMode::Force }
                       else { BusyMode::Default };
            clicom::clicom_cli::cmd_run::run(&cwd, clicom::clicom_cli::cmd_run::RunArgs {
                partial, source: body, mode, timeout_ms: timeout.unwrap_or(600_000),
            })?
        }
        Cmd::Queue { partial, file, source } => {
            let body = read_script_source(source.as_deref(), file.as_deref())?;
            clicom::clicom_cli::cmd_queue::run(&cwd, partial.as_deref(), body)?
        }
        Cmd::Clean { partial, id } =>
            clicom::clicom_cli::cmd_clean::run(&cwd, partial.as_deref(), id.as_deref())?,
        Cmd::Help { topic } => clicom::clicom_cli::cmd_help::run(topic.as_deref()),
        Cmd::Type { partial, raw, no_enter, text } => {
            let body = if no_enter || text.ends_with('\n') { text.clone() } else { format!("{}\n", text) };
            clicom::clicom_cli::quickops::type_text(&cwd, partial.as_deref(), &body, !raw)?
        }
        Cmd::Keys { partial, spec } =>
            clicom::clicom_cli::quickops::type_keys(&cwd, partial.as_deref(), &spec)?,
        Cmd::Screen { partial, no_status } =>
            clicom::clicom_cli::quickops::screen(&cwd, partial.as_deref(), no_status)?,
        Cmd::ScreenAfter { partial, no_status, marker } =>
            clicom::clicom_cli::quickops::screen_after(&cwd, partial.as_deref(), &marker, no_status)?,
        Cmd::ScreenAfterRe { partial, no_status, pattern } =>
            clicom::clicom_cli::quickops::screen_after_re(&cwd, partial.as_deref(), &pattern, no_status)?,
        Cmd::WaitIdle { partial, ms, timeout } =>
            clicom::clicom_cli::quickops::wait_idle(&cwd, partial.as_deref(), ms, timeout)?,
        Cmd::Mcp => clicom::clicom_cli::cmd_mcp::run(&cwd)?,
        Cmd::ExecDetached { cmd } => clicom::clicom_cli::cmd_exec_detached::run(cmd)?,
        Cmd::Whoami { json } => clicom::clicom_cli::cmd_whoami::run(&cwd, json)?,
    };
    std::process::exit(code);
}
