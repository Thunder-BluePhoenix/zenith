/// Python toolchain — uses python-build-standalone (Gregory Szorc's project).
/// Ships as pre-built, fully self-contained Python binaries for every platform.
/// Cache: ~/.zenith/toolchains/python/<version>/bin/python3

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;
use super::toolchain_dir;

pub struct PythonToolchain { version: String }

impl PythonToolchain {
    pub fn new(version: &str) -> Self {
        Self { version: version.to_string() }
    }
}

impl PythonToolchain {
    pub async fn ensure_installed(&self) -> Result<PathBuf> {
        let install_dir = toolchain_dir("python", &self.version);
        let bin_dir = install_dir.join("bin");
        let python_bin = bin_dir.join("python3");

        if python_bin.exists() {
            return Ok(bin_dir);
        }

        info!("Zenith is installing Python {} ...", self.version);

        let url = python_download_url(&self.version)
            .ok_or_else(|| anyhow::anyhow!(
                "No python-build-standalone release URL for Python {} on this platform.\n\
                 Check https://github.com/indygreg/python-build-standalone/releases for available versions.",
                self.version
            ))?;

        std::fs::create_dir_all(&install_dir)?;
        let bytes = crate::tools::fetch_url(&url).await?;
        extract_python_standalone(&bytes, &install_dir)
            .context("Failed to extract Python standalone archive")?;

        if !python_bin.exists() {
            return Err(anyhow::anyhow!(
                "Python {} installation failed — binary not found at {:?}",
                self.version, python_bin
            ));
        }

        info!("Python {} ready at {:?}", self.version, bin_dir);
        Ok(bin_dir)
    }
}

fn python_download_url(version: &str) -> Option<String> {
    // python-build-standalone uses a date tag. We use a known-good release date.
    // Format: cpython-{version}+{date}-{triple}-install_only.tar.gz
    let date = "20240107";

    let triple = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc-shared"
    } else {
        return None;
    };

    Some(format!(
        "https://github.com/indygreg/python-build-standalone/releases/download/{date}/\
         cpython-{version}+{date}-{triple}-install_only.tar.gz"
    ))
}

fn extract_python_standalone(bytes: &[u8], dest: &std::path::Path) -> Result<()> {
    use std::io::Cursor;
    // python-build-standalone archives contain a top-level `python/` directory
    let cursor = Cursor::new(bytes);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        // Strip first component ("python/")
        let stripped: std::path::PathBuf = path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() { continue; }
        let out = dest.join(&stripped);
        if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
        entry.unpack(&out)?;
    }
    Ok(())
}
