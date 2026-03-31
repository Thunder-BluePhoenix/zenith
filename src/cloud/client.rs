/// Zenith cloud API client.
///
/// NOTE: The Zenith cloud service (api.zenith.run) is planned for Phase 10.
/// This client implements the full API surface; calls will return an error
/// until the service is live.
///
/// API design:
///   POST /runs            — submit a workflow run
///   GET  /runs/:id        — poll run status
///   GET  /runs/:id/logs   — fetch logs (SSE stream when ?stream=true)
///   POST /runs/:id/cancel — cancel a running job
///   GET  /runs            — list recent runs

use anyhow::{Context, Result};
use reqwest::Client;
use tracing::info;

use super::types::{CloudConfig, RunInfo};

pub struct CloudClient {
    pub config: CloudConfig,
    http:       Client,
}

impl CloudClient {
    pub fn new(config: CloudConfig) -> Self {
        Self { config, http: Client::new() }
    }

    fn api_key(&self) -> Result<&str> {
        self.config.api_key.as_deref()
            .ok_or_else(|| anyhow::anyhow!(
                "Not authenticated. Run `zenith cloud login` first."
            ))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.api_url, path)
    }

    /// Submit a workflow run. Returns the run ID.
    pub async fn submit_run(
        &self,
        config_yaml: &str,
        project_tar: Vec<u8>,
        job: Option<&str>,
    ) -> Result<String> {
        let key = self.api_key()?;
        use reqwest::multipart::{Form, Part};

        let form = Form::new()
            .text("config", config_yaml.to_string())
            .text("job", job.unwrap_or("").to_string())
            .part("project", Part::bytes(project_tar).file_name("project.tar.gz"));

        let resp = self.http
            .post(self.url("/runs"))
            .header("Authorization", format!("Bearer {}", key))
            .multipart(form)
            .send().await
            .context("Failed to reach Zenith cloud API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Cloud API error {}: {}", status, body));
        }

        #[derive(serde::Deserialize)]
        struct SubmitResp { run_id: String }
        let r: SubmitResp = resp.json().await.context("Invalid submit response")?;
        Ok(r.run_id)
    }

    /// Poll the status of a run.
    pub async fn get_status(&self, run_id: &str) -> Result<RunInfo> {
        let key = self.api_key()?;
        let resp = self.http
            .get(self.url(&format!("/runs/{}", run_id)))
            .header("Authorization", format!("Bearer {}", key))
            .send().await
            .context("Failed to reach Zenith cloud API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Cloud API error {}: {}", status, body));
        }

        resp.json().await.context("Invalid status response")
    }

    /// Stream logs for a run using Server-Sent Events (SSE).
    /// Prints each log line to stdout as it arrives.
    pub async fn stream_logs(&self, run_id: &str) -> Result<()> {
        use futures_util::StreamExt;

        let key = self.api_key()?;
        let resp = self.http
            .get(self.url(&format!("/runs/{}/logs?stream=true", run_id)))
            .header("Authorization", format!("Bearer {}", key))
            .header("Accept", "text/event-stream")
            .send().await
            .context("Failed to reach Zenith cloud API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Cloud API error {}: {}", status, body));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading log stream")?;
            buf.push_str(&String::from_utf8_lossy(&chunk));

            // Parse SSE frames — each event ends with \n\n
            while let Some(pos) = buf.find("\n\n") {
                let event = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                for line in event.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" { return Ok(()); }
                        println!("{}", data);
                    }
                }
            }
        }

        Ok(())
    }

    /// Cancel a running job.
    pub async fn cancel_run(&self, run_id: &str) -> Result<()> {
        let key = self.api_key()?;
        let resp = self.http
            .post(self.url(&format!("/runs/{}/cancel", run_id)))
            .header("Authorization", format!("Bearer {}", key))
            .send().await
            .context("Failed to reach Zenith cloud API")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Cancel failed: {}", body));
        }
        Ok(())
    }

    /// List recent runs.
    pub async fn list_runs(&self) -> Result<Vec<RunInfo>> {
        let key = self.api_key()?;
        let resp = self.http
            .get(self.url("/runs"))
            .header("Authorization", format!("Bearer {}", key))
            .send().await
            .context("Failed to reach Zenith cloud API")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("List runs failed: {}", body));
        }

        resp.json().await.context("Invalid list response")
    }
}

// ─── Cloud config persistence ─────────────────────────────────────────────────

fn config_path() -> std::path::PathBuf {
    crate::sandbox::zenith_home().join("config.toml")
}

pub fn load_cloud_config() -> CloudConfig {
    let path = config_path();
    if !path.exists() { return CloudConfig::default(); }
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    #[derive(serde::Deserialize, Default)]
    struct Wrapper { #[serde(default)] cloud: CloudConfig }
    let w: Wrapper = toml::from_str(&raw).unwrap_or_default();
    w.cloud
}

pub fn save_api_key(key: &str) -> Result<()> {
    let path = config_path();
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }

    // Read existing file to preserve other sections
    let existing = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    // Simple approach: replace or append the [cloud] section
    let new_section = format!("[cloud]\napi_key = \"{}\"\n", key);
    let updated = if existing.contains("[cloud]") {
        // Replace existing [cloud] section up to next section or EOF
        let start = existing.find("[cloud]").unwrap();
        let end = existing[start + 7..].find("\n[")
            .map(|p| start + 7 + p)
            .unwrap_or(existing.len());
        format!("{}{}{}", &existing[..start], new_section, &existing[end..])
    } else {
        format!("{}\n{}", existing.trim_end(), new_section)
    };

    std::fs::write(&path, updated)
        .with_context(|| format!("Cannot write {:?}", path))
}

pub fn clear_api_key() -> Result<()> {
    info!("Clearing cloud API key from config.");
    save_api_key("")
}
