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
        /// Optional specific job name to run
        #[arg(short, long)]
        job: Option<String>,
    },
    /// Manage isolated sandbox lab environments
    #[command(subcommand)]
    Lab(LabCommands),
    /// Execute multi-OS concurrent matrix workflows
    Matrix {
        #[arg(value_enum, default_value_t = MatrixAction::Run)]
        action: MatrixAction,
    },
    /// Drop into an interactive Zenith shell
    Shell,
}

#[derive(Subcommand, Debug)]
pub enum LabCommands {
    /// List all active lab environments
    List,
    /// Create and start a new ephemeral lab environment
    Create {
        /// Target OS environment image (e.g., ubuntu, alpine, debian)
        #[arg(default_value = "ubuntu")]
        os: String,
    },
    /// Execute a command inside a running lab environment
    Run {
        /// Target OS environment
        os: String,
        /// Command to run inside the sandbox
        command: String,
    },
    /// Open an interactive shell inside a lab environment
    Shell {
        /// Target OS environment
        #[arg(default_value = "ubuntu")]
        os: String,
    },
    /// Push the current project files into the canvas (no host bind mount)
    Push {
        /// Target OS environment to push files into
        #[arg(default_value = "ubuntu")]
        os: String,
    },
    /// Destroy and remove a lab environment
    Destroy {
        /// Target OS environment to destroy
        os: String,
    },
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum MatrixAction {
    #[default]
    Run,
    List,
}
