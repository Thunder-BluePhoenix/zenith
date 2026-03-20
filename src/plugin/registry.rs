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
