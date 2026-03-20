use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// All modules are defined in lib.rs; re-import them here for convenience.
use zenith::cli;
use zenith::cloud;
use zenith::config;
use zenith::plugin;
use zenith::remote;
use zenith::runner;
use zenith::sandbox;
use zenith::toolchain;
use zenith::tui;
use zenith::ui;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    let args = cli::Cli::parse();

    match args.command {

        // ─── zenith run ──────────────────────────────────────────────────────
        cli::Commands::Run { job, no_cache, remote } => {
            if let Some(ref target) = remote {
                let remote_cfg = remote::config::get_remote(target)?;
                let config_yaml = std::fs::read_to_string(".zenith.yml")
                    .context("Cannot read .zenith.yml")?;
                remote::runner::execute_remote(target, &remote_cfg, &config_yaml, job.as_deref()).await?;
            } else {
                info!("Running workflow{}...", if no_cache { " (--no-cache)" } else { "" });
                let cfg = config::load_config(".zenith.yml")?;
                runner::execute_local(cfg, job, no_cache).await?;
            }
        }

        // ─── zenith build ────────────────────────────────────────────────────
        cli::Commands::Build { job, no_cache } => {
            info!("Building{}...", if no_cache { " (--no-cache)" } else { "" });
            let cfg = config::load_config(".zenith.yml")?;
            runner::execute_local(cfg, job, no_cache).await?;
        }

        // ─── zenith cache ────────────────────────────────────────────────────
        cli::Commands::Cache(cache_cmd) => {
            handle_cache(cache_cmd)?;
        }

        // ─── zenith lab ──────────────────────────────────────────────────────
        cli::Commands::Lab(lab_cmd) => {
            sandbox::handle_lab(lab_cmd).await?;
        }

        // ─── zenith env ──────────────────────────────────────────────────────
        cli::Commands::Env(env_cmd) => {
            handle_env(env_cmd).await?;
        }

        // ─── zenith matrix ───────────────────────────────────────────────────
        cli::Commands::Matrix { action, no_cache } => {
            match action {
                cli::MatrixAction::Run => {
                    info!("Matrix run{}...", if no_cache { " (--no-cache)" } else { "" });
                    let cfg = config::load_config(".zenith.yml")?;
                    runner::execute_local(cfg, None, no_cache).await?;
                }
                cli::MatrixAction::List => {
                    let cfg = config::load_config(".zenith.yml")?;
                    if let Some(jobs) = cfg.jobs {
                        println!("Jobs defined in .zenith.yml:");
                        for (name, job) in &jobs {
                            if let Some(ref strategy) = job.strategy {
                                let matrix_str: Vec<String> = strategy.matrix.iter()
                                    .map(|(k, v)| format!("{}=[{}]", k, v.join(", ")))
                                    .collect();
                                println!("  {} → matrix: {}", name, matrix_str.join(", "));
                            } else {
                                println!("  {} (no matrix)", name);
                            }
                        }
                    } else {
                        println!("No jobs defined.");
                    }
                }
            }
        }

        // ─── zenith remote ───────────────────────────────────────────────────
        cli::Commands::Remote(remote_cmd) => {
            handle_remote(remote_cmd).await?;
        }

        // ─── zenith cloud ────────────────────────────────────────────────────
        cli::Commands::Cloud(cloud_cmd) => {
            handle_cloud(cloud_cmd).await?;
        }

        // ─── zenith plugin ───────────────────────────────────────────────────
        cli::Commands::Plugin(plugin_cmd) => {
            handle_plugin(plugin_cmd).await?;
        }

        // ─── zenith ui ───────────────────────────────────────────────────────
        cli::Commands::Ui { port } => {
            ui::server::serve(port).await?;
        }

        // ─── zenith tui ──────────────────────────────────────────────────────
        cli::Commands::Tui => {
            tui::run()?;
        }

        // ─── zenith shell ────────────────────────────────────────────────────
        cli::Commands::Shell { lab } => {
            if let Some(os) = lab {
                // Open shell inside a specific lab environment
                sandbox::handle_lab(cli::LabCommands::Shell { os }).await?;
            } else {
                // Open the host shell with Zenith toolchains on PATH (Phase 7)
                handle_env(cli::EnvCommands::Shell).await?;
            }
        }
    }

    Ok(())
}

// ─── Remote command handler ───────────────────────────────────────────────────

async fn handle_remote(cmd: cli::RemoteCommands) -> Result<()> {
    match cmd {
        cli::RemoteCommands::Add { name, host, port, key } => {
            remote::config::add_remote(&name, &host, port.unwrap_or(22), key)?;
            println!("Remote '{}' added ({}).", name, host);
            println!("Test with: zenith remote status {}", name);
        }

        cli::RemoteCommands::List => {
            let remotes = remote::config::list_remotes()?;
            if remotes.is_empty() {
                println!("No remotes registered. Use `zenith remote add <name> <user@host>`.");
                return Ok(());
            }
            println!("{:<20}  {:<30}  {}", "Name", "Host", "Port");
            println!("{}", "-".repeat(60));
            for (name, r) in &remotes {
                println!("{:<20}  {:<30}  {}", name, r.host, r.port);
            }
        }

        cli::RemoteCommands::Remove { name } => {
            remote::config::remove_remote(&name)?;
            println!("Remote '{}' removed.", name);
        }

        cli::RemoteCommands::Status { name } => {
            let r = remote::config::get_remote(&name)?;
            print!("Pinging '{}' ({})... ", name, r.host);
            match remote::transport::ping(&r).await {
                Ok(arch) => println!("OK (arch: {})", arch),
                Err(e)   => println!("FAILED\n  {}", e),
            }
        }
    }
    Ok(())
}

// ─── Cloud command handler ────────────────────────────────────────────────────

async fn handle_cloud(cmd: cli::CloudCommands) -> Result<()> {
    match cmd {
        cli::CloudCommands::Login { api_key } => {
            cloud::client::save_api_key(&api_key)?;
            println!("API key saved. You can now run `zenith cloud run`.");
        }

        cli::CloudCommands::Logout => {
            cloud::client::clear_api_key()?;
            println!("Logged out — API key removed from ~/.zenith/config.toml.");
        }

        cli::CloudCommands::Run { job, watch } => {
            let cfg = cloud::client::load_cloud_config();
            let client = cloud::client::CloudClient::new(cfg);

            let config_yaml = std::fs::read_to_string(".zenith.yml")
                .context("Cannot read .zenith.yml")?;
            let local_dir = std::env::current_dir()?;
            let tarball = cloud::packager::package_project(&local_dir)?;

            info!("Submitting workflow to Zenith cloud ({} bytes)...", tarball.len());
            let run_id = client.submit_run(&config_yaml, tarball, job.as_deref()).await?;
            println!("Run submitted: {}", run_id);

            if watch {
                println!("Streaming logs (Ctrl+C to detach)...");
                client.stream_logs(&run_id).await?;
            } else {
                println!("Track with: zenith cloud status {}", run_id);
                println!("Logs with:  zenith cloud logs {}", run_id);
            }
        }

        cli::CloudCommands::Status { run_id } => {
            let cfg = cloud::client::load_cloud_config();
            let client = cloud::client::CloudClient::new(cfg);
            let info = client.get_status(&run_id).await?;
            println!("Run:     {}", info.run_id);
            println!("Status:  {}", info.status);
            println!("Created: {}", info.created_at);
            println!("Updated: {}", info.updated_at);
        }

        cli::CloudCommands::Logs { run_id } => {
            let cfg = cloud::client::load_cloud_config();
            let client = cloud::client::CloudClient::new(cfg);
            client.stream_logs(&run_id).await?;
        }

        cli::CloudCommands::Cancel { run_id } => {
            let cfg = cloud::client::load_cloud_config();
            let client = cloud::client::CloudClient::new(cfg);
            client.cancel_run(&run_id).await?;
            println!("Run '{}' cancelled.", run_id);
        }

        cli::CloudCommands::List => {
            let cfg = cloud::client::load_cloud_config();
            let client = cloud::client::CloudClient::new(cfg);
            let runs = client.list_runs().await?;
            if runs.is_empty() {
                println!("No cloud runs found.");
                return Ok(());
            }
            println!("{:<36}  {:<12}  {}", "Run ID", "Status", "Created");
            println!("{}", "-".repeat(70));
            for r in &runs {
                println!("{:<36}  {:<12}  {}", r.run_id, r.status, r.created_at);
            }
        }
    }
    Ok(())
}

// ─── Plugin command handler ───────────────────────────────────────────────────

async fn handle_plugin(cmd: cli::PluginCommands) -> Result<()> {
    match cmd {
        cli::PluginCommands::List => {
            let plugins = plugin::registry::discover_plugins();
            if plugins.is_empty() {
                println!("No plugins installed. Use `zenith plugin install <path>` to add one.");
                return Ok(());
            }
            println!("{:<24}  {:<10}  {:<12}  {}", "Name", "Version", "Type", "Description");
            println!("{}", "-".repeat(72));
            for p in &plugins {
                println!("{:<24}  {:<10}  {:<12}  {}",
                    p.name, p.version, p.plugin_type.to_string(),
                    p.description.as_deref().unwrap_or("-"));
            }
            println!("\nTotal: {} plugin(s)", plugins.len());
        }

        cli::PluginCommands::Install { path } => {
            let src = std::path::Path::new(&path);
            if !src.is_dir() {
                return Err(anyhow::anyhow!(
                    "'{}' is not a directory. Provide the path to a plugin directory containing plugin.toml.",
                    path
                ));
            }
            info!("Installing plugin from {:?}...", src);
            let manifest = plugin::registry::install_from_path(src)?;

            // Smoke test: spawn the plugin and call `name`
            print!("Running smoke test... ");
            match plugin::client::smoke_test(&manifest).await {
                Ok(reported_name) => {
                    println!("OK (plugin reports name: '{}')", reported_name);
                }
                Err(e) => {
                    println!("WARNING — smoke test failed: {}", e);
                    println!("Plugin installed but may not work correctly.");
                }
            }

            println!("Plugin '{}' v{} installed successfully.", manifest.name, manifest.version);
            println!("Use `backend: {}` in .zenith.yml to activate it.", manifest.name);
        }

        cli::PluginCommands::Remove { name } => {
            plugin::registry::remove_plugin(&name)?;
            println!("Plugin '{}' removed.", name);
        }

        cli::PluginCommands::Info { name } => {
            let Some(p) = plugin::registry::find_plugin(&name) else {
                return Err(anyhow::anyhow!("Plugin '{}' is not installed.", name));
            };
            println!("Name:        {}", p.name);
            println!("Version:     {}", p.version);
            println!("Type:        {}", p.plugin_type);
            println!("Entrypoint:  {:?}", p.entrypoint_path());
            println!("Install dir: {:?}", p.install_dir);
            if let Some(desc) = &p.description {
                println!("Description: {}", desc);
            }
        }
    }
    Ok(())
}

// ─── Cache command handler ────────────────────────────────────────────────────

fn handle_cache(cmd: cli::CacheCommands) -> Result<()> {
    let cm = sandbox::cache::CacheManager::new()?;

    match cmd {
        cli::CacheCommands::List => {
            let entries = cm.list_entries();
            if entries.is_empty() {
                println!("No cache entries found.");
                return Ok(());
            }
            println!("{:<16}  {:>8}  {:<12}  {:<10}  {}",
                "Hash", "Age", "OS", "Arch", "Command");
            println!("{}", "-".repeat(72));
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            for (hash, entry) in &entries {
                let age_secs = now.saturating_sub(entry.created_at_secs);
                let age_str = human_age(age_secs);
                let artifact_marker = if entry.has_artifacts { " [+artifacts]" } else { "" };
                println!("{:<16}  {:>8}  {:<12}  {:<10}  {}{}",
                    &hash[..16], age_str, entry.os, entry.arch,
                    &entry.run[..entry.run.len().min(40)], artifact_marker);
            }
            println!("\nTotal: {} entries", entries.len());
        }

        cli::CacheCommands::Clean => {
            let count = cm.clean_all()?;
            println!("Removed {} cache entries. Next run will rebuild from scratch.", count);
        }

        cli::CacheCommands::Prune => {
            let count = cm.clean_expired()?;
            println!("Pruned {} expired cache entries.", count);
        }
    }
    Ok(())
}

// ─── Env command handler ──────────────────────────────────────────────────────

async fn handle_env(cmd: cli::EnvCommands) -> Result<()> {
    match cmd {
        cli::EnvCommands::Init => {
            let cfg = config::load_config(".zenith.yml")?;
            let Some(env_cfg) = cfg.env else {
                println!("No 'env:' block found in .zenith.yml.");
                println!("Example:\n  env:\n    node: \"20\"\n    python: \"3.12.3\"");
                return Ok(());
            };
            info!("Initialising toolchains declared in .zenith.yml...");
            let env_map = toolchain::resolve_toolchain_env_from_config(&env_cfg).await;
            if env_map.is_empty() {
                println!("No toolchains were installed (check version strings or network).");
            } else {
                println!("Toolchains ready. PATH prefix: {}", env_map.get("PATH").map(|p| {
                    p.split(if cfg!(target_os = "windows") { ';' } else { ':' })
                        .take(4)
                        .collect::<Vec<_>>()
                        .join(":")
                }).unwrap_or_default());
            }
        }

        cli::EnvCommands::Shell => {
            // Try to read toolchain config; fall back gracefully
            let env_map = if let Ok(cfg) = config::load_config(".zenith.yml") {
                if let Some(env_cfg) = cfg.env {
                    toolchain::resolve_toolchain_env_from_config(&env_cfg).await
                } else {
                    std::collections::HashMap::new()
                }
            } else {
                std::collections::HashMap::new()
            };

            let shell = std::env::var("SHELL")
                .unwrap_or_else(|_| if cfg!(target_os = "windows") {
                    "cmd".into()
                } else {
                    "/bin/sh".into()
                });

            println!("Opening Zenith shell (Ctrl+D or 'exit' to leave)...");
            if let Some(path) = env_map.get("PATH") {
                println!("Toolchain PATH: {}", path.split(':').take(3).collect::<Vec<_>>().join(":"));
            }

            let mut cmd = std::process::Command::new(&shell);
            for (k, v) in &env_map { cmd.env(k, v); }
            cmd.status().ok();
        }

        cli::EnvCommands::List => {
            let installed = toolchain::list_installed();
            if installed.is_empty() {
                println!("No toolchains installed. Run `zenith env init` to install.");
                return Ok(());
            }
            println!("{:<12}  {:<12}  {}", "Toolchain", "Version", "Path");
            println!("{}", "-".repeat(60));
            for (name, version, path) in &installed {
                println!("{:<12}  {:<12}  {}", name, version, path.display());
            }
        }

        cli::EnvCommands::Clean => {
            let count = toolchain::clean_all()?;
            println!("Removed {} toolchain installation(s) from ~/.zenith/toolchains/.", count);
        }
    }
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn human_age(secs: u64) -> String {
    if secs < 60 { format!("{}s", secs) }
    else if secs < 3600 { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else { format!("{}d", secs / 86400) }
}
