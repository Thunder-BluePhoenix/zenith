use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Queued    => write!(f, "queued"),
            RunStatus::Running   => write!(f, "running"),
            RunStatus::Success   => write!(f, "success"),
            RunStatus::Failed    => write!(f, "failed"),
            RunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub run_id:     String,
    pub status:     RunStatus,
    pub created_at: String,
    pub updated_at: String,
    pub job:        Option<String>,
}

/// Cloud API config stored in ~/.zenith/config.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CloudConfig {
    #[serde(default = "default_api_url")]
    pub api_url: String,
    pub api_key:  Option<String>,
}

fn default_api_url() -> String {
    "https://api.zenith.run".to_string()
}
