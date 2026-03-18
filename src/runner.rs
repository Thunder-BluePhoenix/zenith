use crate::config::ZenithConfig;
use anyhow::{Result, Context};
use tracing::{info, error, debug};
use tokio::process::Command;
use std::process::Stdio;

/// Execute a local workflow (Phase 1 runner supporting Sandbox isolation)
pub async fn execute_local(config: ZenithConfig, target_job: Option<String>) -> Result<()> {
    
    // Resolve which steps to run
    let (job_name, runs_on, steps) = if let Some(jobs) = config.jobs {
        let job_name = target_job.unwrap_or_else(|| {
            jobs.keys().next().cloned().unwrap_or_else(|| "default".to_string())
        });
        
        if let Some(job) = jobs.get(&job_name) {
            info!("Preparing job: {}", job_name);
            (job_name, job.runs_on.clone().unwrap_or_else(|| "local".into()), job.steps.clone())
        } else {
            return Err(anyhow::anyhow!("Job '{}' not found in configuration.", job_name));
        }
    } else if let Some(steps) = config.steps {
        info!("Running default steps sequence");
        ("default".into(), "local".into(), steps)
    } else {
        return Err(anyhow::anyhow!("No jobs or steps defined in configuration."));
    };

    if steps.is_empty() {
        info!("No steps to execute.");
        return Ok(());
    }

    let is_sandboxed = runs_on != "local" && runs_on != "host";

    // Phase 1 Sandbox Provisioning
    let container_name = if is_sandboxed {
        info!("Provisioning ephemeral sandbox for OS: {}", runs_on);
        let name = crate::sandbox::provision_lab(&runs_on).await?;
        Some(name)
    } else {
        None
    };

    // Execute steps sequentially
    let mut workflow_success = true;
    for (i, step) in steps.iter().enumerate() {
        let step_name = step.name.as_deref().unwrap_or("Unnamed Step");
        info!(">>> [{}] Step {}: {}", job_name, i + 1, step_name);
        debug!("Command: {}", step.run);
        
        let result = if let Some(ref cname) = container_name {
            crate::sandbox::exec_in_lab(cname, &step.run).await
        } else {
            run_shell_command(&step.run).await
        };

        if let Err(e) = result {
            error!("Step failed: {}", e);
            workflow_success = false;
            break; // Stop executing subsequent steps on failure
        }
    }

    // Phase 1 Sandbox Teardown
    if let Some(ref cname) = container_name {
        info!("Tearing down ephemeral sandbox environment...");
        crate::sandbox::teardown_lab(cname).await.unwrap_or_else(|e| {
            error!("Failed to tear down lab automatically: {}", e);
        });
    }
    
    if workflow_success {
        info!("Workflow '{}' completed successfully!", job_name);
        Ok(())
    } else {
        Err(anyhow::anyhow!("Workflow '{}' failed.", job_name))
    }
}

async fn run_shell_command(cmd: &str) -> Result<()> {
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
        return Err(anyhow::anyhow!("Command failed: {}", cmd));
    }
    
    Ok(())
}
