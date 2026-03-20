/// Remote config — persists registered remotes in ~/.zenith/remotes.toml
///
/// Example remotes.toml:
///
///   [remotes.build-server]
///   host    = "user@192.168.1.10"
///   port    = 22
///   key     = "~/.ssh/id_ed25519"   # optional key override
///
///   [remotes.pi]
///   host = "pi@raspberrypi.local"

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub key:  Option<String>,
}

fn default_port() -> u16 { 22 }

#[derive(Debug, Default, Serialize, Deserialize)]
struct RemotesFile {
    #[serde(default)]
    remotes: HashMap<String, RemoteEntry>,
}

fn remotes_path() -> PathBuf {
    crate::sandbox::zenith_home().join("remotes.toml")
}

fn load_file() -> Result<RemotesFile> {
    let path = remotes_path();
    if !path.exists() {
        return Ok(RemotesFile::default());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read {:?}", path))?;
    toml::from_str(&raw)
        .with_context(|| format!("Invalid remotes.toml at {:?}", path))
}

fn save_file(f: &RemotesFile) -> Result<()> {
    let path = remotes_path();
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    let content = toml::to_string_pretty(f).context("Failed to serialize remotes.toml")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Cannot write {:?}", path))
}

pub fn list_remotes() -> Result<Vec<(String, RemoteEntry)>> {
    let f = load_file()?;
    let mut list: Vec<_> = f.remotes.into_iter().collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list)
}

pub fn get_remote(name: &str) -> Result<RemoteEntry> {
    let f = load_file()?;
    f.remotes.get(name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!(
            "Remote '{}' not found. Use `zenith remote add {} <user@host>` to register it.",
            name, name
        ))
}

pub fn add_remote(name: &str, host: &str, port: u16, key: Option<String>) -> Result<()> {
    let mut f = load_file()?;
    if f.remotes.contains_key(name) {
        return Err(anyhow::anyhow!(
            "Remote '{}' already exists. Remove it first with `zenith remote remove {}`.",
            name, name
        ));
    }
    f.remotes.insert(name.to_string(), RemoteEntry { host: host.to_string(), port, key });
    save_file(&f)
}

pub fn remove_remote(name: &str) -> Result<()> {
    let mut f = load_file()?;
    if f.remotes.remove(name).is_none() {
        return Err(anyhow::anyhow!("Remote '{}' not found.", name));
    }
    save_file(&f)
}
