use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// All modules are defined in lib.rs; re-import them here for convenience.
use zenith::cli;
use zenith::cloud;
use zenith::config;
use zenith::daemon;
use zenith::hypervisor;
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
                // Phase 15: try the daemon first for near-zero startup latency
                if !no_cache && daemon::is_running() {
                    let config_yaml = std::fs::read_to_string(".zenith.yml")
                        .context("Cannot read .zenith.yml")?;
                    match daemon::client::try_run_via_daemon(&config_yaml, job.as_deref(), no_cache).await {
                        Ok(true)  => { /* success */ }
                        Ok(false) => anyhow::bail!("Job failed (via daemon)"),
                        Err(e) => {
                            tracing::warn!("Daemon unreachable ({}), falling back to standalone.", e);
                            let cfg = config::load_config(".zenith.yml")?;
                            runner::execute_local(cfg, job, no_cache).await?;
                        }
                    }
                } else {
                    info!("Running workflow{}...", if no_cache { " (--no-cache)" } else { "" });
                    let cfg = config::load_config(".zenith.yml")?;
                    runner::execute_local(cfg, job, no_cache).await?;
                }
            }
        }

        // ─── zenith build ────────────────────────────────────────────────────
        cli::Commands::Build { job, no_cache, derivation } => {
            let cfg = config::load_config(".zenith.yml")?;
            if derivation {
                // Phase 13: dry-run — print derivation JSON for each step
                print_derivations(&cfg, job.as_deref())?;
            } else {
                info!("Building{}...", if no_cache { " (--no-cache)" } else { "" });
                runner::execute_local(cfg, job, no_cache).await?;
            }
        }

        // ─── zenith store ────────────────────────────────────────────────────
        cli::Commands::Store(store_cmd) => {
            handle_store(store_cmd)?;
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

        // ─── zenith tools ────────────────────────────────────────────────────
        cli::Commands::Tools(tools_cmd) => {
            handle_tools(tools_cmd).await?;
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

        // ─── zenith migrate ──────────────────────────────────────────────────
        cli::Commands::Migrate { file, write } => {
            let src = std::fs::read_to_string(&file)
                .with_context(|| format!("Cannot read '{}'", file))?;
            let upgraded = config::migrate_v1_to_v2(&src)?;
            if write {
                std::fs::write(&file, &upgraded)
                    .with_context(|| format!("Cannot write '{}'", file))?;
                println!("Upgraded '{}' to schema v2 in-place.", file);
            } else {
                println!("{}", upgraded);
                eprintln!("\n# Run with --write to apply changes in-place.");
            }
        }

        // ─── zenith benchmark ────────────────────────────────────────────────
        cli::Commands::Benchmark { save_baseline } => {
            handle_benchmark(save_baseline)?;
        }

        // ─── zenith docs ─────────────────────────────────────────────────────
        cli::Commands::Docs => {
            open_docs()?;
        }

        // ─── zenith daemon ───────────────────────────────────────────────────
        cli::Commands::Daemon(daemon_cmd) => {
            handle_daemon(daemon_cmd).await?;
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

        cli::PluginCommands::Search { query } => {
            plugin::registry::search_registry(&query).await?;
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

        cli::CacheCommands::Remote { url, push, status } => {
            if status || url.is_none() {
                let cfg = zenith::build::remote_cache::load_cache_config();
                match cfg.remote {
                    Some(ref u) => {
                        println!("Remote cache URL : {}", u);
                        println!("Auto-push        : {}", cfg.push);
                    }
                    None => {
                        println!("No remote binary cache configured.");
                        println!("Set one with: zenith cache remote <url>");
                    }
                }
            } else if let Some(ref u) = url {
                let cfg = zenith::build::remote_cache::RemoteCacheConfig {
                    remote:  Some(u.clone()),
                    push,
                    api_key: None,
                };
                zenith::build::remote_cache::save_cache_config(&cfg)?;
                println!("Remote cache set: {}", u);
                if push { println!("Auto-push enabled."); }
            }
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

// ─── Tools command handler (Phase 12) ────────────────────────────────────────

async fn handle_tools(cmd: cli::ToolsCommands) -> Result<()> {
    match cmd {
        cli::ToolsCommands::DownloadKernel => {
            let path = zenith::tools::ensure_zenith_kernel().await?;
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("Zenith kernel ready: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
        }
        cli::ToolsCommands::DownloadRootfs => {
            let path = zenith::tools::ensure_zenith_rootfs().await?;
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("Zenith rootfs ready: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
        }
        cli::ToolsCommands::Status => {
            let home = sandbox::zenith_home();
            let items = [
                ("kernel",        "vmlinux-zenith",           "Zenith custom kernel"),
                ("kernel",        "vmlinux",                  "Stock FC kernel"),
                ("rootfs",        "zenith-minimal.tar.gz",    "Zenith minimal rootfs"),
                ("bin",           "firecracker",              "Firecracker VMM"),
                ("bin",           "zenith-agent",             "Zenith remote agent"),
            ];
            println!("{:<30}  {:>10}  {}", "Artefact", "Size", "Path");
            println!("{}", "-".repeat(72));
            for (dir, file, label) in &items {
                let path = home.join(dir).join(file);
                if path.exists() {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    println!("{:<30}  {:>10}  {}", label, human_size(size), path.display());
                } else {
                    println!("{:<30}  {:>10}  (not downloaded)", label, "-");
                }
            }
            let layer_store = sandbox::layer_store::LayerStore::new()?;
            let layers = layer_store.list_layers();
            if !layers.is_empty() {
                println!("\nLayer store ({} layers, {} total):",
                    layers.len(), human_size(layer_store.total_size_bytes()));
                for (hash, meta) in &layers {
                    println!("  {} — {} ({:.1} MB)",
                        &hash[..16], meta.os, meta.size_bytes as f64 / 1_048_576.0);
                }
            }
        }
    }
    Ok(())
}

// ─── Store command handler (Phase 13) ────────────────────────────────────────

fn handle_store(cmd: cli::StoreCommands) -> Result<()> {
    let store = zenith::build::store::BuildStore::new()?;

    match cmd {
        cli::StoreCommands::List => {
            let entries = store.list();
            if entries.is_empty() {
                println!("Build store is empty. Run `zenith build` to populate it.");
                return Ok(());
            }
            println!("{:<18}  {:>10}  {}", "Derivation ID", "Built", "Host");
            println!("{}", "-".repeat(60));
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            for (id, meta) in &entries {
                let age = human_age(now.saturating_sub(meta.built_at_secs));
                println!("{:<18}  {:>10}  {}", &id[..16], age, meta.host);
            }
            println!("\nTotal: {} store entries ({} on disk)",
                entries.len(), human_size(store.total_size_bytes()));
        }

        cli::StoreCommands::Gc { days } => {
            let max_age_secs = days * 86_400;
            let removed = store.gc(max_age_secs)?;
            if removed == 0 {
                println!("Nothing to remove (no entries older than {} days).", days);
            } else {
                println!("GC removed {} store entries older than {} days.", removed, days);
            }
        }

        cli::StoreCommands::Info { id } => {
            let entries = store.list();
            let hit = entries.iter().find(|(eid, _)| eid.starts_with(&id));
            let Some((full_id, meta)) = hit else {
                return Err(anyhow::anyhow!(
                    "No store entry with ID prefix '{}'. Run `zenith store list` to see entries.", id
                ));
            };
            println!("Derivation ID : {}", full_id);
            println!("Built at      : {} ago ({})", human_age(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .saturating_sub(meta.built_at_secs)
            ), meta.built_at_secs);
            println!("Host          : {}", meta.host);
            if let Some(drv) = store.derivation(full_id) {
                println!("Name          : {}", drv.name);
                println!("Command       : {}", drv.command);
                println!("OS / Arch     : {} / {}", drv.os, drv.arch);
                if !drv.env.is_empty() {
                    println!("Env           : {} variable(s)", drv.env.len());
                }
                if !drv.inputs.is_empty() {
                    println!("Inputs        : {} file(s) watched", drv.inputs.len());
                }
                if !drv.outputs.is_empty() {
                    println!("Outputs       : {}", drv.outputs.join(", "));
                }
                if !drv.deps.is_empty() {
                    println!("Deps          : {} upstream derivation(s)", drv.deps.len());
                }
            }
        }
    }
    Ok(())
}

/// Phase 13: dry-run — compute and print derivation JSON for each step in the job.
fn print_derivations(cfg: &config::ZenithConfig, target_job: Option<&str>) -> Result<()> {
    use zenith::build::derivation::Derivation;

    let job = if let Some(ref jobs) = cfg.jobs {
        let name = target_job
            .map(|s| s.to_string())
            .unwrap_or_else(|| jobs.keys().next().cloned().unwrap_or_default());
        jobs.get(&name)
            .ok_or_else(|| anyhow::anyhow!("Job '{}' not found.", name))?
    } else {
        return Err(anyhow::anyhow!(
            "No jobs block found in .zenith.yml. --derivation requires named jobs."
        ));
    };

    let os   = job.runs_on.as_deref().unwrap_or("local");
    let arch = job.arch.as_deref().unwrap_or(std::env::consts::ARCH);
    let env  = job.env.clone().unwrap_or_default();

    println!("Derivations for {} step(s) on {}/{}:\n", job.steps.len(), os, arch);

    for (i, step) in job.steps.iter().enumerate() {
        let drv = Derivation::from_step(step, &env, os, arch);
        println!("── Step {} — {} ──", i + 1, drv.name);
        println!("   ID: {}", drv.id());
        println!("{}", drv.to_json_pretty()
            .lines()
            .map(|l| format!("   {}", l))
            .collect::<Vec<_>>()
            .join("\n"));
        println!();
    }
    Ok(())
}

// ─── Daemon command handler (Phase 15) ───────────────────────────────────────

async fn handle_daemon(cmd: cli::DaemonCommands) -> Result<()> {
    match cmd {
        cli::DaemonCommands::Start { pool } => {
            if daemon::is_running() {
                println!("Daemon is already running.");
                println!("Run `zenith daemon status` for details.");
                return Ok(());
            }

            // Locate the zenith-daemon binary (same directory as the current binary)
            let daemon_bin = std::env::current_exe()?
                .parent()
                .map(|p| p.join(if cfg!(windows) { "zenith-daemon.exe" } else { "zenith-daemon" }))
                .filter(|p| p.exists())
                .ok_or_else(|| anyhow::anyhow!(
                    "zenith-daemon binary not found next to zenith binary.\n\
                     Build with `cargo build` to produce it."
                ))?;

            // Spawn as detached background process
            let mut cmd = std::process::Command::new(&daemon_bin);
            cmd.arg(pool.to_string());

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                // Detach from terminal: redirect stdio to /dev/null
                cmd.stdin(std::process::Stdio::null())
                   .stdout(std::process::Stdio::null())
                   .stderr(std::process::Stdio::null());
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.stdin(std::process::Stdio::null())
                   .stdout(std::process::Stdio::null())
                   .stderr(std::process::Stdio::null())
                   .creation_flags(0x00000008); // DETACHED_PROCESS
            }

            let child = cmd.spawn()
                .context("Failed to spawn zenith-daemon")?;

            println!("Daemon started (PID {}).", child.id());
            println!("Pool target: {} pre-warmed VM(s).", pool);
            println!("Use `zenith daemon status` to monitor.");
        }

        cli::DaemonCommands::Stop => {
            if !daemon::is_running() {
                println!("Daemon is not running.");
                return Ok(());
            }
            match daemon::client::shutdown().await {
                Ok(()) => println!("Daemon shutdown requested."),
                Err(e) => println!("Shutdown failed: {}. The daemon may have already exited.", e),
            }
            // Remove PID file
            let _ = std::fs::remove_file(daemon::pid_file());
        }

        cli::DaemonCommands::Status => {
            if !daemon::is_running() {
                println!("Daemon is not running.");
                println!("Start with: zenith daemon start");
                return Ok(());
            }
            match daemon::client::ping().await {
                Ok(zenith::daemon::protocol::DaemonResponse::Pong { version, pool_ready, pool_target, active_jobs }) => {
                    println!("Daemon is running.");
                    println!("  Version     : {}", version);
                    println!("  Pool ready  : {}/{}", pool_ready, pool_target);
                    println!("  Active jobs : {}", active_jobs);
                }
                Ok(zenith::daemon::protocol::DaemonResponse::StatusInfo { version, pool_ready, pool_target, active_jobs, uptime_secs }) => {
                    println!("Daemon is running.");
                    println!("  Version     : {}", version);
                    println!("  Uptime      : {}", human_age(uptime_secs));
                    println!("  Pool ready  : {}/{}", pool_ready, pool_target);
                    println!("  Active jobs : {}", active_jobs);
                }
                Ok(other) => println!("Unexpected response: {:?}", other),
                Err(e) => println!("Cannot connect to daemon: {}", e),
            }
        }

        cli::DaemonCommands::Restart { pool } => {
            if daemon::is_running() {
                println!("Stopping existing daemon...");
                let _ = daemon::client::shutdown().await;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let _ = std::fs::remove_file(daemon::pid_file());
            }
            // Inline the Start logic to avoid async recursion
            let daemon_bin = std::env::current_exe()?
                .parent()
                .map(|p| p.join(if cfg!(windows) { "zenith-daemon.exe" } else { "zenith-daemon" }))
                .filter(|p| p.exists())
                .ok_or_else(|| anyhow::anyhow!("zenith-daemon binary not found next to zenith binary."))?;
            let mut cmd = std::process::Command::new(&daemon_bin);
            cmd.arg(pool.to_string())
               .stdin(std::process::Stdio::null())
               .stdout(std::process::Stdio::null())
               .stderr(std::process::Stdio::null());
            let child = cmd.spawn().context("Failed to spawn zenith-daemon")?;
            println!("Daemon restarted (PID {}).", child.id());
        }

        cli::DaemonCommands::HypervisorCheck => {
            if hypervisor::is_supported() {
                println!("KVM hypervisor: AVAILABLE");
                println!("The Zenith custom VMM is supported on this machine.");
                println!("Start the daemon to activate the pre-warmed VM pool: zenith daemon start");
            } else {
                println!("KVM hypervisor: UNAVAILABLE");
                println!("Reason: {}", hypervisor::unavailable_reason());
                println!("Zenith will use the Firecracker or container backend instead.");
            }
        }
    }
    Ok(())
}

// ─── Benchmark command handler (Phase 14) ────────────────────────────────────

fn handle_benchmark(save_baseline: bool) -> Result<()> {
    let home = sandbox::zenith_home();
    let baseline_path = home.join("bench-baseline.json");

    println!("Running Zenith performance benchmarks...\n");

    // Micro-benchmarks run inline (no Criterion dependency needed in the binary)
    let results = run_inline_benchmarks()?;

    // Load previous baseline for comparison
    let baseline: Option<serde_json::Value> = if baseline_path.exists() {
        let s = std::fs::read_to_string(&baseline_path)?;
        serde_json::from_str(&s).ok()
    } else {
        None
    };

    println!("{:<30}  {:>12}  {:>10}", "Benchmark", "Time", "vs baseline");
    println!("{}", "-".repeat(58));

    let mut regression = false;
    let mut new_baseline = serde_json::Map::new();

    for (name, ns) in &results {
        let prev_ns = baseline.as_ref()
            .and_then(|b| b[name].as_f64());

        let delta_str = match prev_ns {
            Some(prev) => {
                let pct = ((*ns as f64 - prev) / prev) * 100.0;
                if pct > 10.0 {
                    regression = true;
                    format!("{:+.1}% !", pct)
                } else if pct < -5.0 {
                    format!("{:+.1}% faster", pct)
                } else {
                    format!("{:+.1}%", pct)
                }
            }
            None => "new".to_string(),
        };

        println!("{:<30}  {:>12}  {:>10}",
            name, format_ns(*ns), delta_str);

        new_baseline.insert(name.clone(), serde_json::Value::from(*ns as f64));
    }

    if save_baseline {
        let data = serde_json::Value::Object(new_baseline);
        std::fs::create_dir_all(&home)?;
        std::fs::write(&baseline_path, serde_json::to_string_pretty(&data)?)?;
        println!("\nBaseline saved to {}.", baseline_path.display());
    } else {
        println!("\nRun with --save-baseline to update the comparison baseline.");
    }

    if regression {
        anyhow::bail!("Performance regression detected (>10% slower than baseline).");
    }

    Ok(())
}

/// Run lightweight inline benchmarks — returns (name, median_ns) pairs.
fn run_inline_benchmarks() -> Result<Vec<(String, u64)>> {
    let mut results = Vec::new();

    // config_parse: measure YAML parse + validation time
    results.push(("config_parse".to_string(), bench_ns(100, || {
        let yaml = "version: \"2\"\njobs:\n  build:\n    runs-on: alpine\n    steps:\n      - name: Build\n        run: make\n";
        let _: zenith::config::ZenithConfig = serde_yaml::from_str(yaml).unwrap();
    })?));

    // cache_key_hash: measure SHA-256 hash of a step command
    results.push(("cache_key_hash".to_string(), bench_ns(1000, || {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        h.update(b"cargo build --release -- target/release/myapp");
        let _ = h.finalize();
    })?));

    // derivation_id: measure full derivation compute
    results.push(("derivation_id".to_string(), bench_ns(500, || {
        use std::collections::HashMap;
        let step = zenith::config::Step {
            name: Some("Build".into()),
            run: "cargo build --release".into(),
            env: None,
            working_directory: None,
            allow_failure: false,
            cache: None,
            watch: vec!["src/**/*.rs".into()],
            outputs: vec!["target/release/myapp".into()],
            cache_key: None,
            depends_on: vec![],
        };
        let env: HashMap<String, String> = HashMap::new();
        let drv = zenith::build::derivation::Derivation::from_step(&step, &env, "alpine", "x86_64");
        let _ = drv.id();
    })?));

    Ok(results)
}

fn bench_ns<F: Fn()>(iters: u64, f: F) -> Result<u64> {
    // Warm up
    for _ in 0..10 { f(); }

    let start = std::time::Instant::now();
    for _ in 0..iters { f(); }
    let elapsed = start.elapsed().as_nanos() as u64;

    Ok(elapsed / iters)
}

fn format_ns(ns: u64) -> String {
    if ns < 1_000 { format!("{}ns", ns) }
    else if ns < 1_000_000 { format!("{:.2}µs", ns as f64 / 1_000.0) }
    else { format!("{:.2}ms", ns as f64 / 1_000_000.0) }
}

// ─── Docs command handler (Phase 14) ─────────────────────────────────────────

fn open_docs() -> Result<()> {
    // Try to open the locally-built mdBook site, fall back to the hosted URL.
    let local_index = std::path::Path::new("book/index.html");
    let url = if local_index.exists() {
        format!("file://{}", local_index.canonicalize()?.display())
    } else {
        "https://zenith.run/docs".to_string()
    };

    println!("Opening docs: {}", url);

    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&url).spawn().ok();
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(&url).spawn().ok();
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/c", "start", &url]).spawn().ok();

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn human_age(secs: u64) -> String {
    if secs < 60 { format!("{}s", secs) }
    else if secs < 3600 { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else { format!("{}d", secs / 86400) }
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{}B", bytes) }
    else if bytes < 1_048_576 { format!("{:.1}KB", bytes as f64 / 1024.0) }
    else if bytes < 1_073_741_824 { format!("{:.1}MB", bytes as f64 / 1_048_576.0) }
    else { format!("{:.2}GB", bytes as f64 / 1_073_741_824.0) }
}
