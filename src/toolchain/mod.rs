/// Phase 7: Environment & Package System
///
/// Motto: "You install Zenith. Zenith installs everything else."
///
/// Zenith manages language runtime versions declared in .zenith.yml:
///
///   env:
///     node:   "20"
///     python: "3.12.3"
///     go:     "1.22"
///     rust:   "1.78.0"
///
/// Zenith downloads exact versioned binaries into ~/.zenith/toolchains/<name>/<version>/
/// and prepends their bin/ directories to PATH before every workflow step.
/// No nvm, pyenv, rbenv, or system packages required.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn, debug};
use crate::config::{EnvConfig, Job};

pub mod node;
pub mod python;
pub mod go;
pub mod rust_tc;

// ─── Toolchain trait ─────────────────────────────────────────────────────────

/// Every language toolchain implements this trait.
/// The only required behaviour: ensure the binaries are on disk and return the bin dir.
pub trait Toolchain {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    /// Ensure the toolchain is installed. Returns the bin/ directory to prepend to PATH.
    fn ensure_installed(&self) -> impl std::future::Future<Output = Result<PathBuf>> + Send;
}

// ─── Toolchain root dir ───────────────────────────────────────────────────────

pub fn toolchain_dir(name: &str, version: &str) -> PathBuf {
    crate::sandbox::zenith_home()
        .join("toolchains")
        .join(name)
        .join(version)
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Build an environment map containing the merged PATH for all declared toolchains.
/// Call this before executing steps so every step gets the right runtime on PATH.
///
/// Returns an empty map on non-Linux/macOS (Windows toolchain support is partial).
pub async fn resolve_toolchain_env(job: &Job) -> HashMap<String, String> {
    // Merge: top-level config env is NOT available in runner context (we only have Job).
    // Per-job toolchain block takes precedence; if absent we return empty.
    // The caller (runner.rs) already merges this into the step env.
    let Some(ref tc) = job.toolchain else {
        return HashMap::new();
    };
    build_env_for_config(tc).await
}

/// Resolve toolchain env from an explicit EnvConfig (used by `zenith env shell`).
pub async fn resolve_toolchain_env_from_config(cfg: &EnvConfig) -> HashMap<String, String> {
    build_env_for_config(cfg).await
}

async fn build_env_for_config(cfg: &EnvConfig) -> HashMap<String, String> {
    let mut bin_dirs: Vec<PathBuf> = Vec::new();

    // Node.js
    if let Some(ref v) = cfg.node {
        match node::NodeToolchain::new(v).ensure_installed().await {
            Ok(bin) => { info!("Toolchain: node {} → {:?}", v, bin); bin_dirs.push(bin); }
            Err(e)  => warn!("Node.js {} not available: {}", v, e),
        }
    }

    // Python
    if let Some(ref v) = cfg.python {
        match python::PythonToolchain::new(v).ensure_installed().await {
            Ok(bin) => { info!("Toolchain: python {} → {:?}", v, bin); bin_dirs.push(bin); }
            Err(e)  => warn!("Python {} not available: {}", v, e),
        }
    }

    // Go
    if let Some(ref v) = cfg.go {
        match go::GoToolchain::new(v).ensure_installed().await {
            Ok(bin) => { info!("Toolchain: go {} → {:?}", v, bin); bin_dirs.push(bin); }
            Err(e)  => warn!("Go {} not available: {}", v, e),
        }
    }

    // Rust
    if let Some(ref v) = cfg.rust {
        match rust_tc::RustToolchain::new(v).ensure_installed().await {
            Ok(bin) => { info!("Toolchain: rust {} → {:?}", v, bin); bin_dirs.push(bin); }
            Err(e)  => warn!("Rust {} not available: {}", v, e),
        }
    }

    if bin_dirs.is_empty() {
        return HashMap::new();
    }

    // Build a PATH that prepends all toolchain bin dirs before the system PATH
    let system_path = std::env::var("PATH").unwrap_or_default();
    let zenith_path = bin_dirs.iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(path_separator());

    let full_path = if system_path.is_empty() {
        zenith_path
    } else {
        format!("{}{}{}", zenith_path, path_separator(), system_path)
    };

    let mut env = HashMap::new();
    env.insert("PATH".to_string(), full_path);
    env
}

#[cfg(target_os = "windows")]
fn path_separator() -> &'static str { ";" }
#[cfg(not(target_os = "windows"))]
fn path_separator() -> &'static str { ":" }

// ─── List installed toolchains ────────────────────────────────────────────────

pub fn list_installed() -> Vec<(String, String, PathBuf)> {
    let base = crate::sandbox::zenith_home().join("toolchains");
    let mut result = Vec::new();

    let Ok(names) = std::fs::read_dir(&base) else { return result };
    for name_entry in names.flatten() {
        let name = name_entry.file_name().to_string_lossy().to_string();
        let Ok(versions) = std::fs::read_dir(name_entry.path()) else { continue };
        for ver_entry in versions.flatten() {
            let version = ver_entry.file_name().to_string_lossy().to_string();
            let path = ver_entry.path();
            result.push((name.clone(), version, path));
        }
    }
    result.sort();
    result
}

/// Remove all cached toolchain binaries.
pub fn clean_all() -> Result<usize> {
    let base = crate::sandbox::zenith_home().join("toolchains");
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&base) {
        for e in entries.flatten() {
            std::fs::remove_dir_all(e.path())?;
            count += 1;
        }
    }
    Ok(count)
}
