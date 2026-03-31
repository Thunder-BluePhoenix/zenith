/// Plugin registry — discovers and manages installed plugins.
///
/// Plugin install directory: ~/.zenith/plugins/<plugin-name>/
///   Each directory must contain a valid plugin.toml manifest.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{debug, warn};
use super::manifest::PluginManifest;

/// Root directory for all installed plugins.
pub fn plugins_dir() -> PathBuf {
    crate::sandbox::zenith_home().join("plugins")
}

/// Walk ~/.zenith/plugins/ and return all valid plugin manifests.
pub fn discover_plugins() -> Vec<PluginManifest> {
    let dir = plugins_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() { continue; }
        match PluginManifest::load(&entry.path()) {
            Ok(m)  => { debug!("Discovered plugin: {}", m.name); plugins.push(m); }
            Err(e) => warn!("Skipping {:?}: {}", entry.path(), e),
        }
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

/// Find a specific plugin by name.
pub fn find_plugin(name: &str) -> Option<PluginManifest> {
    let plugin_dir = plugins_dir().join(name);
    if !plugin_dir.is_dir() { return None; }
    PluginManifest::load(&plugin_dir).ok()
}

/// Install a plugin from a local directory path.
/// Validates the manifest and entrypoint binary before completing.
pub fn install_from_path(src: &std::path::Path) -> Result<PluginManifest> {
    // Load + validate the manifest first
    let manifest = PluginManifest::load(src)
        .context("Failed to read plugin manifest from source directory")?;

    // Phase 14: requires_zenith version check (simple prefix comparison)
    if let Some(ref req) = manifest.requires_zenith {
        let zenith_ver = env!("CARGO_PKG_VERSION");
        if !version_satisfies(zenith_ver, req) {
            return Err(anyhow::anyhow!(
                "Plugin '{}' requires Zenith {} but this is v{}.\n\
                 Upgrade Zenith or install a compatible plugin version.",
                manifest.name, req, zenith_ver
            ));
        }
    }

    let dest = plugins_dir().join(&manifest.name);
    if dest.exists() {
        return Err(anyhow::anyhow!(
            "Plugin '{}' is already installed at {:?}. Run `zenith plugin remove {}` first.",
            manifest.name, dest, manifest.name
        ));
    }

    // Copy entire directory
    copy_dir(src, &dest)
        .with_context(|| format!("Failed to copy plugin to {:?}", dest))?;

    // Reload from installed location so install_dir is correct
    let installed = PluginManifest::load(&dest)
        .context("Plugin installed but manifest unreadable — install may be corrupt")?;

    // Check entrypoint exists
    let ep = installed.entrypoint_path();
    if !ep.exists() {
        std::fs::remove_dir_all(&dest).ok();
        return Err(anyhow::anyhow!(
            "Entrypoint binary {:?} not found in plugin directory. Aborting install.",
            ep
        ));
    }

    Ok(installed)
}

/// Remove an installed plugin by name.
pub fn remove_plugin(name: &str) -> Result<()> {
    let plugin_dir = plugins_dir().join(name);
    if !plugin_dir.exists() {
        return Err(anyhow::anyhow!("Plugin '{}' is not installed.", name));
    }
    std::fs::remove_dir_all(&plugin_dir)
        .with_context(|| format!("Failed to remove plugin directory {:?}", plugin_dir))
}

// ─── Phase 14: Plugin registry search ────────────────────────────────────────

/// Registry entry returned from the hosted plugin index.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct RegistryEntry {
    name:        String,
    version:     String,
    description: Option<String>,
    author:      Option<String>,
    requires_zenith: Option<String>,
}

/// Search the hosted Zenith plugin registry and print matching results.
/// Falls back to local installed plugins if the registry is unreachable.
pub async fn search_registry(query: &str) -> anyhow::Result<()> {
    // Attempt to fetch the registry index.
    let registry_url = "https://zenith.run/registry/plugins.json";
    let q = query.to_lowercase();

    match reqwest::get(registry_url).await {
        Ok(resp) if resp.status().is_success() => {
            let entries: Vec<RegistryEntry> = resp.json().await
                .unwrap_or_default();

            let hits: Vec<&RegistryEntry> = entries.iter()
                .filter(|e| {
                    e.name.to_lowercase().contains(&q)
                    || e.description.as_deref().unwrap_or("").to_lowercase().contains(&q)
                })
                .collect();

            if hits.is_empty() {
                println!("No registry plugins match '{}'.", query);
                return Ok(());
            }

            println!("{:<24}  {:<10}  {:<10}  {}",
                "Name", "Version", "Requires", "Description");
            println!("{}", "-".repeat(72));
            for e in hits {
                println!("{:<24}  {:<10}  {:<10}  {}",
                    e.name,
                    e.version,
                    e.requires_zenith.as_deref().unwrap_or("-"),
                    e.description.as_deref().unwrap_or("-"));
            }
            println!("\nInstall with: zenith plugin install <name>");
        }
        _ => {
            // Offline fallback: search locally installed plugins
            eprintln!("Registry unreachable — searching locally installed plugins...");
            let installed = discover_plugins();
            let hits: Vec<&PluginManifest> = installed.iter()
                .filter(|p| {
                    p.name.to_lowercase().contains(&q)
                    || p.description.as_deref().unwrap_or("").to_lowercase().contains(&q)
                })
                .collect();

            if hits.is_empty() {
                println!("No local plugins match '{}'.", query);
                return Ok(());
            }
            println!("{:<24}  {:<10}  {}", "Name", "Version", "Description");
            println!("{}", "-".repeat(60));
            for p in hits {
                println!("{:<24}  {:<10}  {}",
                    p.name, p.version,
                    p.description.as_deref().unwrap_or("-"));
            }
        }
    }
    Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let dest_path = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

/// Minimal semver constraint check for `requires_zenith`.
/// Supports operators: >=, >, <=, <, = (or bare version = equality).
/// Only compares major.minor.patch numeric components.
fn version_satisfies(actual: &str, req: &str) -> bool {
    let (op, req_ver) = if let Some(r) = req.strip_prefix(">=") {
        (">=", r.trim())
    } else if let Some(r) = req.strip_prefix('>') {
        (">", r.trim())
    } else if let Some(r) = req.strip_prefix("<=") {
        ("<=", r.trim())
    } else if let Some(r) = req.strip_prefix('<') {
        ("<", r.trim())
    } else if let Some(r) = req.strip_prefix('=') {
        ("=", r.trim())
    } else {
        ("=", req.trim())
    };

    fn parse(v: &str) -> (u64, u64, u64) {
        let parts: Vec<u64> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (parts.first().copied().unwrap_or(0),
         parts.get(1).copied().unwrap_or(0),
         parts.get(2).copied().unwrap_or(0))
    }

    let a = parse(actual);
    let r = parse(req_ver);

    match op {
        ">=" => a >= r,
        ">"  => a > r,
        "<=" => a <= r,
        "<"  => a < r,
        _    => a == r,
    }
}
