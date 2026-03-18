use super::backend::Backend;
use anyhow::Result;
use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{info, warn};

pub struct FirecrackerBackend;

#[async_trait]
impl Backend for FirecrackerBackend {
    fn name(&self) -> &str {
        "firecracker"
    }

    async fn provision(&self, lab_id: &str, base_os: &str) -> Result<()> {
        info!("[Firecracker] Initializing MicroVM for lab: {} (base: {})", lab_id, base_os);
        warn!("Firecracker backend is currently a skeleton. Phase 4 progress: kernel/kvm integration pending.");
        // For now, this just validates the base OS and does nothing
        super::ensure_rootfs(base_os).await?;
        Ok(())
    }

    async fn execute(
        &self, 
        _lab_id: &str, 
        _base_os: &str,
        _cmd: &str, 
        _env: Option<HashMap<String, String>>,
        _working_directory: Option<String>
    ) -> Result<()> {
        warn!("Firecracker execution is not yet implemented on this platform.");
        Err(anyhow::anyhow!("Firecracker requires a Linux host with KVM enabled. Falling back to container backend is recommended on Windows."))
    }

    async fn teardown(&self, _lab_id: &str) -> Result<()> {
        info!("[Firecracker] Tearing down MicroVM session.");
        Ok(())
    }
}
