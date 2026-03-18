use crate::config::{ZenithConfig, Job, Step};
use anyhow::{Result, Context};
use tracing::{info, error, debug, warn};
use tokio::process::Command;
use std::collections::HashMap;
use std::process::Stdio;

/// Execute a local workflow (Phase 1 runner supporting Sandbox isolation)
pub async fn execute_local(config: ZenithConfig, target_job: Option<String>) -> Result<()> {
    
    // Resolve which job to run
    let (job_name, job) = if let Some(jobs) = config.jobs {
        let name = target_job.unwrap_or_else(|| {
            jobs.keys().next().cloned().unwrap_or_else(|| "default".to_string())
        });
        
        let job = jobs.get(&name)
            .ok_or_else(|| anyhow::anyhow!("Job '{}' not found in configuration.", name))?;
            
        info!("Preparing job: {}", name);
        (name, job.clone())
    } else if let Some(steps) = config.steps {
        info!("Running default steps sequence");
        ("default".into(), Job {
            runs_on: Some("local".into()),
            steps,
            env: None,
            working_directory: None,
        })
    } else {
        return Err(anyhow::anyhow!("No jobs or steps defined in configuration."));
    };

    let runs_on = job.runs_on.unwrap_or_else(|| "local".into());
    let steps = job.steps;

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
        
        // Merge environment variables: Job level -> Step level
        let mut merged_env = HashMap::new();
        if let Some(ref job_env) = job.env {
            merged_env.extend(job_env.clone());
        }
        if let Some(ref step_env) = step.env {
            merged_env.extend(step_env.clone());
        }

        // Determine working directory (Step level overrides Job level)
        let wd = step.working_directory.clone().or_else(|| job.working_directory.clone());

        let result = if let Some(ref cname) = container_name {
            crate::sandbox::exec_in_lab(cname, &step.run, Some(merged_env), wd).await
        } else {
            run_shell_command(&step.run, Some(merged_env), wd).await
        };

        if let Err(e) = result {
            if step.allow_failure {
                warn!("Step failed (allowed): {}", e);
            } else {
                error!("Step failed: {}", e);
                workflow_success = false;
                break; // Stop executing subsequent steps on failure
            }
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

async fn run_shell_command(
    cmd: &str, 
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    #[cfg(target_os = "windows")]
    let shell = "cmd";
    #[cfg(target_os = "windows")]
    let args = ["/C", cmd];

    #[cfg(not(target_os = "windows"))]
    let shell = "sh";
    #[cfg(not(target_os = "windows"))]
    let args = ["-c", cmd];

    let mut command = Command::new(shell);
    command.args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(wd) = working_directory {
        command.current_dir(wd);
    }

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            command.env(k, v);
        }
    }

    let mut child = command.spawn()
        .context(format!("Failed to spawn command: {}", cmd))?;

    let status = child.wait().await?;
    
    if !status.success() {
        return Err(anyhow::anyhow!("Command failed: {}", cmd));
    }
    
    Ok(())
}
