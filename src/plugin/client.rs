/// PluginBackend — implements the Backend trait by communicating with an
/// external plugin process over JSON-RPC / stdio.
///
/// Lifecycle:
///   - The plugin process is spawned fresh for each call (stateless model).
///   - This avoids managing long-lived child process state across async tasks.
///   - For heavy plugins that need persistent state, Phase 9 will add a
///     daemon-mode variant that keeps the process alive between calls.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use std::process::Stdio;
use tracing::debug;

use crate::sandbox::backend::Backend;
use super::manifest::PluginManifest;
use super::protocol::{RpcRequest, RpcResponse};

pub struct PluginBackend {
    manifest: PluginManifest,
}

impl PluginBackend {
    pub fn new(manifest: PluginManifest) -> Self {
        Self { manifest }
    }

    /// Spawn the plugin binary, send one JSON-RPC request, read one response.
    async fn call(&self, req: RpcRequest) -> Result<serde_json::Value> {
        let bin = self.manifest.entrypoint_path();
        let mut child = Command::new(&bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn plugin {:?}", bin))?;

        let mut stdin  = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        // Write request as one JSON line
        let line = serde_json::to_string(&req)? + "\n";
        debug!("Plugin → {}", line.trim_end());
        stdin.write_all(line.as_bytes()).await.context("Write to plugin stdin failed")?;
        drop(stdin); // signal EOF so plugin knows request is complete

        // Read one response line
        let mut reader = BufReader::new(stdout);
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.context("Read from plugin stdout failed")?;
        debug!("Plugin ← {}", resp_line.trim_end());

        let _ = child.wait().await;

        let resp: RpcResponse = serde_json::from_str(resp_line.trim())
            .with_context(|| format!("Invalid JSON-RPC response from plugin: {:?}", resp_line))?;

        if resp.id != req.id {
            return Err(anyhow::anyhow!(
                "Plugin response id {} does not match request id {}", resp.id, req.id
            ));
        }

        resp.into_result()
    }
}

#[async_trait]
impl Backend for PluginBackend {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    async fn provision(&self, lab_id: &str, base_os: &str, target_arch: &str) -> Result<()> {
        let req = RpcRequest::new(1, "provision", json!({
            "lab_id": lab_id,
            "base_os": base_os,
            "target_arch": target_arch,
        }));
        self.call(req).await.with_context(|| {
            format!("Plugin '{}' provision failed", self.manifest.name)
        })?;
        Ok(())
    }

    async fn execute(
        &self,
        lab_id: &str,
        base_os: &str,
        target_arch: &str,
        cmd: &str,
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>,
    ) -> Result<()> {
        let req = RpcRequest::new(2, "execute", json!({
            "lab_id": lab_id,
            "base_os": base_os,
            "target_arch": target_arch,
            "cmd": cmd,
            "env": env.unwrap_or_default(),
            "working_directory": working_directory,
        }));
        self.call(req).await.with_context(|| {
            format!("Plugin '{}' execute failed", self.manifest.name)
        })?;
        Ok(())
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        let req = RpcRequest::new(3, "teardown", json!({ "lab_id": lab_id }));
        self.call(req).await.with_context(|| {
            format!("Plugin '{}' teardown failed", self.manifest.name)
        })?;
        Ok(())
    }
}

/// Run a quick smoke test: send `name` RPC and expect a string response.
/// Returns the plugin's self-reported name.
pub async fn smoke_test(manifest: &PluginManifest) -> Result<String> {
    let backend = PluginBackend::new(manifest.clone());
    let req = RpcRequest::new(0, "name", serde_json::Value::Null);
    let result = backend.call(req).await
        .context("Plugin smoke test failed")?;
    result.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Plugin 'name' RPC did not return a string"))
}
