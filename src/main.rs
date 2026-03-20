use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod config;
mod runner;
mod sandbox;
mod toolchain;
mod tools;

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
                info!("Remote execution on '{}' (Phase 9 — not yet implemented).", target);
                println!("Remote runner coming in Phase 9. Run locally for now.");
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
