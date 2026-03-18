use crate::config::{ZenithConfig, Job};
use anyhow::{Result, Context};
use tracing::{info, error, debug, warn};
use tokio::process::Command;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Execute a local workflow (Phase 1 runner supporting Sandbox isolation)
pub async fn execute_local(config: ZenithConfig, target_job: Option<String>) -> Result<()> {
    
    // Resolve which job to run
    let (job_name, base_job) = if let Some(jobs) = config.jobs {
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
            strategy: None,
            backend: None,
            arch: None,
            cache: None,
        })
    } else {
        return Err(anyhow::anyhow!("No jobs or steps defined in configuration."));
    };

    // Expand matrix if strategy is present
    let matrix_combinations = if let Some(ref strategy) = base_job.strategy {
        expand_matrix(&strategy.matrix)
    } else {
        vec![HashMap::new()]
    };

    let mut set = JoinSet::new();
    let base_job = Arc::new(base_job);
    let job_name = Arc::new(job_name);

    for matrix in matrix_combinations {
        let job = base_job.clone();
        let name = job_name.clone();
        
        set.spawn(async move {
            execute_single_job(&name, &job, matrix).await
        });
    }

    let mut overall_success = true;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => {
                error!("Parallel job failed: {}", e);
                overall_success = false;
            },
            Err(e) => {
                error!("Task execution error: {}", e);
                overall_success = false;
            }
        }
    }

    if overall_success {
        info!("All workflow jobs completed successfully!");
        Ok(())
    } else {
        Err(anyhow::anyhow!("Some workflow jobs failed."))
    }
}

/// Execute a single job instance (potentially one node of a matrix)
async fn execute_single_job(
    base_name: &str, 
    job: &Job, 
    matrix: HashMap<String, String>
) -> Result<()> {
    // Generate a specific name for this matrix instance
    let instance_name = if matrix.is_empty() {
        base_name.to_string()
    } else {
        let suffix = matrix.values().cloned().collect::<Vec<_>>().join("-");
        format!("{}-{}", base_name, suffix)
    };

    info!("Starting job instance: {}", instance_name);

    // Resolve placeholders in runs-on
    let runs_on = resolve_placeholders(
        job.runs_on.as_deref().unwrap_or("local"), 
        &matrix
    );

    // Resolve target architecture (default to host arch)
    let arch = job.arch.as_deref()
        .map(|a| resolve_placeholders(a, &matrix))
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());

    // Get the requested backend (default to container)
    let backend_name = job.backend.as_deref().unwrap_or("container");
    let backend = crate::sandbox::get_backend(backend_name);

    let is_sandboxed = (runs_on != "local" && runs_on != "host") || backend.name() != "container";

    // Phase 6: Initialize Cache Manager
    let cache_manager = crate::sandbox::cache::CacheManager::new().ok();

    // Phase 1/4/5 Sandbox Provisioning using the backend abstraction
    let container_id = if is_sandboxed {
        let unique_id = format!("{}-{}", runs_on, uuid::Uuid::new_v4().simple());
        info!("[{}] Provisioning {} sandbox: {} (Arch: {})", instance_name, backend.name(), unique_id, arch);
        backend.provision(&unique_id, &runs_on, &arch).await?;
        Some(unique_id)
    } else {
        None
    };

    // Execute steps sequentially within this job instance
    let mut success = true;
    for (i, step) in job.steps.iter().enumerate() {
        let step_name = step.name.as_deref().unwrap_or("Unnamed Step");
        let resolved_name = resolve_placeholders(step_name, &matrix);
        
        // Merge environment variables: Job level -> Step level
        let mut merged_env = HashMap::new();
        if let Some(ref job_env) = job.env {
            for (k, v) in job_env {
                merged_env.insert(k.clone(), resolve_placeholders(v, &matrix));
            }
        }
        if let Some(ref step_env) = step.env {
            for (k, v) in step_env {
                merged_env.insert(k.clone(), resolve_placeholders(v, &matrix));
            }
        }

        // Phase 6: Performance Caching check
        let step_cache_enabled = step.cache.unwrap_or(job.cache.unwrap_or(true));
        let step_hash = if step_cache_enabled {
            cache_manager.as_ref().map(|cm| cm.compute_step_hash(&runs_on, &arch, step, &merged_env))
        } else {
            None
        };

        if let Some(ref hash) = step_hash {
            if cache_manager.as_ref().map(|cm| cm.is_cached(hash)).unwrap_or(false) {
                info!("[{}] [CACHED] Step {}: {} (Skipping execution)", instance_name, i + 1, resolved_name);
                continue;
            }
        }

        info!("[{}] Step {}: {}", instance_name, i + 1, resolved_name);

        // Resolve command placeholders
        let resolved_run = resolve_placeholders(&step.run, &matrix);

        // Determine working directory (Step level overrides Job level)
        let wd = step.working_directory.clone()
            .or_else(|| job.working_directory.clone())
            .map(|d| resolve_placeholders(&d, &matrix));

        let result = if let Some(ref cid) = container_id {
            backend.execute(cid, &runs_on, &arch, &resolved_run, Some(merged_env), wd).await
        } else {
            run_shell_command(&resolved_run, Some(merged_env), wd).await
        };

        if let Err(e) = result {
            if step.allow_failure {
                warn!("[{}] Step failed (allowed): {}", instance_name, e);
            } else {
                error!("[{}] Step failed: {}", instance_name, e);
                success = false;
                break;
            }
        } else {
            // Phase 6: Update cache on success
            if let Some(ref hash) = step_hash {
                if let Some(ref cm) = cache_manager {
                    let _ = cm.update_cache(hash);
                }
            }
            info!("[{}] Step completed successfully", instance_name);
        }
    }

    // Phase 1/4 Sandbox Teardown
    if let Some(ref cid) = container_id {
        debug!("[{}] Tearing down sandbox...", instance_name);
        backend.teardown(cid).await.unwrap_or_else(|e| {
            error!("[{}] Failed to tear down lab: {}", instance_name, e);
        });
    }

    if success {
        info!("[{}] Completed successfully!", instance_name);
        Ok(())
    } else {
        Err(anyhow::anyhow!("[{}] Failed.", instance_name))
    }
}

/// Helper to expand matrix strategy combinations
fn expand_matrix(matrix: &HashMap<String, Vec<String>>) -> Vec<HashMap<String, String>> {
    let mut combinations = vec![HashMap::new()];

    for (key, values) in matrix {
        let mut new_combinations = Vec::new();
        for combination in combinations {
            for value in values {
                let mut new_combination = combination.clone();
                new_combination.insert(key.clone(), value.clone());
                new_combinations.push(new_combination);
            }
        }
        combinations = new_combinations;
    }

    combinations
}

/// Replace ${{ matrix.key }} in strings
fn resolve_placeholders(text: &str, matrix: &HashMap<String, String>) -> String {
    let mut resolved = text.to_string();
    for (key, value) in matrix {
        let placeholder = format!("${{{{ matrix.{} }}}}", key);
        resolved = resolved.replace(&placeholder, value);
    }
    resolved
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
