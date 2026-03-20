/// Node.js toolchain — Zenith downloads official Node.js release tarballs.
/// Cache: ~/.zenith/toolchains/node/<version>/bin/node

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;
use super::toolchain_dir;

pub struct NodeToolchain { version: String }

impl NodeToolchain {
    pub fn new(version: &str) -> Self {
        // Strip a leading 'v' if the user wrote "v20" instead of "20"
        Self { version: version.trim_start_matches('v').to_string() }
    }
}

impl NodeToolchain {
    pub async fn ensure_installed(&self) -> Result<PathBuf> {
        let install_dir = toolchain_dir("node", &self.version);
        // Windows: node.exe is in the root; Unix: bin/node
        let (bin_dir, node_bin) = if cfg!(target_os = "windows") {
            let d = install_dir.clone();
            let b = d.join("node.exe");
            (d, b)
        } else {
            let d = install_dir.join("bin");
            let b = d.join("node");
            (d, b)
        };

        if node_bin.exists() {
            return Ok(bin_dir);
        }

        info!("Zenith is installing Node.js {} ...", self.version);

        let url = node_download_url(&self.version);
        std::fs::create_dir_all(&install_dir)?;

        // Download + extract
        download_and_extract_node(&url, &install_dir, &self.version).await?;

        if !node_bin.exists() {
            return Err(anyhow::anyhow!(
                "Node.js {} installation failed — binary not found at {:?}",
                self.version, node_bin
            ));
        }

        info!("Node.js {} ready at {:?}", self.version, bin_dir);
        Ok(bin_dir)
    }
}

fn node_download_url(version: &str) -> String {
    let platform = if cfg!(target_os = "linux") { "linux" }
        else if cfg!(target_os = "macos") { "darwin" }
        else { "win" };

    let arch = if cfg!(target_arch = "aarch64") { "arm64" }
        else if cfg!(target_os = "windows") { "x64" }
        else { "x64" };

    if cfg!(target_os = "windows") {
        format!("https://nodejs.org/dist/v{version}/node-v{version}-win-{arch}.zip")
    } else {
        format!("https://nodejs.org/dist/v{version}/node-v{version}-{platform}-{arch}.tar.gz")
    }
}

async fn download_and_extract_node(url: &str, install_dir: &std::path::Path, version: &str) -> Result<()> {
    let bytes = crate::tools::fetch_url(url).await?;

    if url.ends_with(".zip") {
        extract_zip_strip(&bytes, install_dir)
            .context("Failed to extract Node.js zip")?;
    } else {
        extract_targz_strip(&bytes, install_dir)
            .context("Failed to extract Node.js tar.gz")?;
    }
    Ok(())
}

/// Extract a tar.gz, stripping the top-level directory (node-vX.Y.Z-linux-x64/)
fn extract_targz_strip(bytes: &[u8], dest: &std::path::Path) -> Result<()> {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        // Strip first component (e.g. "node-v20.0.0-linux-x64/")
        let stripped: PathBuf = path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() { continue; }
        let out_path = dest.join(&stripped);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&out_path)?;
    }
    Ok(())
}

/// Extract a .zip, stripping the top-level directory
fn extract_zip_strip(bytes: &[u8], dest: &std::path::Path) -> Result<()> {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let raw = file.name().to_string();
        // strip first component
        let stripped: PathBuf = std::path::Path::new(&raw).components().skip(1).collect();
        if stripped.as_os_str().is_empty() { continue; }
        let out_path = dest.join(&stripped);
        if file.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(p) = out_path.parent() { std::fs::create_dir_all(p)?; }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }
    Ok(())
}
