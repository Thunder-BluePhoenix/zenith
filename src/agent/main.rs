/// zenith-agent — remote workflow executor
///
/// This binary is the same crate as `zenith`, compiled as a second [[bin]] target.
/// It runs on the remote machine when `zenith run --remote <name>` is used.
///
/// Invocation (by the transport layer over SSH):
///   echo '<task-json>' | zenith-agent --agent-mode
///
/// Input (stdin): one JSON object
///   {
///     "config_yaml": "<.zenith.yml contents>",
///     "job":         "job-name or null",
///     "workspace":   "/path/to/uploaded/workspace"
///   }
///
/// Output (stdout/stderr): plain text log lines from the workflow runner.
/// Exit code: 0 on success, 1 on failure.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Deserialize)]
struct AgentTask {
    config_yaml: String,
    job:         Option<String>,
    workspace:   String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Agent mode is indicated by the --agent-mode flag
    let args: Vec<String> = std::env::args().collect();
    if !args.contains(&"--agent-mode".to_string()) {
        eprintln!("zenith-agent: not invoked in --agent-mode. Use `zenith` for local runs.");
        std::process::exit(1);
    }

    // Set up logging to stderr only (stdout is reserved for structured output if needed)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    // Read the task JSON from stdin
    let mut input = String::new();
    use std::io::Read;
    std::io::stdin().read_to_string(&mut input)
        .context("Failed to read agent task from stdin")?;

    let task: AgentTask = serde_json::from_str(input.trim())
        .context("Failed to parse agent task JSON from stdin")?;

    // Change to the uploaded workspace directory
    std::env::set_current_dir(&task.workspace)
        .with_context(|| format!("Cannot change to workspace {:?}", task.workspace))?;

    info!("zenith-agent: running in workspace {:?}", task.workspace);

    // Write the config to a temp file and parse it
    let config_path = format!("{}/zenith-agent-run.yml", task.workspace);
    std::fs::write(&config_path, &task.config_yaml)
        .context("Failed to write config to workspace")?;

    let cfg = zenith::config::load_config(&config_path)
        .context("Failed to parse workflow config")?;

    // Run the workflow locally on the remote machine
    zenith::runner::execute_local(cfg, task.job, false).await
        .context("Workflow execution failed on remote")?;

    Ok(())
}
