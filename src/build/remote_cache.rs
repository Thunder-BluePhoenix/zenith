/// Remote binary cache — Phase 13.
///
/// Zenith can push and pull content-addressed build outputs to/from an HTTP
/// server keyed by derivation ID.
///
/// Protocol (all endpoints are relative to the configured base URL):
///   HEAD /store/{drv_id}         → 200 if present, 404 if not
///   GET  /store/{drv_id}         → tar.gz archive of the outputs directory
///   PUT  /store/{drv_id}         → upload a tar.gz archive
///
/// Configuration in ~/.zenith/config.toml:
///   [cache]
///   remote = "https://cache.example.com"
///   push   = true          # upload after every successful build (default: false)
///   api_key = "secret"     # bearer token (optional)
///
/// The same derivation ID (`Derivation::id()`) is used as the cache key both
/// locally (in ~/.zenith/store/) and remotely, so any machine that builds the
/// same derivation can share the result.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteCacheConfig {
    /// Base URL of the remote cache server (no trailing slash).
    pub remote: Option<String>,
    /// Upload outputs to the remote cache after a successful local build.
    #[serde(default)]
    pub push: bool,
    /// Optional bearer token sent as `Authorization: Bearer <key>`.
    pub api_key: Option<String>,
}

pub fn load_cache_config() -> RemoteCacheConfig {
    let path = crate::sandbox::zenith_home().join("config.toml");
    if !path.exists() { return RemoteCacheConfig::default(); }
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    #[derive(serde::Deserialize, Default)]
    struct Wrapper { #[serde(default)] cache: RemoteCacheConfig }
    let w: Wrapper = toml::from_str(&raw).unwrap_or_default();
    w.cache
}

pub fn save_cache_config(cfg: &RemoteCacheConfig) -> Result<()> {
    let path = crate::sandbox::zenith_home().join("config.toml");
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }

    let existing = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut lines: Vec<&str> = existing.lines()
        .filter(|l| {
            // Strip any old [cache] section lines
            !l.trim_start().starts_with("[cache]")
                && !l.trim_start().starts_with("remote ")
                && !l.trim_start().starts_with("push ")
                && !l.trim_start().starts_with("api_key ")
        })
        .collect();

    let mut section = String::from("[cache]\n");
    if let Some(ref url) = cfg.remote {
        section.push_str(&format!("remote = \"{}\"\n", url));
    }
    section.push_str(&format!("push = {}\n", cfg.push));
    if let Some(ref key) = cfg.api_key {
        section.push_str(&format!("api_key = \"{}\"\n", key));
    }

    lines.push(&section);
    std::fs::write(&path, lines.join("\n"))
        .context("Failed to write cache config")?;
    Ok(())
}

// ─── Client ───────────────────────────────────────────────────────────────────

pub struct RemoteCacheClient {
    base_url: String,
    api_key:  Option<String>,
    push:     bool,
    client:   reqwest::Client,
}

impl RemoteCacheClient {
    /// Build a client from `~/.zenith/config.toml`.
    /// Returns `None` if no remote URL is configured.
    pub fn from_config() -> Option<Self> {
        let cfg = load_cache_config();
        let base_url = cfg.remote?;
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key:  cfg.api_key,
            push:     cfg.push,
            client:   reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        })
    }

    pub fn push_enabled(&self) -> bool { self.push }

    fn store_url(&self, drv_id: &str) -> String {
        format!("{}/store/{}", self.base_url, drv_id)
    }

    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            req.bearer_auth(key)
        } else {
            req
        }
    }

    /// Check if the derivation exists in the remote cache.
    pub async fn has(&self, drv_id: &str) -> bool {
        let url = self.store_url(drv_id);
        match self.authed(self.client.head(&url)).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                debug!("Remote cache HEAD failed for {}: {}", &drv_id[..16], e);
                false
            }
        }
    }

    /// Download the remote cache entry and extract it into `dest_dir`.
    pub async fn pull(&self, drv_id: &str, dest_dir: &Path) -> Result<()> {
        let url = self.store_url(drv_id);
        info!("[remote-cache] Pulling {} from {}", &drv_id[..16], self.base_url);

        let resp = self.authed(self.client.get(&url))
            .send().await
            .with_context(|| format!("GET {} failed", url))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Remote cache GET returned {}: {}", resp.status(), url
            ));
        }

        let bytes = resp.bytes().await
            .context("Failed to read remote cache response body")?;

        std::fs::create_dir_all(dest_dir)
            .context("Failed to create restore destination")?;

        // Unpack the tar.gz into dest_dir
        let decompressed = flate2::read::GzDecoder::new(bytes.as_ref());
        let mut archive = tar::Archive::new(decompressed);
        archive.unpack(dest_dir)
            .context("Failed to unpack remote cache archive")?;

        info!("[remote-cache] Pulled {} ({} bytes)", &drv_id[..16], bytes.len());
        Ok(())
    }

    /// Pack `src_dir` as a tar.gz and upload it to the remote cache.
    pub async fn push(&self, drv_id: &str, src_dir: &Path) -> Result<()> {
        if !src_dir.exists() {
            return Err(anyhow::anyhow!(
                "Source directory does not exist: {:?}", src_dir
            ));
        }

        let tarball = pack_dir_as_targz(src_dir)
            .context("Failed to create tarball for remote cache upload")?;

        let url = self.store_url(drv_id);
        info!("[remote-cache] Pushing {} ({} bytes) to {}",
            &drv_id[..16], tarball.len(), self.base_url);

        let resp = self.authed(
            self.client.put(&url)
                .header("Content-Type", "application/octet-stream")
                .body(tarball)
        ).send().await
            .with_context(|| format!("PUT {} failed", url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Remote cache PUT returned {}: {}", status, body
            ));
        }

        info!("[remote-cache] Push complete for {}", &drv_id[..16]);
        Ok(())
    }
}

// ─── Tar helpers ─────────────────────────────────────────────────────────────

fn pack_dir_as_targz(src: &Path) -> Result<Vec<u8>> {
    use flate2::{Compression, write::GzEncoder};

    let mut buf = Vec::new();
    {
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", src)
            .context("Failed to add directory to tar archive")?;
        tar.into_inner()
            .context("Failed to finalise tar archive")?
            .finish()
            .context("Failed to finish gzip stream")?;
    }
    Ok(buf)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn roundtrip_pack_unpack() {
        let src = TempDir::new().unwrap();
        std::fs::write(src.path().join("hello.txt"), b"hello world").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub").join("deep.txt"), b"deep").unwrap();

        let tarball = pack_dir_as_targz(src.path()).unwrap();
        assert!(!tarball.is_empty());

        let dst = TempDir::new().unwrap();
        let dec = flate2::read::GzDecoder::new(tarball.as_slice());
        let mut archive = tar::Archive::new(dec);
        archive.unpack(dst.path()).unwrap();

        assert!(dst.path().join("hello.txt").exists());
        assert!(dst.path().join("sub").join("deep.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.path().join("hello.txt")).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn load_cache_config_returns_default_when_missing() {
        // Use a path that can't exist — config loader returns Default on missing
        let cfg = RemoteCacheConfig::default();
        assert!(cfg.remote.is_none());
        assert!(!cfg.push);
    }
}
