use crate::config::{ZenithConfig, Job, EnvConfig};
use crate::ui::history::RunLogger;
use anyhow::{Result, Context};
use tracing::{info, error, debug, warn};
use tokio::process::Command;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Execute a local workflow.
/// `force` — when true, bypass all cache checks (used by `zenith build --no-cache`)
pub async fn execute_local(config: ZenithConfig, target_job: Option<String>, force: bool) -> Result<()> {
    // Capture top-level env block before consuming config fields
    let global_env = config.env.clone();

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
            toolchain: None,
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
        let genv = global_env.clone();
        set.spawn(async move {
            execute_single_job(&name, &job, matrix, force, genv.as_ref()).await
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

/// Execute a single job instance (potentially one node of a matrix).
async fn execute_single_job(
    base_name: &str,
    job: &Job,
    matrix: HashMap<String, String>,
    force: bool,
    global_env: Option<&EnvConfig>,
) -> Result<()> {
    // Generate a specific name for this matrix instance
    let instance_name = if matrix.is_empty() {
        base_name.to_string()
    } else {
        let suffix = matrix.values().cloned().collect::<Vec<_>>().join("-");
        format!("{}-{}", base_name, suffix)
    };

    info!("Starting job instance: {}", instance_name);

    // Phase 11: run logger — wrapped in Arc<Mutex> for concurrent step access
    let logger = Arc::new(tokio::sync::Mutex::new(RunLogger::new(&instance_name)));

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

    // Phase 6: Cache manager (None if cache dir can't be created)
    let cache_manager = crate::sandbox::cache::CacheManager::new().ok();

    // Workspace path for artifact restore/save (local execution path)
    let workspace_path = std::env::current_dir().ok();

    // Phase 7: Resolve toolchain env
    let effective_tc = job.toolchain.as_ref().or(global_env);
    let tool_env = if let Some(tc) = effective_tc {
        crate::toolchain::resolve_toolchain_env_from_config(tc).await
    } else {
        std::collections::HashMap::new()
    };

    // Phase 1/4/5 Sandbox Provisioning
    let container_id = if is_sandboxed {
        let unique_id = format!("{}-{}", runs_on, uuid::Uuid::new_v4().simple());
        info!("[{}] Provisioning {} sandbox: {} (Arch: {})", instance_name, backend.name(), unique_id, arch);
        backend.provision(&unique_id, &runs_on, &arch).await?;
        Some(unique_id)
    } else {
        None
    };

    // ─── Phase 13: Dependency-aware parallel step executor ─────────────────────
    // Wrap shared resources in Arc so concurrent step tasks can reference them.
    let backend      = Arc::new(backend);
    let container_id = Arc::new(container_id);
    let cache_mgr    = Arc::new(cache_manager);
    let workspace    = Arc::new(workspace_path);
    let tool_env     = Arc::new(tool_env);
    let job_env      = Arc::new(job.env.clone());
    let job_wd       = Arc::new(job.working_directory.clone());
    let job_cache    = job.cache;

    // Track which steps have not yet been started (by index).
    let mut pending: Vec<usize> = (0..job.steps.len()).collect();
    // Track completed step names so deps can be resolved.
    let mut completed: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Running step tasks — each yields (step_index, resolved_name, success).
    let mut running: JoinSet<(usize, String, bool)> = JoinSet::new();
    let mut success = true;

    loop {
        // Each iteration: spawn every pending step whose deps are now satisfied.
        if success {
            let mut still_pending = vec![];
            for idx in pending {
                let step = &job.steps[idx];
                let deps_met = step.depends_on.iter().all(|dep| {
                    completed.contains(&resolve_placeholders(dep, &matrix))
                });
                if !deps_met {
                    still_pending.push(idx);
                    continue;
                }

                // Clone everything the task needs.
                let step         = step.clone();
                let matrix       = matrix.clone();
                let iname        = instance_name.clone();
                let runs_on      = runs_on.clone();
                let arch         = arch.clone();
                let backend      = Arc::clone(&backend);
                let container_id = Arc::clone(&container_id);
                let cache_mgr    = Arc::clone(&cache_mgr);
                let workspace    = Arc::clone(&workspace);
                let tool_env     = Arc::clone(&tool_env);
                let job_env      = Arc::clone(&job_env);
                let job_wd       = Arc::clone(&job_wd);
                let logger       = Arc::clone(&logger);

                running.spawn(async move {
                    let step_name = resolve_placeholders(
                        step.name.as_deref().unwrap_or("Unnamed Step"),
                        &matrix,
                    );

                    // Merge env: toolchain → job → step
                    let mut merged_env: HashMap<String, String> = (*tool_env).clone();
                    if let Some(ref je) = *job_env {
                        for (k, v) in je {
                            merged_env.insert(k.clone(), resolve_placeholders(v, &matrix));
                        }
                    }
                    if let Some(ref se) = step.env {
                        for (k, v) in se {
                            merged_env.insert(k.clone(), resolve_placeholders(v, &matrix));
                        }
                    }

                    // Phase 6: cache check
                    let step_cache_on = !force && step.cache.unwrap_or(job_cache.unwrap_or(true));
                    let step_hash = if step_cache_on {
                        (*cache_mgr).as_ref().map(|cm| {
                            cm.compute_step_hash(&runs_on, &arch, &step, &merged_env)
                        })
                    } else {
                        None
                    };

                    if let Some(ref hash) = step_hash {
                        if (*cache_mgr).as_ref().map(|cm| cm.is_cached(hash)).unwrap_or(false) {
                            info!("[{}] [CACHED] {}", iname, step_name);
                            logger.lock().await.log_step_cached(idx, &step_name);
                            if let (Some(cm), Some(ws)) =
                                ((*cache_mgr).as_ref(), (*workspace).as_ref())
                            {
                                let _ = cm.restore_artifacts(hash, ws);
                            }
                            return (idx, step_name, true);
                        }
                    }

                    info!("[{}] Running: {}", iname, step_name);
                    logger.lock().await.log_step_start(idx, &step_name);

                    let resolved_run = resolve_placeholders(&step.run, &matrix);
                    let wd = step.working_directory.clone()
                        .or_else(|| (*job_wd).clone())
                        .map(|d| resolve_placeholders(&d, &matrix));

                    let (ok, log_lines) = if let Some(ref cid) = *container_id {
                        let res = backend
                            .execute(cid, &runs_on, &arch, &resolved_run, Some(merged_env), wd)
                            .await;
                        (res.is_ok(), vec![])
                    } else {
                        match run_shell_command(&resolved_run, Some(merged_env), wd).await {
                            Ok(lines)  => (true, lines),
                            Err(lines) => (false, lines),
                        }
                    };

                    logger.lock().await.log_step_done(idx, &step_name, ok, log_lines);

                    if !ok {
                        if step.allow_failure {
                            warn!("[{}] Step failed (allowed): {}", iname, step_name);
                        } else {
                            error!("[{}] Step failed: {}", iname, step_name);
                        }
                    } else if let Some(ref hash) = step_hash {
                        if let Some(cm) = (*cache_mgr).as_ref() {
                            let ws_ref = (*workspace).as_ref().map(|p| p.as_path());
                            let _ = cm.update_cache(hash, &runs_on, &arch, &step, ws_ref);
                        }
                    }

                    (idx, step_name, ok || step.allow_failure)
                });
            }
            pending = still_pending;
        }

        // Nothing running: either all done or remaining steps have unresolvable deps.
        if running.is_empty() {
            if !pending.is_empty() {
                warn!(
                    "[{}] {} step(s) have unsatisfied dependencies (cycle or unknown step name).",
                    instance_name, pending.len()
                );
                for idx in &pending {
                    warn!("  step {}: depends_on {:?}", idx + 1, job.steps[*idx].depends_on);
                }
                success = false;
            }
            break;
        }

        // Wait for the next step to finish.
        match running.join_next().await {
            Some(Ok((idx, step_name, ok))) => {
                info!(
                    "[{}] Step {} '{}' {}.",
                    instance_name, idx + 1, step_name,
                    if ok { "done" } else { "FAILED" }
                );
                completed.insert(step_name);
                if !ok {
                    success = false;
                    running.abort_all();
                }
            }
            Some(Err(e)) => {
                error!("[{}] Step task panicked: {}", instance_name, e);
                success = false;
                running.abort_all();
            }
            None => break,
        }
    }

    // Phase 1/4 Sandbox Teardown
    if let Some(ref cid) = *container_id {
        debug!("[{}] Tearing down sandbox...", instance_name);
        backend.teardown(cid).await.unwrap_or_else(|e| {
            error!("[{}] Failed to tear down lab: {}", instance_name, e);
        });
    }

    logger.lock().await.finalize(success);

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


/// Runs a shell command, printing output in real time and collecting log lines.
/// Returns Ok(lines) on success, Err(lines) on failure.
async fn run_shell_command(
    cmd: &str,
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>,
) -> std::result::Result<Vec<String>, Vec<String>> {
    #[cfg(target_os = "windows")]
    let (shell, flag) = ("cmd", "/C");
    #[cfg(not(target_os = "windows"))]
    let (shell, flag) = ("sh", "-c");

    let mut command = Command::new(shell);
    command.args([flag, cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(wd) = working_directory {
        command.current_dir(wd);
    }
    if let Some(env_vars) = env {
        for (k, v) in env_vars { command.env(k, v); }
    }

    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return Err(vec![format!("Failed to spawn: {}", e)]),
    };

    let out = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => return Err(vec![format!("Failed to wait: {}", e)]),
    };

    // Print captured output so the user still sees it in the terminal
    if !out.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&out.stdout));
    }
    if !out.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
    }

    // Collect both streams into log lines
    let mut lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();
    for l in String::from_utf8_lossy(&out.stderr).lines() {
        lines.push(format!("[stderr] {}", l));
    }

    if out.status.success() {
        Ok(lines)
    } else {
        lines.push(format!("exit status: {}", out.status));
        Err(lines)
    }
}
