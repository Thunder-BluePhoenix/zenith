use super::backend::Backend;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

pub struct ContainerBackend;

#[async_trait]
impl Backend for ContainerBackend {
    fn name(&self) -> &str {
        "container"
    }

    async fn provision(&self, lab_id: &str, base_os: &str, target_arch: &str) -> Result<()> {
        info!("[Container] Provisioning lab: {} (OS: {}, Arch: {})", lab_id, base_os, target_arch);
        super::provision_lab(lab_id, base_os).await
    }

    async fn execute(
        &self, 
        lab_id: &str, 
        base_os: &str,
        target_arch: &str,
        cmd: &str, 
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>
    ) -> Result<()> {
        // In Phase 5, super::exec_in_lab should handle architecture emulation
        super::exec_in_lab(lab_id, base_os, target_arch, cmd, env, working_directory).await
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        super::teardown_lab(lab_id).await
    }
}
