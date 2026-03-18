use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod config;
mod runner;
mod sandbox;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    // Parse command line arguments
    let args = cli::Cli::parse();

    // Execute the corresponding subcommand
    match args.command {
        cli::Commands::Run { job } => {
            info!("Executing workflow run...");
            let cfg = config::load_config(".zenith.yml")?;
            runner::execute_local(cfg, job).await?;
        }
        cli::Commands::Lab(lab_cmd) => {
            sandbox::handle_lab(lab_cmd).await?;
        }
        cli::Commands::Matrix { action } => {
            info!("Matrix action: {:?}", action);
            println!("Matrix runner is not yet implemented (Phase 3).");
        }
        cli::Commands::Shell => {
            info!("Opening Zenith shell...");
            println!("Interactive shell is not yet implemented.");
        }
    }

    Ok(())
}
