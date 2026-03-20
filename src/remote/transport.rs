/// SSH transport — shells out to the system `ssh` and `scp` binaries.
///
/// Design: we use the system SSH client rather than a Rust SSH library.
/// This means zero extra dependencies and works everywhere OpenSSH is installed
/// (Linux, macOS, Windows 10+ all ship OpenSSH). SSH handles key agents,
/// known_hosts, and ProxyJump — Zenith gets that for free.
///
/// Upload protocol:
///   Local project → tar.gz in memory → piped to `ssh host 'tar xz -C <dir>'`
///
/// Execution protocol:
///   JSON config piped to `ssh host 'zenith-agent'` → stdout streamed back line by line.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};

use super::config::RemoteEntry;

/// Build the base `ssh` argument list for a remote entry.
fn ssh_args(remote: &RemoteEntry) -> Vec<String> {
    let mut args = vec![
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(), "BatchMode=yes".to_string(),
        "-p".to_string(), remote.port.to_string(),
    ];
    if let Some(ref key) = remote.key {
        args.push("-i".to_string());
        args.push(key.clone());
    }
    args.push(remote.host.clone());
    args
}

/// Check that the remote host is reachable. Returns the remote's `uname -m`.
pub async fn ping(remote: &RemoteEntry) -> Result<String> {
    let mut args = ssh_args(remote);
    args.push("uname -m".to_string());

    let out = Command::new("ssh")
        .args(&args)
        .output().await
        .context("Failed to run ssh — is OpenSSH installed?")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow::anyhow!("SSH connection failed:\n{}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Create an in-memory tar.gz of the project directory, excluding common noise.
pub fn package_project(dir: &Path) -> Result<Vec<u8>> {
    use flate2::{write::GzEncoder, Compression};
    use tar::Builder;

    let skip = [".git", "target", ".zenith", "node_modules", ".venv", "__pycache__"];

    let enc = GzEncoder::new(Vec::new(), Compression::fast());
    let mut ar = Builder::new(enc);

    for entry in walkdir(dir, &skip)? {
        let rel = entry.strip_prefix(dir)
            .with_context(|| format!("Failed to strip prefix from {:?}", entry))?;
        if entry.is_file() {
            ar.append_path_with_name(&entry, rel)
                .with_context(|| format!("Failed to archive {:?}", entry))?;
        }
    }

    let gz = ar.into_inner()
        .context("Failed to finalise tar builder")?;
    gz.finish().context("Failed to finish gzip encoder")
}

fn walkdir(dir: &Path, skip: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if skip.iter().any(|s| *s == name) { continue; }
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir(&path, skip)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}

/// Upload the project tarball to the remote.
/// Creates `~/.zenith/workspace/<label>/` on the remote and extracts there.
pub async fn upload_project(remote: &RemoteEntry, tarball: &[u8], label: &str) -> Result<String> {
    let remote_dir = format!("~/.zenith/workspace/{}", label);

    // Create the remote directory first
    let mkdir_args = {
        let mut a = ssh_args(remote);
        a.push(format!("mkdir -p {}", remote_dir));
        a
    };
    let mk = Command::new("ssh").args(&mkdir_args).output().await
        .context("Failed to create remote workspace dir")?;
    if !mk.status.success() {
        return Err(anyhow::anyhow!("mkdir on remote failed: {}", String::from_utf8_lossy(&mk.stderr)));
    }

    // Pipe tarball to `tar xz -C <dir>` on the remote
    let extract_args = {
        let mut a = ssh_args(remote);
        a.push(format!("tar xz -C {}", remote_dir));
        a
    };

    let mut child = Command::new("ssh")
        .args(&extract_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn ssh for project upload")?;

    child.stdin.take().unwrap().write_all(tarball).await
        .context("Failed to write tarball to ssh stdin")?;

    let out = child.wait_with_output().await
        .context("ssh upload process failed")?;

    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "Project upload failed: {}", String::from_utf8_lossy(&out.stderr)
        ));
    }

    info!("Project uploaded to remote:{}", remote_dir);
    Ok(remote_dir)
}

/// Ensure zenith-agent is installed on the remote under ~/.zenith/bin/zenith-agent.
/// Uploads the local zenith binary (which acts as the agent) if not present.
pub async fn bootstrap_agent(remote: &RemoteEntry) -> Result<()> {
    // Check if agent already exists
    let check_args = {
        let mut a = ssh_args(remote);
        a.push("test -x ~/.zenith/bin/zenith-agent && echo yes || echo no".to_string());
        a
    };
    let check = Command::new("ssh").args(&check_args).output().await
        .context("SSH check for agent failed")?;
    let present = String::from_utf8_lossy(&check.stdout).trim() == "yes";

    if present {
        debug!("zenith-agent already installed on remote.");
        return Ok(());
    }

    info!("Installing zenith-agent on remote...");

    // Determine path to the current zenith binary to use as the agent
    let agent_bin = std::env::current_exe()
        .context("Cannot determine path to current zenith binary")?;

    let agent_bytes = std::fs::read(&agent_bin)
        .with_context(|| format!("Cannot read agent binary from {:?}", agent_bin))?;

    // Ensure remote bin dir exists, then write binary
    let install_cmd = "mkdir -p ~/.zenith/bin && cat > ~/.zenith/bin/zenith-agent && chmod +x ~/.zenith/bin/zenith-agent";
    let install_args = {
        let mut a = ssh_args(remote);
        a.push(install_cmd.to_string());
        a
    };

    let mut child = Command::new("ssh")
        .args(&install_args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn ssh for agent install")?;

    child.stdin.take().unwrap().write_all(&agent_bytes).await
        .context("Failed to stream agent binary to remote")?;

    let out = child.wait_with_output().await?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "Agent install failed: {}", String::from_utf8_lossy(&out.stderr)
        ));
    }

    info!("zenith-agent installed on remote.");
    Ok(())
}

/// Run the workflow on the remote by piping a JSON task to zenith-agent over SSH.
/// Streams log output back to the local terminal with a `[remote:<label>]` prefix.
pub async fn run_agent(
    remote: &RemoteEntry,
    remote_name: &str,
    workspace_dir: &str,
    config_yaml: &str,
    job: Option<&str>,
) -> Result<()> {
    use serde_json::json;

    let task = serde_json::to_string(&json!({
        "config_yaml": config_yaml,
        "job":         job,
        "workspace":   workspace_dir,
    })).context("Failed to serialize agent task")?;

    // Run agent in the workspace directory
    let agent_cmd = format!(
        "cd {} && ~/.zenith/bin/zenith-agent --agent-mode",
        workspace_dir
    );
    let run_args = {
        let mut a = ssh_args(remote);
        a.push(agent_cmd);
        a
    };

    let mut child = Command::new("ssh")
        .args(&run_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn ssh for remote run")?;

    // Write the task JSON to agent stdin
    child.stdin.take().unwrap().write_all(task.as_bytes()).await
        .context("Failed to write task to agent stdin")?;

    // Stream stdout from agent, prefixing each line
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let prefix = format!("[remote:{}] ", remote_name);

    let prefix_out = prefix.clone();
    let out_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("{}{}", prefix_out, line);
        }
    });

    let err_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("{}{}", prefix, line);
        }
    });

    let status = child.wait().await.context("Remote agent process failed")?;
    let _ = out_task.await;
    let _ = err_task.await;

    if !status.success() {
        return Err(anyhow::anyhow!("Remote workflow failed (exit {})", status));
    }
    Ok(())
}
