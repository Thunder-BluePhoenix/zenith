/// Remote execution orchestrator.
///
/// Flow:
///   1. Load the remote config (host, port, key)
///   2. Read the local .zenith.yml
///   3. Package the project as a tar.gz (excluding target/, .git/, etc.)
///   4. Upload the tarball to ~/.zenith/workspace/<run-id>/ on the remote
///   5. Bootstrap zenith-agent on the remote if not installed
///   6. Pipe the workflow task JSON to zenith-agent over SSH
///   7. Stream log output back to the local terminal

use anyhow::{Context, Result};
use tracing::info;
use uuid::Uuid;

use super::config::RemoteEntry;
use super::transport;

pub async fn execute_remote(
    remote_name: &str,
    remote: &RemoteEntry,
    config_yaml: &str,
    job: Option<&str>,
) -> Result<()> {
    // 1. Ping the remote and print its arch
    info!("Connecting to remote '{}' ({})...", remote_name, remote.host);
    let arch = transport::ping(remote).await
        .context("Cannot reach the remote host. Check your SSH config.")?;
    info!("Remote '{}' reachable (arch: {}).", remote_name, arch);

    // 2. Package the local project
    let local_dir = std::env::current_dir().context("Cannot determine current directory")?;
    info!("Packaging project from {:?}...", local_dir);
    let tarball = transport::package_project(&local_dir)
        .context("Failed to package project")?;
    info!("Project packaged ({} bytes).", tarball.len());

    // 3. Upload project to a unique workspace on the remote
    let run_id = Uuid::new_v4().simple().to_string();
    let workspace = transport::upload_project(remote, &tarball, &run_id).await
        .context("Failed to upload project to remote")?;

    // 4. Bootstrap the agent
    transport::bootstrap_agent(remote).await
        .context("Failed to install zenith-agent on remote")?;

    // 5. Run the workflow on the remote, streaming output back
    info!("Starting remote workflow on '{}' (workspace: {})...", remote_name, workspace);
    transport::run_agent(remote, remote_name, &workspace, config_yaml, job).await
        .context("Remote workflow execution failed")?;

    info!("Remote workflow on '{}' completed successfully.", remote_name);
    Ok(())
}
