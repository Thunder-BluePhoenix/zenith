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
        /// Print the derivation JSON for each step without executing (dry-run)
        #[arg(long, default_value_t = false)]
        derivation: bool,
    },

    /// Manage the content-addressable build store (Phase 13)
    #[command(subcommand)]
    Store(StoreCommands),

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

    /// Manage Zenith plugins (Phase 8)
    #[command(subcommand)]
    Plugin(PluginCommands),

    /// Manage remote SSH build machines (Phase 9)
    #[command(subcommand)]
    Remote(RemoteCommands),

    /// Run workflows on the Zenith cloud service (Phase 10)
    #[command(subcommand)]
    Cloud(CloudCommands),

    /// Start the web dashboard (Phase 11)
    Ui {
        /// Port to listen on (default: 7622)
        #[arg(long, default_value_t = 7622)]
        port: u16,
    },

    /// Open the terminal (TUI) dashboard (Phase 11)
    Tui,

    /// Download and manage Zenith low-level tools (Phase 12)
    #[command(subcommand)]
    Tools(ToolsCommands),
}

// ─── Store subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum StoreCommands {
    /// List all build-store entries with timestamps
    List,
    /// Remove store entries older than N days
    Gc {
        /// Maximum age in days before an entry is removed (default: 30)
        #[arg(default_value_t = 30)]
        days: u64,
    },
    /// Show the derivation that produced a store entry
    Info {
        /// Derivation ID hex prefix (at least 8 characters)
        id: String,
    },
}

// ─── Tools subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ToolsCommands {
    /// Download the Zenith custom kernel to ~/.zenith/kernel/vmlinux-zenith
    DownloadKernel,
    /// Download the Zenith minimal rootfs to ~/.zenith/rootfs/zenith-minimal.tar.gz
    DownloadRootfs,
    /// Show paths and sizes of all downloaded low-level tools
    Status,
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

// ─── Plugin subcommands ───────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum PluginCommands {
    /// List all installed plugins
    List,
    /// Install a plugin from a local directory path
    Install {
        /// Path to the plugin directory containing plugin.toml
        path: String,
    },
    /// Remove an installed plugin
    Remove {
        /// Plugin name (as declared in plugin.toml)
        name: String,
    },
    /// Show full details of an installed plugin
    Info {
        /// Plugin name
        name: String,
    },
}

// ─── Remote subcommands ───────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum RemoteCommands {
    /// Register a remote SSH build machine
    Add {
        /// Short name for the remote (e.g., build-server)
        name: String,
        /// SSH target in user@host format
        host: String,
        /// SSH port (default: 22)
        #[arg(long)]
        port: Option<u16>,
        /// Path to SSH private key (optional, uses SSH agent if omitted)
        #[arg(long)]
        key: Option<String>,
    },
    /// List all registered remotes
    List,
    /// Unregister a remote
    Remove {
        name: String,
    },
    /// Ping a remote and show its status
    Status {
        name: String,
    },
}

// ─── Cloud subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum CloudCommands {
    /// Authenticate with the Zenith cloud service
    Login {
        /// API key from https://zenith.run/settings
        api_key: String,
    },
    /// Remove stored cloud credentials
    Logout,
    /// Submit the current workflow to Zenith cloud
    Run {
        /// Run a specific named job
        #[arg(short, long)]
        job: Option<String>,
        /// Stream logs until the run completes
        #[arg(long, default_value_t = false)]
        watch: bool,
    },
    /// Show the status of a cloud run
    Status {
        run_id: String,
    },
    /// Stream logs for a cloud run
    Logs {
        run_id: String,
    },
    /// Cancel a running cloud job
    Cancel {
        run_id: String,
    },
    /// List recent cloud runs
    List,
}

// ─── Matrix action ────────────────────────────────────────────────────────────

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum MatrixAction {
    #[default]
    Run,
    List,
}
