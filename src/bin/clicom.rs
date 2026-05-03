use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clicom", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
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
    Help { topic: Option<String> },
}

fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    let code = match cli.cmd {
        Cmd::Start { mouse, nopty, name, command } => {
            clicom::clicom_cli::cmd_start::run(&cwd, clicom::clicom_cli::cmd_start::StartArgs { mouse, nopty, name, command })?
        }
        Cmd::Status { partial } => clicom::clicom_cli::cmd_status::run(&cwd, partial.as_deref())?,
        Cmd::Help { topic } => clicom::clicom_cli::cmd_help::run(topic.as_deref()),
    };
    std::process::exit(code);
}
