/// Phase 6: Build & Cache System
///
/// CacheManager provides:
///   1. File-content-aware hashing — changes to watched files bust the cache
///   2. JSON metadata per entry — timestamp, OS, command (enables TTL expiry)
///   3. Artifact archiving — tar.gz outputs into the cache store
///   4. Artifact restoration — extract on cache hit so downstream steps can use them
///   5. TTL-based expiry — stale entries are automatically treated as misses

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use hex;
use serde::{Deserialize, Serialize};
use crate::config::Step;
use tracing::{info, debug};

// ─── Default TTL ──────────────────────────────────────────────────────────────
const DEFAULT_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

// ─── Cache entry metadata ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    pub created_at_secs: u64,
    pub os:              String,
    pub arch:            String,
    pub run:             String,
    pub has_artifacts:   bool,
}

// ─── CacheManager ─────────────────────────────────────────────────────────────

pub struct CacheManager {
    /// ~/.zenith/cache/
    cache_dir: PathBuf,
    /// TTL in seconds before an entry is treated as a miss
    ttl_secs: u64,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let home = crate::sandbox::zenith_home();
        let cache_dir = home.join("cache");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir, ttl_secs: DEFAULT_TTL_SECS })
    }

    // ─── Hashing ─────────────────────────────────────────────────────────────

    /// Build a deterministic SHA-256 hash that captures every input of a step:
    ///   - os + arch
    ///   - environment variables (sorted)
    ///   - step command, name, working directory
    ///   - content of every file matched by `step.watch` globs (Phase 6 key feature)
    ///   - optional `cache_key` override (skips os/arch for cross-job sharing)
    pub fn compute_step_hash(
        &self,
        base_os: &str,
        target_arch: &str,
        step: &Step,
        env: &HashMap<String, String>,
    ) -> String {
        let mut hasher = Sha256::new();

        // 1. OS + arch (or custom cache_key overrides both)
        if let Some(ref key) = step.cache_key {
            hasher.update(key.as_bytes());
        } else {
            hasher.update(base_os.as_bytes());
            hasher.update(target_arch.as_bytes());
        }

        // 2. Environment variables — sorted for determinism
        let mut env_keys: Vec<_> = env.keys().collect();
        env_keys.sort();
        for k in env_keys {
            hasher.update(k.as_bytes());
            hasher.update(env[k].as_bytes());
        }

        // 3. Step command + metadata
        hasher.update(step.run.as_bytes());
        if let Some(ref name) = step.name {
            hasher.update(name.as_bytes());
        }
        if let Some(ref wd) = step.working_directory {
            hasher.update(wd.as_bytes());
        }

        // 4. Watched file contents (the Phase 6 key feature)
        //    For each glob pattern, walk matching files (sorted), hash their contents.
        //    If a file changes, the hash changes → cache is invalidated automatically.
        if !step.watch.is_empty() {
            let file_hash = hash_watched_files(&step.watch);
            hasher.update(file_hash.as_bytes());
        }

        hex::encode(hasher.finalize())
    }

    // ─── Cache lookup ─────────────────────────────────────────────────────────

    /// Returns true only if the cache entry exists AND is within the TTL.
    pub fn is_cached(&self, hash: &str) -> bool {
        let meta_path = self.entry_dir(hash).join("meta.json");
        match read_entry_meta(&meta_path) {
            None => false,
            Some(entry) => {
                let now = now_secs();
                let age = now.saturating_sub(entry.created_at_secs);
                if age > self.ttl_secs {
                    debug!("Cache entry '{}' expired (age {}s > TTL {}s).", &hash[..8], age, self.ttl_secs);
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Returns entry metadata if it exists (regardless of TTL).
    pub fn get_entry(&self, hash: &str) -> Option<CacheEntry> {
        read_entry_meta(&self.entry_dir(hash).join("meta.json"))
    }

    // ─── Cache write ──────────────────────────────────────────────────────────

    /// Mark a step successful. Optionally archive output artifacts.
    ///
    /// `workspace` — root directory of the job workspace (artifacts are relative to it)
    pub fn update_cache(
        &self,
        hash: &str,
        base_os: &str,
        target_arch: &str,
        step: &Step,
        workspace: Option<&Path>,
    ) -> Result<()> {
        let entry_dir = self.entry_dir(hash);
        std::fs::create_dir_all(&entry_dir)?;

        // Archive artifacts if the step declares any outputs
        let has_artifacts = if !step.outputs.is_empty() {
            if let Some(ws) = workspace {
                match archive_artifacts(hash, &entry_dir, ws, &step.outputs) {
                    Ok(()) => {
                        info!("[Cache] Archived {} output(s) for step '{}'.",
                            step.outputs.len(),
                            step.name.as_deref().unwrap_or(&step.run));
                        true
                    }
                    Err(e) => {
                        // Non-fatal — cache the step result without artifacts
                        debug!("Could not archive artifacts: {}. Step still cached.", e);
                        false
                    }
                }
            } else { false }
        } else { false };

        // Write metadata
        let meta = CacheEntry {
            created_at_secs: now_secs(),
            os:  base_os.to_string(),
            arch: target_arch.to_string(),
            run: step.run.clone(),
            has_artifacts,
        };
        let json = serde_json::to_string_pretty(&meta)?;
        std::fs::write(entry_dir.join("meta.json"), json)?;

        debug!("[Cache] Entry saved: hash={}", &hash[..8]);
        Ok(())
    }

    // ─── Artifact restore ─────────────────────────────────────────────────────

    /// Restore archived artifacts to `workspace` from a cache hit.
    /// Called before skipping the step so downstream steps can use the outputs.
    pub fn restore_artifacts(&self, hash: &str, workspace: &Path) -> Result<()> {
        let artifacts_path = self.entry_dir(hash).join("artifacts.tar.gz");
        if !artifacts_path.exists() {
            return Ok(()); // No artifacts stored — nothing to restore
        }

        info!("[Cache] Restoring artifacts from cache entry '{}'...", &hash[..8]);
        let file = std::fs::File::open(&artifacts_path)
            .context("Cannot open cached artifacts")?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(workspace)
            .context("Failed to extract cached artifacts")?;

        info!("[Cache] Artifacts restored to {:?}", workspace);
        Ok(())
    }

    // ─── Cache management ─────────────────────────────────────────────────────

    /// List all cache entries with their metadata. Returns (hash, CacheEntry) pairs.
    pub fn list_entries(&self) -> Vec<(String, CacheEntry)> {
        let mut entries = Vec::new();
        let Ok(dir) = std::fs::read_dir(&self.cache_dir) else { return entries };
        for item in dir.flatten() {
            let hash = item.file_name().to_string_lossy().to_string();
            let meta_path = item.path().join("meta.json");
            if let Some(entry) = read_entry_meta(&meta_path) {
                entries.push((hash, entry));
            }
        }
        // Sort by creation time, newest first
        entries.sort_by(|a, b| b.1.created_at_secs.cmp(&a.1.created_at_secs));
        entries
    }

    /// Delete all cache entries.
    pub fn clean_all(&self) -> Result<usize> {
        let mut count = 0;
        if let Ok(dir) = std::fs::read_dir(&self.cache_dir) {
            for item in dir.flatten() {
                if item.path().is_dir() {
                    std::fs::remove_dir_all(item.path())?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Remove entries older than the configured TTL.
    pub fn clean_expired(&self) -> Result<usize> {
        let now = now_secs();
        let mut count = 0;
        if let Ok(dir) = std::fs::read_dir(&self.cache_dir) {
            for item in dir.flatten() {
                let meta_path = item.path().join("meta.json");
                if let Some(entry) = read_entry_meta(&meta_path) {
                    if now.saturating_sub(entry.created_at_secs) > self.ttl_secs {
                        std::fs::remove_dir_all(item.path())?;
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    // ─── Internals ────────────────────────────────────────────────────────────

    fn entry_dir(&self, hash: &str) -> PathBuf {
        self.cache_dir.join(hash)
    }
}

// ─── File-content hashing ─────────────────────────────────────────────────────

/// Hash the contents of all files matching the given glob patterns.
/// Files are sorted before hashing so the result is deterministic.
fn hash_watched_files(patterns: &[String]) -> String {
    let mut hasher = Sha256::new();
    let mut paths: Vec<PathBuf> = Vec::new();

    for pattern in patterns {
        if let Ok(entries) = glob::glob(pattern) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    paths.push(entry);
                }
            }
        }
    }

    // Sort for determinism
    paths.sort();

    for path in &paths {
        // Hash the path itself (catches renames/deletes)
        hasher.update(path.to_string_lossy().as_bytes());
        // Hash file content
        if let Ok(content) = std::fs::read(path) {
            hasher.update(&content);
        }
    }

    hex::encode(hasher.finalize())
}

// ─── Artifact archive / restore ───────────────────────────────────────────────

fn archive_artifacts(
    _hash: &str,
    entry_dir: &Path,
    workspace: &Path,
    outputs: &[String],
) -> Result<()> {
    let archive_path = entry_dir.join("artifacts.tar.gz");
    let file = std::fs::File::create(&archive_path)
        .context("Cannot create artifacts archive")?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut tar_builder = tar::Builder::new(encoder);

    for output_rel in outputs {
        let full_path = workspace.join(output_rel);
        if !full_path.exists() {
            debug!("Artifact '{}' not found — skipping from archive.", output_rel);
            continue;
        }
        if full_path.is_dir() {
            tar_builder.append_dir_all(output_rel, &full_path)
                .with_context(|| format!("Failed to archive dir '{}'", output_rel))?;
        } else {
            let mut f = std::fs::File::open(&full_path)
                .with_context(|| format!("Cannot open artifact '{}'", output_rel))?;
            tar_builder.append_file(output_rel, &mut f)
                .with_context(|| format!("Failed to archive file '{}'", output_rel))?;
        }
    }

    tar_builder.finish().context("Failed to finalize artifact archive")?;
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_entry_meta(path: &Path) -> Option<CacheEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
