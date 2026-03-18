use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ZenithConfig {
    pub jobs: Option<HashMap<String, Job>>,
    pub steps: Option<Vec<Step>>, // Simple single-job format fallback
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    #[serde(rename = "runs-on")]
    pub runs_on: Option<String>,
    pub steps: Vec<Step>,
    pub env: Option<HashMap<String, String>>,
    pub working_directory: Option<String>,
    pub strategy: Option<Strategy>,
    pub backend: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Strategy {
    pub matrix: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Step {
    pub name: Option<String>,
    pub run: String,
    pub env: Option<HashMap<String, String>>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub allow_failure: bool,
}

/// Load and parse .zenith.yml from the current directory
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<ZenithConfig> {
    if !path.as_ref().exists() {
        return Err(anyhow::anyhow!("Configuration file not found. Create a .zenith.yml file."));
    }
    
    let content = fs::read_to_string(path)
        .context("Failed to read configuration file")?;
        
    let config: ZenithConfig = serde_yaml::from_str(&content)
        .context("Failed to parse configuration file syntax")?;
        
    Ok(config)
}
