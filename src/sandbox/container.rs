use super::backend::Backend;
use anyhow::Result;
use std::collections::HashMap;
use async_trait::async_trait;

pub struct ContainerBackend;

#[async_trait]
impl Backend for ContainerBackend {
    fn name(&self) -> &str {
        "container"
    }

    async fn provision(&self, lab_id: &str, base_os: &str) -> Result<()> {
        super::provision_lab(lab_id, base_os).await
    }

    async fn execute(
        &self, 
        lab_id: &str, 
        base_os: &str,
        cmd: &str, 
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>
    ) -> Result<()> {
        super::exec_in_lab(lab_id, base_os, cmd, env, working_directory).await
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        super::teardown_lab(lab_id).await
    }
}
