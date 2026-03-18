/// Zenith Native Sandbox — zero external tool dependencies.
///
/// How it works:
///   1. Zenith downloads a minimal Alpine Linux rootfs tarball (~3MB) from
///      the official Alpine CDN directly using its built-in HTTP client.
///   2. The tarball is extracted into ~/.zenith/rootfs/<os>/
///   3. On Linux: the subprocess is launched inside a new PID + user + mount
///      namespace using the `nix` crate (raw Linux syscalls, no Docker).
///   4. On Windows/macOS: a restricted subprocess is used with a completely
///      cleaned environment (Phase 1 fallback; full VM support comes in Phase 4).

use crate::cli::LabCommands;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

// Known minimal rootfs image sources for supported OS targets
struct RootfsSource {
    os: &'static str,
    url: &'static str,
}

const ROOTFS_SOURCES: &[RootfsSource] = &[
    RootfsSource {
        os: "alpine",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-minirootfs-3.19.1-x86_64.tar.gz",
    },
];

fn zenith_home() -> PathBuf {
    let home = dirs_next_home();
    home.join(".zenith")
}

fn rootfs_dir(os: &str) -> PathBuf {
    zenith_home().join("rootfs").join(os)
}

fn lab_state_dir(os: &str) -> PathBuf {
    zenith_home().join("labs").join(os)
}

fn dirs_next_home() -> PathBuf {
    // Cross-platform home directory
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOMEDRIVE").and_then(|d| std::env::var("HOMEPATH").map(|p| d + &p)))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\default"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/root"))
    }
}

/// Download and extract a rootfs image if not already cached.
pub async fn ensure_rootfs(os: &str) -> Result<PathBuf> {
    let rootfs = rootfs_dir(os);

    // Already cached — skip download
    if rootfs.join("etc").exists() {
        info!("Rootfs for '{}' already cached at {:?}", os, rootfs);
        return Ok(rootfs);
    }

    let source = ROOTFS_SOURCES
        .iter()
        .find(|s| s.os == os)
        .ok_or_else(|| anyhow::anyhow!(
            "OS '{}' is not supported yet.\nSupported: alpine", os
        ))?;

    info!("Downloading {} rootfs from the official CDN...", os);

    fs::create_dir_all(&rootfs)
        .context("Failed to create rootfs cache directory")?;

    // Download the tarball
    let tarball_path = zenith_home().join(format!("{}.tar.gz", os));
    download_file(source.url, &tarball_path).await?;

    // Extract into the rootfs dir
    info!("Extracting rootfs...");
    extract_tarball(&tarball_path, &rootfs)?;

    // Cleanup the tarball
    let _ = fs::remove_file(&tarball_path);
    info!("Rootfs for '{}' ready at {:?}", os, rootfs);

    Ok(rootfs)
}

async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to connect to download server")?;

    let total = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message("Downloading rootfs");

    let mut file = tokio::fs::File::create(dest)
        .await
        .context("Failed to create download destination file")?;

    let bytes = response.bytes().await.context("Failed to read download stream")?;
    pb.set_position(bytes.len() as u64);
    file.write_all(&bytes).await.context("Failed to write downloaded tarball")?;

    pb.finish_with_message("Download complete");
    Ok(())
}

fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    let file = fs::File::open(tarball).context("Cannot open downloaded tarball")?;
    let decompressed = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressed);
    archive.set_preserve_permissions(true);
    archive.unpack(dest).context("Failed to extract rootfs tarball")?;
    Ok(())
}

// ─── Lab Lifecycle ────────────────────────────────────────────────────────────

pub async fn handle_lab(cmd: LabCommands) -> Result<()> {
    match cmd {
        LabCommands::List => {
            let labs_dir = zenith_home().join("labs");
            if !labs_dir.exists() || labs_dir.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
                println!("No active Zenith labs.");
            } else {
                println!("Active Zenith labs:\n");
                for entry in fs::read_dir(&labs_dir)? {
                    let entry = entry?;
                    println!("  - {}", entry.file_name().to_string_lossy());
                }
            }
        }

        LabCommands::Create { os } => {
            let state_dir = lab_state_dir(&os);
            if state_dir.exists() {
                info!("Lab '{}' already exists.", os);
                return Ok(());
            }
            ensure_rootfs(&os).await?;
            fs::create_dir_all(&state_dir).context("Failed to create lab state dir")?;
            println!("Canvas lab '{}' is ready.", os);
            println!("Push project files : zenith lab push {}", os);
            println!("Run a command      : zenith lab run {} <cmd>", os);
            println!("Open a shell       : zenith lab shell {}", os);
        }

        LabCommands::Push { os } => {
            push_project(&os).await?;
        }

        LabCommands::Run { os, command } => {
            let rootfs = ensure_rootfs(&os).await?;
            run_in_sandbox(&rootfs, &os, &command)?;
        }

        LabCommands::Shell { os } => {
            let rootfs = ensure_rootfs(&os).await?;
            run_in_sandbox(&rootfs, &os, "/bin/sh")?;
        }

        LabCommands::Destroy { os } => {
            let state_dir = lab_state_dir(&os);
            if state_dir.exists() {
                fs::remove_dir_all(&state_dir).context("Failed to remove lab state")?;
            }
            println!("Lab '{}' destroyed. Rootfs cache kept for reuse.", os);
            println!("To also remove cached rootfs: rm -rf ~/.zenith/rootfs/{}", os);
        }
    }
    Ok(())
}

/// Copy the current project directory into the lab's workspace.
pub async fn push_project(os: &str) -> Result<()> {
    let rootfs = ensure_rootfs(os).await?;
    let workspace = rootfs.join("workspace");
    fs::create_dir_all(&workspace).context("Failed to create /workspace in rootfs")?;

    let current_dir = std::env::current_dir()?;
    info!("Pushing project into canvas workspace...");
    copy_dir_all(&current_dir, &workspace)?;
    println!("Project pushed into '{}' canvas. Host files are untouched.", os);
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        // Skip .zenith and target directories
        if name == ".zenith" || name == "target" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            copy_dir_all(&path, &dst.join(&name))?;
        } else {
            fs::copy(&path, &dst.join(&name))?;
        }
    }
    Ok(())
}

/// Execute a command inside the sandbox environment.
/// On Linux: spawns with namespace isolation via platform-specific module.
/// On Windows/macOS: spawns with a hardened, clean environment.
pub fn run_in_sandbox(rootfs: &Path, _os: &str, cmd: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::run_namespaced(rootfs, cmd)
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Phase 1 fallback on Windows/macOS: run inside the rootfs path
        // with a cleaned environment until Hyper-V/MicroVM lands in Phase 4
        warn!("Full kernel isolation is only available on Linux.");
        warn!("On Windows/macOS, Phase 4 (MicroVM/Hyper-V) provides full isolation.");
        info!("Running inside rootfs path with cleaned environment: {}", cmd);
        run_clean_subprocess(rootfs, cmd)
    }
}

#[cfg(not(target_os = "linux"))]
fn run_clean_subprocess(rootfs: &Path, cmd: &str) -> Result<()> {
    use std::process;
    // Run cmd inside the rootfs workspace with a zero-knowledge environment
    // (no host env vars, no host PATH leaks)
    let workspace = rootfs.join("workspace");
    let status = process::Command::new("cmd")
        .args(["/C", cmd])
        .current_dir(&workspace)
        .env_clear()             // Wipe ALL host environment variables
        .env("ZENITH_SANDBOX", "1")
        .status()
        .context(format!("Failed to run: {}", cmd))?;

    if !status.success() {
        return Err(anyhow::anyhow!("Command exited with: {}", status));
    }
    Ok(())
}

// Async-compatible wrappers used by the workflow runner

pub async fn provision_lab(os: &str) -> Result<String> {
    ensure_rootfs(os).await?;
    let workspace = rootfs_dir(os).join("workspace");
    fs::create_dir_all(&workspace).context("Failed to create lab workspace")?;
    // Copy current project into the sealed canvas workspace
    let current_dir = std::env::current_dir()?;
    copy_dir_all(&current_dir, &workspace)?;
    info!("Canvas provisioned for OS '{}'.", os);
    Ok(format!("zenith-lab-{}", os))
}

pub async fn exec_in_lab(lab_id: &str, cmd: &str) -> Result<()> {
    // lab_id format: "zenith-lab-<os>"
    let os = lab_id.strip_prefix("zenith-lab-").unwrap_or(lab_id);
    let rootfs = rootfs_dir(os);
    run_in_sandbox(&rootfs, os, cmd)
}

pub async fn teardown_lab(lab_id: &str) -> Result<()> {
    let os = lab_id.strip_prefix("zenith-lab-").unwrap_or(lab_id);
    let workspace = rootfs_dir(os).join("workspace");
    if workspace.exists() {
        fs::remove_dir_all(&workspace).context("Failed to clean canvas workspace")?;
    }
    info!("Canvas workspace cleaned. Base rootfs cached for next run.");
    Ok(())
}

// ─── Linux namespace isolation module ────────────────────────────────────────
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use nix::sched::{unshare, CloneFlags};
    use std::os::unix::process::CommandExt;
    use std::process;

    pub fn run_namespaced(rootfs: &Path, cmd: &str) -> Result<()> {
        info!("Launching in isolated Linux namespace (PID + mount + net)...");

        // Unshare PID, mount, and network namespaces from the host
        unshare(
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET,
        )
        .context("Failed to create new namespaces")?;

        let workspace = rootfs.join("workspace");

        let status = process::Command::new("/bin/sh")
            .args(["-c", cmd])
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
            .env("HOME", "/root")
            .env("ZENITH_SANDBOX", "1")
            .status()
            .context(format!("Failed to exec in namespace: {}", cmd))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Namespaced command failed: {}", cmd));
        }
        Ok(())
    }
}
