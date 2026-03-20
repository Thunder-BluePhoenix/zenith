/// Plugin manifest — parsed from ~/.zenith/plugins/<name>/plugin.toml
///
/// Example plugin.toml:
///
///   [plugin]
///   name        = "bhyve-backend"
///   version     = "0.1.0"
///   type        = "backend"
///   entrypoint  = "zenith-backend-bhyve"
///   description = "FreeBSD bhyve VM backend for Zenith"

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Backend,
    Toolchain,
    Syntax,
    Logger,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Backend   => write!(f, "backend"),
            PluginType::Toolchain => write!(f, "toolchain"),
            PluginType::Syntax    => write!(f, "syntax"),
            PluginType::Logger    => write!(f, "logger"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name:        String,
    pub version:     String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    pub entrypoint:  String,
    pub description: Option<String>,

    /// Resolved absolute path to the plugin directory (set after loading, not in TOML)
    #[serde(skip)]
    pub install_dir: PathBuf,
}

impl PluginManifest {
    /// Load a manifest from a plugin directory.
    pub fn load(plugin_dir: &Path) -> Result<Self> {
        let toml_path = plugin_dir.join("plugin.toml");
        let raw = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Cannot read {:?}", toml_path))?;

        // TOML files wrap the fields in a [plugin] table
        let wrapper: TomlWrapper = toml::from_str(&raw)
            .with_context(|| format!("Invalid plugin.toml at {:?}", toml_path))?;

        let mut manifest = wrapper.plugin;
        manifest.install_dir = plugin_dir.to_path_buf();
        Ok(manifest)
    }

    /// Absolute path to the entrypoint binary.
    pub fn entrypoint_path(&self) -> PathBuf {
        let bin = if cfg!(target_os = "windows") && !self.entrypoint.ends_with(".exe") {
            format!("{}.exe", self.entrypoint)
        } else {
            self.entrypoint.clone()
        };
        self.install_dir.join(bin)
    }

    /// Write this manifest as plugin.toml inside the given directory.
    pub fn write(&self, plugin_dir: &Path) -> Result<()> {
        let wrapper = TomlWrapper { plugin: self.clone() };
        let content = toml::to_string_pretty(&wrapper)
            .context("Failed to serialize plugin.toml")?;
        std::fs::write(plugin_dir.join("plugin.toml"), content)
            .context("Failed to write plugin.toml")
    }
}

// TOML wraps fields in a [plugin] table
#[derive(Serialize, Deserialize)]
struct TomlWrapper {
    plugin: PluginManifest,
}
