use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zenith")]
#[command(author = "Zenith")]
#[command(version = "0.1.0")]
#[command(about = "Local Multi-OS Workflow Runtime", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a local workflow defined in .zenith.yml
    Run {
        /// Optional specific job to run
        #[arg(short, long)]
        job: Option<String>,
    },
    /// Manage isolated sandbox lab environments
    Lab {
        action: LabAction,
    },
    /// Execute multi-OS concurrent matrix workflows
    Matrix {
        action: MatrixAction,
    },
    /// Drop into an interactive Zenith shell
    Shell,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum LabAction {
    #[default]
    List,
    Create,
    Destroy,
    Shell,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum MatrixAction {
    #[default]
    List,
    Run,
}
