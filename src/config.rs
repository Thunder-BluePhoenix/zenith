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

    /// Top-level cache settings (Phase 14: Config Schema v2)
    /// Example:
    ///   cache:
    ///     ttl_days: 14
    ///     remote: "https://cache.zenith.run"
    ///     push: true
    pub cache: Option<CacheConfig>,

    /// Named jobs (GitHub Actions style)
    pub jobs: Option<HashMap<String, Job>>,

    /// Simple single-job flat steps (legacy / quick-run format)
    pub steps: Option<Vec<Step>>,
}

fn default_version() -> String { "1".into() }

// ─── Phase 14: Top-level cache config ────────────────────────────────────────

/// Top-level cache configuration block in .zenith.yml
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CacheConfig {
    /// Maximum age in days before step-cache entries are pruned (default: 7)
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u64,

    /// URL of the remote binary cache (optional)
    pub remote: Option<String>,

    /// Automatically push build outputs to the remote cache (default: false)
    #[serde(default)]
    pub push: bool,
}

fn default_ttl_days() -> u64 { 7 }

// ─── Migration helpers (Phase 14: zenith migrate) ────────────────────────────

/// Upgrade a v1 .zenith.yml string to a v2 string with all schema features
/// made explicit. The upgraded YAML is returned as a String.
pub fn migrate_v1_to_v2(v1_yaml: &str) -> Result<String> {
    let mut cfg: ZenithConfig = serde_yaml::from_str(v1_yaml)
        .context("Failed to parse .zenith.yml — check your YAML syntax")?;

    // Bump version
    cfg.version = "2".to_string();

    // Ensure cache block is present
    if cfg.cache.is_none() {
        cfg.cache = Some(CacheConfig::default());
    }

    // Ensure every step has the new v2 fields defaulted
    if let Some(ref mut jobs) = cfg.jobs {
        for job in jobs.values_mut() {
            for step in job.steps.iter_mut() {
                // watch / outputs / depends_on already have #[serde(default)] on the struct;
                // they deserialize to empty vecs automatically, so nothing extra needed.
                let _ = step;
            }
        }
    }

    let out = serde_yaml::to_string(&cfg)
        .context("Failed to serialise upgraded config")?;

    Ok(format!(
        "# Zenith config — schema v2 (auto-migrated by `zenith migrate`)\n# All Phase 6–13 features are now explicit.\n\n{}",
        out
    ))
}

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
