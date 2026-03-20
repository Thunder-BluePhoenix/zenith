use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zenith")]
#[command(author = "Zenith")]
#[command(version = "0.1.0")]
#[command(about = "Local Multi-OS Workflow Runtime — You install Zenith. Zenith installs everything else.", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a local workflow defined in .zenith.yml
    Run {
        /// Run a specific named job instead of the default
        #[arg(short, long)]
        job: Option<String>,
        /// Force re-run all steps, ignoring the cache
        #[arg(long, default_value_t = false)]
        no_cache: bool,
        /// Run workflow on a registered remote machine
        #[arg(long)]
        remote: Option<String>,
    },

    /// Build — run only steps that produce outputs, with caching
    Build {
        /// Build a specific named job
        #[arg(short, long)]
        job: Option<String>,
        /// Force full rebuild, skip cache
        #[arg(long, default_value_t = false)]
        no_cache: bool,
    },

    /// Manage the build cache
    #[command(subcommand)]
    Cache(CacheCommands),

    /// Manage isolated sandbox lab environments
    #[command(subcommand)]
    Lab(LabCommands),

    /// Manage declarative language toolchain environments (Phase 7)
    #[command(subcommand)]
    Env(EnvCommands),

    /// Execute multi-OS concurrent matrix workflows
    Matrix {
        #[arg(value_enum, default_value_t = MatrixAction::Run)]
        action: MatrixAction,
        /// Force re-run all steps, ignoring the cache
        #[arg(long, default_value_t = false)]
        no_cache: bool,
    },

    /// Drop into an interactive shell with Zenith toolchains on PATH
    Shell {
        /// Open shell inside a specific lab environment
        #[arg(long)]
        lab: Option<String>,
    },
}

// ─── Cache subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// List all cached step entries with timestamps and metadata
    List,
    /// Delete all cache entries (forces full rebuild on next run)
    Clean,
    /// Remove only entries older than the configured TTL
    Prune,
}

// ─── Lab subcommands ─────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum LabCommands {
    /// List all active lab environments
    List,
    /// Create and start a new ephemeral lab environment
    Create {
        #[arg(default_value = "alpine")]
        os: String,
    },
    /// Execute a command inside a running lab environment
    Run {
        os: String,
        command: String,
    },
    /// Open an interactive shell inside a lab environment
    Shell {
        #[arg(default_value = "alpine")]
        os: String,
    },
    /// Push the current project files into the lab workspace
    Push {
        #[arg(default_value = "alpine")]
        os: String,
    },
    /// Destroy a lab environment and clean its workspace
    Destroy {
        os: String,
    },
}

// ─── Env subcommands ─────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum EnvCommands {
    /// Download all toolchains declared in .zenith.yml env block
    Init,
    /// Open a shell with Zenith-managed toolchains on PATH
    Shell,
    /// List all installed toolchains and their versions
    List,
    /// Remove all cached toolchain binaries
    Clean,
}

// ─── Matrix action ────────────────────────────────────────────────────────────

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum MatrixAction {
    #[default]
    Run,
    List,
}
