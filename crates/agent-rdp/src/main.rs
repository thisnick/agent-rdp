//! agent-rdp: CLI tool for AI agents to control Windows Remote Desktop sessions.

mod cli;
mod ipc_client;
mod output;
mod session_manager;

use clap::Parser;
use tracing::error;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        error!("{}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    use output::Output;

    let output = Output::new(cli.json);

    match cli.command {
        Commands::Connect(args) => {
            cli::commands::connect::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Disconnect => {
            cli::commands::disconnect::run(&cli.session, &output, cli.timeout).await
        }
        Commands::Screenshot(args) => {
            cli::commands::screenshot::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Mouse(args) => {
            cli::commands::mouse::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Keyboard(args) => {
            cli::commands::keyboard::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Scroll(args) => {
            cli::commands::scroll::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Clipboard(args) => {
            cli::commands::clipboard::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Drive(args) => {
            cli::commands::drive::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Session(args) => {
            cli::commands::session::run(&cli.session, args, &output, cli.timeout).await
        }
        Commands::Wait { ms } => {
            cli::commands::wait::run(ms).await
        }
    }
}
