use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Name of the backend (e.g., "container", "firecracker")
    fn name(&self) -> &str;

    /// Provision a unique lab workspace/session
    async fn provision(&self, lab_id: &str, base_os: &str, target_arch: &str) -> Result<()>;

    /// Execute a command within the lab session
    async fn execute(
        &self, 
        lab_id: &str, 
        base_os: &str,
        target_arch: &str,
        cmd: &str, 
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>
    ) -> Result<()>;

    /// Clean up the lab session
    async fn teardown(&self, lab_id: &str) -> Result<()>;
}
