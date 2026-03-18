use super::backend::Backend;
use anyhow::{Result, Context};
use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{info, warn};

pub struct WasmBackend;

#[async_trait]
impl Backend for WasmBackend {
    fn name(&self) -> &str {
        "wasm"
    }

    async fn provision(&self, lab_id: &str, _base_os: &str, _target_arch: &str) -> Result<()> {
        info!("[Wasm] Initializing WASI runtime for lab: {}", lab_id);
        // Wasm doesn't need a rootfs in the traditional sense, but we'll use the workspace
        let workspace = super::lab_state_dir(lab_id).join("workspace");
        std::fs::create_dir_all(&workspace).context("Failed to create wasm workspace")?;
        Ok(())
    }

    async fn execute(
        &self, 
        lab_id: &str, 
        _base_os: &str,
        _target_arch: &str,
        cmd: &str, 
        _env: Option<HashMap<String, String>>,
        _working_directory: Option<String>
    ) -> Result<()> {
        info!("[Wasm] Executing WASI module: {}", cmd);
        warn!("Wasm native execution (Wasmtime) is currently a skeleton. Phase 5 progress: WASI integration pending.");
        
        // In a real implementation, we would use the wasmtime crate here.
        // For the skeleton, we'll look for a .wasm file matching the command.
        let workspace = super::lab_state_dir(lab_id).join("workspace");
        let wasm_file = workspace.join(cmd);
        
        if wasm_file.exists() {
            info!("Found WASM module at: {:?}", wasm_file);
            Ok(())
        } else {
            warn!("WASM module not found: {:?}", wasm_file);
            // Simulate success for the skeleton
            Ok(())
        }
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        info!("[Wasm] Tearing down WASI session.");
        super::teardown_lab(lab_id).await
    }
}
