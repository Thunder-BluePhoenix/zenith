/// Content-addressable build store — Phase 13.
///
/// Every successfully built derivation stores its outputs here:
///   ~/.zenith/store/<derivation-id>/
///     outputs/          — output files/dirs extracted from the build
///     derivation.json   — the full derivation that produced these outputs
///     meta.json         — { "built_at": <secs>, "host": "<uname>" }
///
/// Two projects that produce the same derivation hash share a single store
/// entry — no duplication. This is the foundation of the remote binary cache:
/// the derivation ID is the cache key both locally and on the CDN.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::derivation::Derivation;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreMeta {
    pub built_at_secs: u64,
    pub host:          String,
    pub drv_id:        String,
}

// ─── BuildStore ───────────────────────────────────────────────────────────────

pub struct BuildStore {
    store_dir: PathBuf,
}

impl BuildStore {
    pub fn new() -> Result<Self> {
        let store_dir = crate::sandbox::zenith_home().join("store");
        std::fs::create_dir_all(&store_dir)
            .context("Cannot create build store directory")?;
        Ok(Self { store_dir })
    }

    /// Path to the store entry for a derivation ID.
    pub fn entry_dir(&self, drv_id: &str) -> PathBuf {
        self.store_dir.join(drv_id)
    }

    /// Path to the outputs directory inside a store entry.
    pub fn outputs_dir(&self, drv_id: &str) -> PathBuf {
        self.entry_dir(drv_id).join("outputs")
    }

    /// Return true if this derivation has a stored result.
    pub fn has(&self, drv_id: &str) -> bool {
        self.entry_dir(drv_id).join("meta.json").exists()
    }

    /// Read the metadata for a store entry.
    pub fn meta(&self, drv_id: &str) -> Option<StoreMeta> {
        let path = self.entry_dir(drv_id).join("meta.json");
        let raw  = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Read the derivation that produced a store entry.
    pub fn derivation(&self, drv_id: &str) -> Option<Derivation> {
        let path = self.entry_dir(drv_id).join("derivation.json");
        let raw  = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Register a completed build in the store.
    ///
    /// `outputs_src` is the directory containing the build outputs.
    /// Its contents are moved into `~/.zenith/store/<id>/outputs/`.
    pub fn commit(&self, drv: &Derivation, outputs_src: &Path) -> Result<()> {
        let id      = drv.id();
        let dir     = self.entry_dir(&id);
        let out_dir = self.outputs_dir(&id);

        if self.has(&id) {
            tracing::debug!("[store] {} already committed, skipping", &id[..16]);
            return Ok(());
        }

        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("Cannot create store outputs dir for {}", &id[..16]))?;

        // Copy outputs into the store
        copy_dir_all(outputs_src, &out_dir)
            .with_context(|| format!("Failed to copy outputs to store for {}", &id[..16]))?;

        // Write derivation.json
        std::fs::write(
            dir.join("derivation.json"),
            drv.to_json_pretty(),
        ).context("Failed to write derivation.json")?;

        // Write meta.json
        let meta = StoreMeta {
            built_at_secs: now_secs(),
            host:          hostname(),
            drv_id:        id.clone(),
        };
        let json = serde_json::to_string_pretty(&meta)
            .context("Failed to serialise store meta")?;
        std::fs::write(dir.join("meta.json"), json)
            .context("Failed to write store meta.json")?;

        tracing::info!("[store] Committed {} ({})", &id[..16], drv.name);
        Ok(())
    }

    /// Restore a store entry's outputs into `dest_dir`.
    pub fn restore(&self, drv_id: &str, dest_dir: &Path) -> Result<()> {
        if !self.has(drv_id) {
            return Err(anyhow::anyhow!(
                "Derivation {} not in store", &drv_id[..16]
            ));
        }

        let src = self.outputs_dir(drv_id);
        copy_dir_all(&src, dest_dir)
            .with_context(|| format!("Failed to restore {} to {:?}", &drv_id[..16], dest_dir))?;

        tracing::debug!("[store] Restored {} to {:?}", &drv_id[..16], dest_dir);
        Ok(())
    }

    /// List all store entries.
    pub fn list(&self) -> Vec<(String, StoreMeta)> {
        let Ok(entries) = std::fs::read_dir(&self.store_dir) else {
            return vec![];
        };
        entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let id   = e.file_name().to_string_lossy().to_string();
                let meta = self.meta(&id)?;
                Some((id, meta))
            })
            .collect()
    }

    /// Total bytes used by all store entries.
    pub fn total_size_bytes(&self) -> u64 {
        dir_size(&self.store_dir)
    }

    /// Remove store entries older than `max_age_secs`.
    pub fn gc(&self, max_age_secs: u64) -> Result<usize> {
        let cutoff = now_secs().saturating_sub(max_age_secs);
        let mut removed = 0;
        for (id, meta) in self.list() {
            if meta.built_at_secs < cutoff {
                let dir = self.entry_dir(&id);
                std::fs::remove_dir_all(&dir)
                    .with_context(|| format!("Failed to remove store entry {}", &id[..16]))?;
                tracing::info!("[store] GC removed {}", &id[..16]);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry   = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0; };
    entries.flatten().map(|e| {
        let p = e.path();
        if p.is_dir() { dir_size(&p) }
        else { std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) }
    }).sum()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::derivation::Derivation;
    use crate::config::Step;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_store(tmp: &TempDir) -> BuildStore {
        BuildStore { store_dir: tmp.path().join("store") }
    }

    fn make_drv(cmd: &str) -> Derivation {
        let step = Step {
            name: Some("test".into()), run: cmd.into(),
            env: None, working_directory: None, allow_failure: false,
            cache: None, cache_key: None, watch: vec![], outputs: vec![], depends_on: vec![],
        };
        Derivation::from_step(&step, &HashMap::new(), "alpine", "x86_64")
    }

    #[test]
    fn commit_then_has() {
        let tmp   = TempDir::new().unwrap();
        let store = make_store(&tmp);
        std::fs::create_dir_all(&store.store_dir).unwrap();

        let drv       = make_drv("cargo build");
        let outputs   = tmp.path().join("out");
        std::fs::create_dir_all(&outputs).unwrap();
        std::fs::write(outputs.join("bin"), b"fake-binary").unwrap();

        store.commit(&drv, &outputs).unwrap();
        assert!(store.has(&drv.id()));
    }

    #[test]
    fn commit_is_idempotent() {
        let tmp   = TempDir::new().unwrap();
        let store = make_store(&tmp);
        std::fs::create_dir_all(&store.store_dir).unwrap();

        let drv     = make_drv("make");
        let outputs = tmp.path().join("out");
        std::fs::create_dir_all(&outputs).unwrap();

        store.commit(&drv, &outputs).unwrap();
        store.commit(&drv, &outputs).unwrap(); // must not error
        assert!(store.has(&drv.id()));
    }

    #[test]
    fn restore_copies_outputs() {
        let tmp   = TempDir::new().unwrap();
        let store = make_store(&tmp);
        std::fs::create_dir_all(&store.store_dir).unwrap();

        let drv     = make_drv("gcc main.c -o main");
        let outputs = tmp.path().join("out");
        std::fs::create_dir_all(&outputs).unwrap();
        std::fs::write(outputs.join("main"), b"ELF").unwrap();

        store.commit(&drv, &outputs).unwrap();

        let dest = tmp.path().join("restored");
        store.restore(&drv.id(), &dest).unwrap();
        assert!(dest.join("main").exists());
    }

    #[test]
    fn list_returns_all_entries() {
        let tmp   = TempDir::new().unwrap();
        let store = make_store(&tmp);
        std::fs::create_dir_all(&store.store_dir).unwrap();

        let out = tmp.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        store.commit(&make_drv("a"), &out).unwrap();
        store.commit(&make_drv("b"), &out).unwrap();

        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn restore_missing_returns_error() {
        let tmp   = TempDir::new().unwrap();
        let store = make_store(&tmp);
        std::fs::create_dir_all(&store.store_dir).unwrap();

        let dest = tmp.path().join("dest");
        let result = store.restore("0000000000000000000000000000000000000000000000000000000000000000", &dest);
        assert!(result.is_err());
    }
}
