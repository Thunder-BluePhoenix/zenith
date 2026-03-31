/// Wine Backend — Phase 5: Windows .exe execution on Linux
///
/// Motto: "You install Zenith. Zenith installs everything else."
///
/// Zenith automatically downloads a portable Wine build (Kron4ek's Wine-Builds)
/// into ~/.zenith/wine/<version>/ — no `apt install wine` required.
///
/// Usage in .zenith.yml:
///   jobs:
///     test-windows:
///       runs-on: windows-wine
///       backend: wine
///       steps:
///         - run: app.exe --test
///
/// Zenith sets up an isolated WINEPREFIX per lab so Wine configuration
/// from one job never bleeds into another.

use super::backend::Backend;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use async_trait::async_trait;
use tracing::info;

pub struct WineBackend;

#[async_trait]
impl Backend for WineBackend {
    fn name(&self) -> &str { "wine" }

    async fn provision(&self, lab_id: &str, _base_os: &str, _target_arch: &str) -> Result<()> {
        info!("[Wine] Initializing Wine environment for lab: {}", lab_id);

        #[cfg(target_os = "linux")]
        {
            // Motto: Zenith downloads Wine — the user installs nothing.
            let wine_bin = crate::tools::ensure_wine().await?;
            info!("[Wine] Binary ready at {:?}", wine_bin);

            // Create per-lab workspace and an isolated WINEPREFIX
            let workspace  = super::lab_state_dir(lab_id).join("workspace");
            let wineprefix = super::lab_state_dir(lab_id).join("wineprefix");
            std::fs::create_dir_all(&workspace)  .context("Failed to create Wine workspace")?;
            std::fs::create_dir_all(&wineprefix) .context("Failed to create WINEPREFIX")?;

            // Copy the current project into the isolated workspace
            let current = std::env::current_dir()?;
            super::copy_dir_all(&current, &workspace)?;

            // Run `wineboot --init` to initialise the prefix silently
            let status = std::process::Command::new(&wine_bin)
                .arg("wineboot")
                .arg("--init")
                .env("WINEPREFIX", &wineprefix)
                .env("WINEDEBUG", "-all")        // suppress Wine debug noise
                .env("DISPLAY", "")              // no GUI needed for CI
                .status()
                .context("wineboot --init failed")?;

            if !status.success() {
                warn!("[Wine] wineboot init returned non-zero ({}). Prefix may be partial.", status);
            }

            info!("[Wine] Wine prefix initialised at {:?}", wineprefix);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(anyhow::anyhow!(
                "Wine backend is Linux-only.\n\
                 On Windows, .exe files run natively — use 'backend: container'.\n\
                 On macOS, consider CrossOver or a Linux VM."
            ))
        }
    }

    async fn execute(
        &self,
        lab_id: &str,
        _base_os: &str,
        _target_arch: &str,
        cmd: &str,
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>,
    ) -> Result<()> {
        // lab_id / env / working_directory are used in the Linux-only cfg block.
        #[allow(unused_variables)]
        let (lab_id, env, working_directory) = (lab_id, env, working_directory);

        info!("[Wine] Executing: {}", cmd);

        #[cfg(target_os = "linux")]
        {
            let wine_bin   = crate::tools::ensure_wine().await?;
            let workspace  = super::lab_state_dir(lab_id).join("workspace");
            let wineprefix = super::lab_state_dir(lab_id).join("wineprefix");

            // Resolve working directory
            let wd: PathBuf = working_directory
                .map(|d| workspace.join(d))
                .unwrap_or_else(|| workspace.clone());

            // Parse `cmd`: may be `app.exe`, `app.exe --args`, or a path
            let (exe, extra_args) = parse_exe_cmd(cmd, &workspace);

            info!("[Wine] Running {:?} with Wine", exe);

            let mut command = tokio::process::Command::new(&wine_bin);
            command.arg(&exe);
            for a in &extra_args { command.arg(a); }

            command
                .current_dir(&wd)
                .env("WINEPREFIX", &wineprefix)
                .env("WINEDEBUG", "-all")
                .env("DISPLAY", "")
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit());

            if let Some(env_vars) = env {
                for (k, v) in env_vars { command.env(k, v); }
            }

            let mut child = command.spawn()
                .context(format!("Failed to launch Wine for: {}", cmd))?;

            let status = child.wait().await?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Wine exe exited with code {}: {}",
                    status.code().unwrap_or(-1), cmd
                ));
            }

            info!("[Wine] Execution completed successfully.");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(anyhow::anyhow!("Wine backend is Linux-only."))
        }
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        info!("[Wine] Cleaning up Wine lab '{}'.", lab_id);
        super::teardown_lab(lab_id).await
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Split `cmd` into (exe_path, extra_args).
/// Handles: `app.exe`, `app.exe --flag`, `C:\path\app.exe`, relative `bin/app.exe`
#[allow(dead_code)]
fn parse_exe_cmd(cmd: &str, workspace: &std::path::Path) -> (PathBuf, Vec<String>) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let exe_part = parts[0];
    let extra: Vec<String> = if parts.len() > 1 {
        parts[1].split_whitespace().map(String::from).collect()
    } else {
        vec![]
    };

    // If it's an absolute path or contains backslashes it's Windows-native
    let exe_path = if exe_part.contains('\\') || std::path::Path::new(exe_part).is_absolute() {
        PathBuf::from(exe_part)
    } else {
        workspace.join(exe_part)
    };

    (exe_path, extra)
}
