/// WebAssembly / WASI Backend
///
/// Motto: Zenith downloads wasmtime automatically on first use.
/// Users only need to provide a .wasm file — Zenith handles the runtime.
///
/// wasmtime is downloaded from Bytecode Alliance GitHub releases into
/// ~/.zenith/bin/wasmtime and reused for all subsequent WASM executions.
/// Works on Linux, macOS, and Windows.

use super::backend::Backend;
use anyhow::{Context, Result};
use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{info, warn};

pub struct WasmBackend;

#[async_trait]
impl Backend for WasmBackend {
    fn name(&self) -> &str {
        "wasm"
    }

    async fn provision(&self, lab_id: &str, _base_os: &str, _target_arch: &str) -> Result<()> {
        info!("[Wasm] Initializing WASI runtime for lab: {}", lab_id);

        // Motto: Zenith downloads wasmtime — the user installs nothing.
        let wasmtime_path = crate::tools::ensure_wasmtime().await?;
        info!("[Wasm] wasmtime ready at {:?}", wasmtime_path);

        // Set up the workspace directory
        let workspace = super::lab_state_dir(lab_id).join("workspace");
        std::fs::create_dir_all(&workspace).context("Failed to create wasm workspace")?;

        // Copy the current project into the workspace
        let current_dir = std::env::current_dir()?;
        super::copy_dir_all(&current_dir, &workspace)?;

        Ok(())
    }

    async fn execute(
        &self,
        lab_id: &str,
        _base_os: &str,
        _target_arch: &str,
        cmd: &str,
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>
    ) -> Result<()> {
        info!("[Wasm] Executing: {}", cmd);

        // Ensure wasmtime is ready
        let wasmtime_path = crate::tools::ensure_wasmtime().await?;

        let workspace = super::lab_state_dir(lab_id).join("workspace");

        // Determine the .wasm file path
        // cmd can be: "app.wasm", "app.wasm --arg1 val", or a full path
        let (wasm_file, extra_args) = parse_wasm_command(cmd, &workspace);

        if !wasm_file.exists() {
            warn!("[Wasm] Module not found at {:?}. Searched workspace: {:?}", wasm_file, workspace);
            return Err(anyhow::anyhow!(
                "WASM module not found: '{}'\n\
                 Place the .wasm file in your project directory and push it with:\n  zenith lab push wasm",
                wasm_file.display()
            ));
        }

        info!("[Wasm] Running module: {:?}", wasm_file);

        // Build the wasmtime invocation:
        // wasmtime run --dir . <file.wasm> -- <extra args>
        let mut command = tokio::process::Command::new(&wasmtime_path);
        command.arg("run");

        // Grant WASI access to the workspace directory
        command.arg("--dir").arg(&workspace);

        // Set working directory for the wasmtime process itself
        let wd = working_directory
            .map(|d| workspace.join(d))
            .unwrap_or_else(|| workspace.clone());
        command.current_dir(&wd);

        // Pass environment variables to the WASM module via --env
        if let Some(env_vars) = env {
            for (k, v) in env_vars {
                command.arg("--env").arg(format!("{}={}", k, v));
            }
        }

        command.arg(&wasm_file);

        // Extra args after the wasm file (e.g. "app.wasm --verbose")
        for arg in extra_args {
            command.arg(arg);
        }

        command
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        let mut child = command
            .spawn()
            .context(format!("Failed to spawn wasmtime for: {}", cmd))?;

        let status = child.wait().await?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "WASM module exited with non-zero status: {}",
                status.code().unwrap_or(-1)
            ));
        }

        info!("[Wasm] Module completed successfully.");
        Ok(())
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        info!("[Wasm] Tearing down WASI session '{}'.", lab_id);
        super::teardown_lab(lab_id).await
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Split `cmd` into (wasm_path, extra_args).
/// Examples:
///   "app.wasm"              → (workspace/app.wasm, [])
///   "app.wasm --foo bar"    → (workspace/app.wasm, ["--foo", "bar"])
///   "/abs/path/app.wasm"    → (/abs/path/app.wasm, [])
fn parse_wasm_command(cmd: &str, workspace: &std::path::Path) -> (std::path::PathBuf, Vec<String>) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let wasm_part = parts[0];
    let extra: Vec<String> = if parts.len() > 1 {
        parts[1].split_whitespace().map(String::from).collect()
    } else {
        vec![]
    };

    let wasm_path = if std::path::Path::new(wasm_part).is_absolute() {
        std::path::PathBuf::from(wasm_part)
    } else {
        workspace.join(wasm_part)
    };

    (wasm_path, extra)
}
