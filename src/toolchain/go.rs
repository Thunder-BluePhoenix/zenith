/// Go toolchain — downloads official Go release tarballs from go.dev.
/// Cache: ~/.zenith/toolchains/go/<version>/bin/go

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;
use super::toolchain_dir;

pub struct GoToolchain { version: String }

impl GoToolchain {
    pub fn new(version: &str) -> Self {
        Self { version: version.to_string() }
    }
}

impl GoToolchain {
    pub async fn ensure_installed(&self) -> Result<PathBuf> {
        let install_dir = toolchain_dir("go", &self.version);
        let bin_dir = install_dir.join("bin");
        let go_bin = bin_dir.join(if cfg!(target_os = "windows") { "go.exe" } else { "go" });

        if go_bin.exists() {
            return Ok(bin_dir);
        }

        info!("Zenith is installing Go {} ...", self.version);

        let url = go_download_url(&self.version);
        std::fs::create_dir_all(&install_dir)?;

        let bytes = crate::tools::fetch_url(&url).await?;
        extract_go_archive(&bytes, &install_dir, url.ends_with(".zip"))
            .context("Failed to extract Go archive")?;

        if !go_bin.exists() {
            return Err(anyhow::anyhow!(
                "Go {} installation failed — binary not found at {:?}", self.version, go_bin
            ));
        }

        info!("Go {} ready at {:?}", self.version, bin_dir);
        Ok(bin_dir)
    }
}

fn go_download_url(version: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("https://go.dev/dl/go{version}.windows-amd64.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        format!("https://go.dev/dl/go{version}.linux-arm64.tar.gz")
    } else if cfg!(target_os = "linux") {
        format!("https://go.dev/dl/go{version}.linux-amd64.tar.gz")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        format!("https://go.dev/dl/go{version}.darwin-arm64.tar.gz")
    } else {
        format!("https://go.dev/dl/go{version}.darwin-amd64.tar.gz")
    }
}

fn extract_go_archive(bytes: &[u8], dest: &std::path::Path, is_zip: bool) -> Result<()> {
    use std::io::Cursor;
    if is_zip {
        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            let raw = f.name().to_string();
            // Strip "go/" prefix
            let stripped: std::path::PathBuf = std::path::Path::new(&raw).components().skip(1).collect();
            if stripped.as_os_str().is_empty() { continue; }
            let out = dest.join(&stripped);
            if f.is_dir() { std::fs::create_dir_all(&out)?; }
            else {
                if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
                let mut file = std::fs::File::create(&out)?;
                std::io::copy(&mut f, &mut file)?;
            }
        }
    } else {
        let cursor = Cursor::new(bytes);
        let decoder = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(decoder);
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            // Strip "go/" prefix
            let stripped: std::path::PathBuf = path.components().skip(1).collect();
            if stripped.as_os_str().is_empty() { continue; }
            let out = dest.join(&stripped);
            if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
            entry.unpack(&out)?;
        }
    }
    Ok(())
}
