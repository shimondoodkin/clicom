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
    };
    std::process::exit(code);
}
