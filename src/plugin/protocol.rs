/// JSON-RPC over stdio — the communication protocol between Zenith and plugin processes.
///
/// Every message is a single JSON object terminated by a newline (\n).
/// Zenith writes a RpcRequest to the plugin's stdin and reads one RpcResponse from stdout.
///
/// Required methods a plugin must implement:
///   "name"      → returns { "result": "<plugin-name>", ... }
///   "provision" → params: { lab_id, base_os, target_arch }
///   "execute"   → params: { lab_id, base_os, target_arch, cmd, env, working_directory }
///   "teardown"  → params: { lab_id }

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub params: Value,
    pub id:     u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    pub result: Option<Value>,
    pub error:  Option<String>,
    pub id:     u64,
}

impl RpcRequest {
    pub fn new(id: u64, method: &str, params: Value) -> Self {
        Self { method: method.to_string(), params, id }
    }
}

impl RpcResponse {
    pub fn into_result(self) -> anyhow::Result<Value> {
        if let Some(err) = self.error {
            Err(anyhow::anyhow!("Plugin error: {}", err))
        } else {
            Ok(self.result.unwrap_or(Value::Null))
        }
    }
}
