use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;
use std::path::Path;

// ─── Top-level config ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ZenithConfig {
    /// Schema version — "2" enables all new features (default = "1")
    #[serde(default = "default_version")]
    pub version: String,

    /// Top-level declarative toolchain versions (Phase 7: Env & Package System)
    /// Example:
    ///   env:
    ///     node: "20"
    ///     python: "3.12.3"
    ///     go: "1.22"
    pub env: Option<EnvConfig>,

    /// Named jobs (GitHub Actions style)
    pub jobs: Option<HashMap<String, Job>>,

    /// Simple single-job flat steps (legacy / quick-run format)
    pub steps: Option<Vec<Step>>,
}

fn default_version() -> String { "1".into() }

// ─── Phase 7: Toolchain declarations ─────────────────────────────────────────

/// Declarative runtime/toolchain versions Zenith provisions automatically.
/// All versions are pinned strings — Zenith downloads exact matching binaries.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EnvConfig {
    pub node:   Option<String>,
    pub python: Option<String>,
    pub go:     Option<String>,
    pub rust:   Option<String>,
}

// ─── Job ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    #[serde(rename = "runs-on")]
    pub runs_on: Option<String>,
    pub steps: Vec<Step>,
    pub env: Option<HashMap<String, String>>,

    /// Per-job toolchain overrides (takes priority over top-level env block)
    pub toolchain: Option<EnvConfig>,

    pub working_directory: Option<String>,
    pub strategy: Option<Strategy>,
    pub backend: Option<String>,
    pub arch: Option<String>,

    /// Enable/disable step-level caching for this entire job (default: true)
    pub cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Strategy {
    pub matrix: HashMap<String, Vec<String>>,
}

// ─── Step ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Step {
    pub name: Option<String>,

    /// Shell command to execute
    pub run: String,

    pub env: Option<HashMap<String, String>>,
    pub working_directory: Option<String>,

    /// Allow this step to fail without aborting the job
    #[serde(default)]
    pub allow_failure: bool,

    /// Override job-level cache toggle for this specific step
    pub cache: Option<bool>,

    /// Glob patterns for files whose content changes should invalidate the cache.
    /// Example:
    ///   watch:
    ///     - src/**/*.rs
    ///     - Cargo.toml
    #[serde(default)]
    pub watch: Vec<String>,

    /// Paths to files/directories produced by this step.
    /// Zenith archives these on a cache miss and restores them on a hit.
    /// Example:
    ///   outputs:
    ///     - target/release/myapp
    ///     - dist/
    #[serde(default)]
    pub outputs: Vec<String>,

    /// Optional manual cache key override (for cross-OS artifact sharing).
    /// When set, Zenith ignores OS/arch in the hash so two matrix nodes can
    /// share the same cached artifact if their outputs are identical.
    pub cache_key: Option<String>,

    /// Step names that must complete before this step starts (Phase 13).
    /// Steps with no unfulfilled dependencies run concurrently.
    /// Example:
    ///   depends_on:
    ///     - Build
    ///     - Install deps
    #[serde(default)]
    pub depends_on: Vec<String>,
}

// ─── Config loader ────────────────────────────────────────────────────────────

/// Load and parse .zenith.yml from the given path.
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<ZenithConfig> {
    if !path.as_ref().exists() {
        return Err(anyhow::anyhow!(
            "Configuration file '{}' not found.\n\
             Create a .zenith.yml file in the current directory.\n\
             Example:\n\
             \n\
             jobs:\n\
               build:\n\
                 runs-on: alpine\n\
                 steps:\n\
                   - name: Build\n\
                     run: make build",
            path.as_ref().display()
        ));
    }

    let content = fs::read_to_string(&path)
        .context("Failed to read configuration file")?;

    let config: ZenithConfig = serde_yaml::from_str(&content)
        .context("Failed to parse .zenith.yml — check your YAML syntax")?;

    Ok(config)
}
