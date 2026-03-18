use crate::config::{ZenithConfig, Job, Step};
use anyhow::{Result, Context};
use tracing::{info, error, debug};
use tokio::process::Command;
use std::process::Stdio;

/// Execute a local workflow sequentially (Phase 0 runner)
pub async fn execute_local(config: ZenithConfig, target_job: Option<String>) -> Result<()> {
    
    // Resolve which steps to run
    let steps = if let Some(jobs) = config.jobs {
        // Multi-job format
        let job_name = target_job.unwrap_or_else(|| {
            // Pick the first one if not specified
            jobs.keys().next().cloned().unwrap_or_else(|| "default".to_string())
        });
        
        if let Some(job) = jobs.get(&job_name) {
            info!("Running job: {}", job_name);
            job.steps.clone()
        } else {
            return Err(anyhow::anyhow!("Job '{}' not found in configuration.", job_name));
        }
    } else if let Some(steps) = config.steps {
        // Simple single-level steps format
        info!("Running default steps sequence");
        steps
    } else {
        return Err(anyhow::anyhow!("No jobs or steps defined in configuration."));
    };

    if steps.is_empty() {
        info!("No steps to execute.");
        return Ok(());
    }

    // Execute steps sequentially
    for (i, step) in steps.iter().enumerate() {
        let step_name = step.name.as_deref().unwrap_or("Unnamed Step");
        info!(">>> Step {}: {}", i + 1, step_name);
        debug!("Command: {}", step.run);
        
        run_shell_command(&step.run).await?;
    }
    
    info!("Workflow completed successfully!");
    Ok(())
}

async fn run_shell_command(cmd: &str) -> Result<()> {
    // On Windows, use cmd.exe /C, on Unix use sh -c
    #[cfg(target_os = "windows")]
    let shell = "cmd";
    #[cfg(target_os = "windows")]
    let args = ["/C", cmd];

    #[cfg(not(target_os = "windows"))]
    let shell = "sh";
    #[cfg(not(target_os = "windows"))]
    let args = ["-c", cmd];

    let mut child = Command::new(shell)
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context(format!("Failed to spawn command: {}", cmd))?;

    let status = child.wait().await?;
    
    if !status.success() {
        error!("Process exited with status: {}", status);
        return Err(anyhow::anyhow!("Command failed: {}", cmd));
    }
    
    Ok(())
}
