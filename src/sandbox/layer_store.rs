/// Content-addressable rootfs layer store (Phase 12).
///
/// Directory layout:
///   ~/.zenith/layers/
///     <sha256-hex>/
///       layer.tar.gz    — compressed rootfs layer archive
///       meta.json       — { "os": "alpine", "source_url": "...", "created_at": <secs> }
///
/// Multiple Firecracker VMs (or container jobs) running the same OS can share
/// the same read-only base layer — only the per-run overlay differs.
/// This mirrors Docker image layers but without Docker.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LayerMeta {
    pub os:         String,
    pub source_url: String,
    pub created_at: u64,
    pub size_bytes: u64,
}

// ─── LayerStore ───────────────────────────────────────────────────────────────

pub struct LayerStore {
    layers_dir: PathBuf,
}

impl LayerStore {
    pub fn new() -> Result<Self> {
        let layers_dir = crate::sandbox::zenith_home().join("layers");
        std::fs::create_dir_all(&layers_dir)
            .context("Cannot create layers directory")?;
        Ok(Self { layers_dir })
    }

    /// Content-address a layer by hashing the source URL + OS name.
    /// Two identical OS sources always map to the same layer hash.
    pub fn layer_hash(os: &str, source_url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(os.as_bytes());
        hasher.update(b"|");
        hasher.update(source_url.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Path to the directory for a layer hash.
    pub fn layer_dir(&self, hash: &str) -> PathBuf {
        self.layers_dir.join(hash)
    }

    /// Path to the compressed layer archive.
    pub fn layer_archive(&self, hash: &str) -> PathBuf {
        self.layer_dir(hash).join("layer.tar.gz")
    }

    /// Check whether a layer is already stored.
    pub fn has_layer(&self, hash: &str) -> bool {
        self.layer_archive(hash).exists()
    }

    /// Read the metadata for a stored layer.
    pub fn get_meta(&self, hash: &str) -> Option<LayerMeta> {
        let path = self.layer_dir(hash).join("meta.json");
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Store a downloaded rootfs archive as a layer.
    ///
    /// `data` is the raw bytes of the tar.gz archive.
    /// Returns the content-address hash.
    pub fn store_layer(&self, os: &str, source_url: &str, data: &[u8]) -> Result<String> {
        let hash = Self::layer_hash(os, source_url);
        let dir  = self.layer_dir(&hash);

        if self.has_layer(&hash) {
            tracing::debug!("[layer-store] Layer {} already present, skipping", &hash[..16]);
            return Ok(hash);
        }

        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Cannot create layer dir for {}", &hash[..16]))?;

        std::fs::write(dir.join("layer.tar.gz"), data)
            .context("Failed to write layer archive")?;

        let meta = LayerMeta {
            os:         os.to_string(),
            source_url: source_url.to_string(),
            created_at: now_secs(),
            size_bytes: data.len() as u64,
        };
        let json = serde_json::to_string_pretty(&meta)
            .context("Failed to serialise layer meta")?;
        std::fs::write(dir.join("meta.json"), json)
            .context("Failed to write layer meta")?;

        tracing::info!(
            "[layer-store] Stored layer {} ({} for {})",
            &hash[..16], fmt_bytes(data.len()), os
        );
        Ok(hash)
    }

    /// Extract a stored layer into `dest_dir`.
    ///
    /// Used to set up the read-only base layer for a VM or container run.
    /// Multiple concurrent VMs call this with the same `hash`; since `dest_dir`
    /// is unique per VM, there is no write contention on the shared archive.
    pub fn extract_layer(&self, hash: &str, dest_dir: &Path) -> Result<()> {
        let archive = self.layer_archive(hash);
        if !archive.exists() {
            return Err(anyhow::anyhow!(
                "Layer {} not in store — call store_layer() first", &hash[..16]
            ));
        }

        std::fs::create_dir_all(dest_dir)
            .with_context(|| format!("Cannot create extraction dir {:?}", dest_dir))?;

        let file    = std::fs::File::open(&archive)
            .with_context(|| format!("Cannot open layer archive {:?}", archive))?;
        let gz      = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gz);
        tar.unpack(dest_dir)
            .with_context(|| format!("Failed to extract layer {} to {:?}", &hash[..16], dest_dir))?;

        tracing::debug!("[layer-store] Extracted {} to {:?}", &hash[..16], dest_dir);
        Ok(())
    }

    /// List all layers in the store with their metadata.
    pub fn list_layers(&self) -> Vec<(String, LayerMeta)> {
        let Ok(entries) = std::fs::read_dir(&self.layers_dir) else {
            return vec![];
        };
        entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let hash = e.file_name().to_string_lossy().to_string();
                let meta = self.get_meta(&hash)?;
                Some((hash, meta))
            })
            .collect()
    }

    /// Remove layers that have not been used for more than `max_age_secs`.
    pub fn prune(&self, max_age_secs: u64) -> Result<usize> {
        let cutoff = now_secs().saturating_sub(max_age_secs);
        let mut removed = 0;
        for (hash, meta) in self.list_layers() {
            if meta.created_at < cutoff {
                let dir = self.layer_dir(&hash);
                std::fs::remove_dir_all(&dir)
                    .with_context(|| format!("Failed to remove layer {}", &hash[..16]))?;
                tracing::info!("[layer-store] Pruned old layer {} ({})", &hash[..16], meta.os);
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Total bytes consumed by all stored layers.
    pub fn total_size_bytes(&self) -> u64 {
        self.list_layers().iter().map(|(_, m)| m.size_bytes).sum()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fmt_bytes(n: usize) -> String {
    if n < 1024 { format!("{}B", n) }
    else if n < 1024 * 1024 { format!("{:.1}KB", n as f64 / 1024.0) }
    else { format!("{:.1}MB", n as f64 / (1024.0 * 1024.0)) }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store_in(dir: &Path) -> LayerStore {
        // Override via a direct path — bypasses zenith_home()
        LayerStore { layers_dir: dir.join("layers") }
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = LayerStore::layer_hash("alpine", "https://example.com/alpine.tar.gz");
        let h2 = LayerStore::layer_hash("alpine", "https://example.com/alpine.tar.gz");
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_os_different_hash() {
        let h1 = LayerStore::layer_hash("alpine", "https://example.com/rootfs.tar.gz");
        let h2 = LayerStore::layer_hash("ubuntu", "https://example.com/rootfs.tar.gz");
        assert_ne!(h1, h2);
    }

    #[test]
    fn store_and_retrieve_layer() {
        let tmp = TempDir::new().unwrap();
        let store = store_in(tmp.path());
        std::fs::create_dir_all(&store.layers_dir).unwrap();

        let data = b"fake-tar-gz-data";
        let hash = store.store_layer("alpine", "https://example.com/alpine.tar.gz", data).unwrap();

        assert!(store.has_layer(&hash));

        let meta = store.get_meta(&hash).unwrap();
        assert_eq!(meta.os, "alpine");
        assert_eq!(meta.size_bytes, data.len() as u64);
    }

    #[test]
    fn storing_same_layer_twice_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let store = store_in(tmp.path());
        std::fs::create_dir_all(&store.layers_dir).unwrap();

        let data = b"data";
        let h1 = store.store_layer("alpine", "https://cdn/x", data).unwrap();
        let h2 = store.store_layer("alpine", "https://cdn/x", data).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn total_size_sums_layers() {
        let tmp = TempDir::new().unwrap();
        let store = store_in(tmp.path());
        std::fs::create_dir_all(&store.layers_dir).unwrap();

        store.store_layer("alpine", "https://cdn/a", b"aaa").unwrap();
        store.store_layer("debian", "https://cdn/b", b"bbbbb").unwrap();

        assert_eq!(store.total_size_bytes(), 8); // 3 + 5
    }

    #[test]
    fn list_layers_returns_all() {
        let tmp = TempDir::new().unwrap();
        let store = store_in(tmp.path());
        std::fs::create_dir_all(&store.layers_dir).unwrap();

        store.store_layer("alpine", "https://cdn/a", b"x").unwrap();
        store.store_layer("ubuntu", "https://cdn/b", b"y").unwrap();

        assert_eq!(store.list_layers().len(), 2);
    }
}
