use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use sha2::{Sha256, Digest};
use hex;
use crate::config::Step;

pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let home = crate::sandbox::zenith_home();
        let cache_dir = home.join("cache");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Compute a unique hash for a step based on its inputs
    pub fn compute_step_hash(
        &self,
        base_os: &str,
        target_arch: &str,
        step: &Step,
        env: &HashMap<String, String>,
    ) -> String {
        let mut hasher = Sha256::new();

        // 1. Hash the environment (sorted for determinism)
        let mut env_keys: Vec<_> = env.keys().collect();
        env_keys.sort();
        for key in env_keys {
            hasher.update(key.as_bytes());
            hasher.update(env.get(key).unwrap().as_bytes());
        }

        // 2. Hash the base environment details
        hasher.update(base_os.as_bytes());
        hasher.update(target_arch.as_bytes());

        // 3. Hash the step command and attributes
        hasher.update(step.run.as_bytes());
        if let Some(ref name) = step.name {
            hasher.update(name.as_bytes());
        }
        if let Some(ref wd) = step.working_directory {
            hasher.update(wd.as_bytes());
        }

        // Return hex string of the hash
        hex::encode(hasher.finalize())
    }

    /// Check if a step hash is in the cache
    pub fn is_cached(&self, hash: &str) -> bool {
        self.cache_dir.join(hash).exists()
    }

    /// Mark a step as cached
    pub fn update_cache(&self, hash: &str) -> Result<()> {
        std::fs::write(self.cache_dir.join(hash), "SUCCESS")?;
        Ok(())
    }
}
