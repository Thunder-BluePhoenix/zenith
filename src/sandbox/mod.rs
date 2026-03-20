/// Zenith Native Sandbox — zero external tool dependencies.
///
/// Motto: "You install Zenith. Zenith installs everything else."
///
/// How it works:
///   1. Zenith downloads a minimal Alpine Linux rootfs tarball (~3MB) from
///      the official Alpine CDN directly using its built-in HTTP client.
///   2. The tarball is extracted into ~/.zenith/rootfs/<os>/
///   3. On Linux: the subprocess is launched inside a new PID + user + mount
///      namespace using the `nix` crate (raw Linux syscalls, no Docker).
///   4. Cross-arch (e.g. aarch64 on x86_64): Zenith downloads qemu-user-static
///      from multiarch GitHub releases into ~/.zenith/bin/ automatically.
///   5. On Windows/macOS: a restricted subprocess is used with a completely
///      cleaned environment. Use the 'wasm' backend for cross-platform isolation.

use crate::cli::LabCommands;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn, debug};

pub mod backend;
pub mod container;
pub mod firecracker;
pub mod wasm;
pub mod wine;
pub mod cache;

use backend::Backend;
use container::ContainerBackend;
use firecracker::FirecrackerBackend;
use wasm::WasmBackend;
use wine::WineBackend;

/// Factory to get the requested isolation engine.
/// "You install Zenith. Zenith installs everything else."
pub fn get_backend(name: &str) -> Box<dyn Backend> {
    match name {
        "firecracker" | "fc" => Box::new(FirecrackerBackend),
        "wasm"               => Box::new(WasmBackend),
        "wine"               => Box::new(WineBackend),
        _                    => Box::new(ContainerBackend),
    }
}

lazy_static::lazy_static! {
    static ref DOWNLOAD_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
}

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
    RootfsSource {
        os: "alpine-arm64",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/aarch64/alpine-minirootfs-3.19.1-aarch64.tar.gz",
    },
];

pub fn zenith_home() -> PathBuf {
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
    let _lock = DOWNLOAD_MUTEX.lock().await;
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
            let arch = std::env::consts::ARCH;
            run_in_sandbox(&rootfs, &os, arch, &command, None, None)?;
        }

        LabCommands::Shell { os } => {
            let rootfs = ensure_rootfs(&os).await?;
            let arch = std::env::consts::ARCH;
            run_in_sandbox(&rootfs, &os, arch, "/bin/sh", None, None)?;
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
pub fn run_in_sandbox(
    rootfs: &Path,
    os: &str,
    target_arch: &str,
    cmd: &str,
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    let workspace = rootfs.join("workspace");
    // Direct lab commands (zenith lab run) have no pre-downloaded qemu path;
    // for cross-arch from CLI, the user should use the container backend via zenith run.
    run_in_sandbox_with_workspace(rootfs, &workspace, os, target_arch, None, cmd, env, working_directory)
}

/// Execute a command inside the sandbox environment with an explicit workspace path.
/// `qemu_binary`: path to a qemu-user-static binary downloaded by Zenith's tool
/// manager (tools.rs). Pass `None` for native-arch execution.
pub fn run_in_sandbox_with_workspace(
    _rootfs: &Path,
    workspace: &Path,
    _os: &str,
    target_arch: &str,
    qemu_binary: Option<&Path>,
    cmd: &str,
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    let host_arch = std::env::consts::ARCH;
    let is_emulated = target_arch != host_arch
        && target_arch != "x86_64"
        && target_arch != "amd64"
        && target_arch != "native";

    #[cfg(target_os = "linux")]
    {
        if is_emulated && qemu_binary.is_some() {
            info!("Cross-arch: host={} target={} — using QEMU user-mode emulation.", host_arch, target_arch);
        } else if is_emulated {
            warn!("Cross-arch target={} but no QEMU binary available. Running natively (may fail).", target_arch);
        }
        linux::run_namespaced(workspace, qemu_binary, cmd, env, working_directory)
    }

    #[cfg(not(target_os = "linux"))]
    {
        if is_emulated {
            warn!("Cross-arch emulation requires Linux + QEMU. Running natively on {}.", std::env::consts::OS);
        }
        warn!("Full kernel isolation is Linux-only. Running in cleaned workspace subprocess.");
        run_clean_subprocess_with_workspace(workspace, cmd, env, working_directory)
    }
}

#[cfg(not(target_os = "linux"))]
fn run_clean_subprocess_with_workspace(
    workspace: &Path, 
    cmd: &str, 
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    use std::process;
    // Run cmd inside the rootfs workspace with a zero-knowledge environment
    // (no host env vars, no host PATH leaks)
    let mut base_workspace = workspace.to_path_buf();
    if let Some(wd) = working_directory {
        base_workspace = base_workspace.join(wd);
    }
    
    let mut command = process::Command::new("cmd");
    command.args(["/C", cmd])
        .current_dir(&base_workspace)
        .env_clear()             // Wipe ALL host environment variables
        .env("ZENITH_SANDBOX", "1");

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            command.env(k, v);
        }
    }

    let status = command.status()
        .context(format!("Failed to run: {}", cmd))?;

    if !status.success() {
        return Err(anyhow::anyhow!("Command exited with: {}", status));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn run_clean_subprocess(
    rootfs: &Path, 
    cmd: &str, 
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    let workspace = rootfs.join("workspace");
    run_clean_subprocess_with_workspace(&workspace, cmd, env, working_directory)
}

// Async-compatible wrappers used by the workflow runner

/// Phase 1 — provision a lab using OverlayFS on Linux, workspace-copy elsewhere.
/// OverlayFS gives true copy-on-write isolation: writes inside the sandbox never
/// touch the cached base rootfs; only the per-lab upper layer accumulates changes.
pub async fn provision_lab(lab_id: &str, base_os: &str) -> Result<()> {
    ensure_rootfs(base_os).await?;

    #[cfg(target_os = "linux")]
    {
        let rootfs = rootfs_dir(base_os);
        let lab_dir = lab_state_dir(lab_id);
        let upper   = lab_dir.join("upper");
        let work    = lab_dir.join("work");
        let merged  = lab_dir.join("merged");

        fs::create_dir_all(&upper).context("Failed to create overlay upper dir")?;
        fs::create_dir_all(&work).context("Failed to create overlay work dir")?;
        fs::create_dir_all(&merged).context("Failed to create overlay merged dir")?;

        match linux::mount_overlay(&rootfs, &upper, &work, &merged) {
            Ok(()) => {
                // Bind-mount the project into /workspace inside the overlay
                let workspace = merged.join("workspace");
                fs::create_dir_all(&workspace).context("Failed to create workspace dir in overlay")?;
                let current_dir = std::env::current_dir()?;
                copy_dir_all(&current_dir, &workspace)?;
                info!("OverlayFS lab '{}' ready (lower=rootfs, upper=writes-only).", lab_id);
                return Ok(());
            }
            Err(e) => {
                warn!("OverlayFS unavailable ({}). Falling back to workspace-copy isolation.", e);
                // Fall through to the copy-based approach below
            }
        }
    }

    // Fallback: copy-based workspace (Windows, macOS, or no overlayfs privilege)
    let workspace = lab_state_dir(lab_id).join("workspace");
    fs::create_dir_all(&workspace).context("Failed to create lab workspace")?;
    let current_dir = std::env::current_dir()?;
    copy_dir_all(&current_dir, &workspace)?;
    info!("Workspace-copy lab '{}' provisioned (OS: {}).", lab_id, base_os);
    Ok(())
}

pub async fn exec_in_lab(
    lab_id: &str,
    base_os: &str,
    target_arch: &str,
    cmd: &str,
    env: Option<HashMap<String, String>>,
    working_directory: Option<String>
) -> Result<()> {
    let rootfs = rootfs_dir(base_os);
    let lab_workspace = lab_state_dir(lab_id).join("workspace");

    // Phase 5 (Motto): If cross-arch is needed, Zenith auto-downloads
    // qemu-user-static — the user installs nothing.
    let host_arch = std::env::consts::ARCH;
    let needs_qemu = target_arch != host_arch
        && target_arch != "x86_64"
        && target_arch != "amd64"
        && target_arch != "native";

    #[cfg(target_os = "linux")]
    let qemu_binary: Option<PathBuf> = if needs_qemu {
        match crate::tools::ensure_qemu_for_arch(target_arch).await {
            Ok(p) => {
                info!("QEMU user-mode ready for arch '{}': {:?}", target_arch, p);
                // Phase 5: also try to register binfmt_misc for transparent emulation.
                // Failure is non-fatal — explicit prefix in run_namespaced() still works.
                if let Err(e) = linux::register_binfmt_qemu(target_arch, &p) {
                    debug!("binfmt_misc registration skipped ({}). Using explicit QEMU prefix.", e);
                }
                Some(p)
            }
            Err(e) => {
                warn!("QEMU download failed for arch '{}': {}. Running without emulation.", target_arch, e);
                None
            }
        }
    } else {
        None
    };

    #[cfg(not(target_os = "linux"))]
    let qemu_binary: Option<PathBuf> = {
        if needs_qemu {
            warn!("Cross-arch emulation requires Linux. Running natively on this platform.");
        }
        None
    };

    // On Linux: if an OverlayFS merged dir exists for this lab, use it.
    // Otherwise fall back to the workspace-copy path.
    let lab_dir = lab_state_dir(lab_id);
    let overlay_merged = lab_dir.join("merged");
    let workspace = if overlay_merged.exists() {
        overlay_merged.join("workspace")
    } else {
        lab_dir.join("workspace")
    };

    run_in_sandbox_with_workspace(&rootfs, &workspace, base_os, target_arch, qemu_binary.as_deref(), cmd, env, working_directory)
}

pub async fn teardown_lab(lab_id: &str) -> Result<()> {
    let state_dir = lab_state_dir(lab_id);
    if !state_dir.exists() {
        return Ok(());
    }

    // If OverlayFS was mounted, unmount it first before deleting the directory
    #[cfg(target_os = "linux")]
    {
        let merged = state_dir.join("merged");
        if merged.exists() {
            if let Err(e) = linux::unmount_overlay(&merged) {
                warn!("Failed to unmount overlay at {:?}: {}. Continuing teardown.", merged, e);
            }
        }
    }

    fs::remove_dir_all(&state_dir).context("Failed to clean lab session")?;
    debug!("Lab session '{}' cleaned.", lab_id);
    Ok(())
}

// ─── Linux namespace isolation + overlay module ───────────────────────────────
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use nix::mount::{mount, umount2, MsFlags, MntFlags};
    use nix::sched::{unshare, CloneFlags};
    use std::process;

    /// Mount an OverlayFS:
    ///   lower = read-only base rootfs (Alpine, Ubuntu, …)
    ///   upper = per-lab writable layer (only changes go here)
    ///   work  = required temp dir for overlayfs kernel internals
    ///   merged = the unified view presented to the process
    ///
    /// Requires either root CAP_SYS_ADMIN or a kernel ≥ 5.11 with
    /// user-namespace overlayfs support (CONFIG_OVERLAY_FS_METACOPY=y).
    pub fn mount_overlay(lower: &Path, upper: &Path, work: &Path, merged: &Path) -> Result<()> {
        let opts = format!(
            "lowerdir={},upperdir={},workdir={}",
            lower.display(), upper.display(), work.display()
        );
        mount(
            Some("overlay"),
            merged,
            Some("overlay"),
            MsFlags::empty(),
            Some(opts.as_str()),
        )
        .context(
            "overlayfs mount failed. This requires either root or a kernel ≥5.11 \
             with user-namespace overlay support. Zenith will fall back to \
             workspace-copy isolation automatically."
        )?;
        Ok(())
    }

    /// Lazily unmount the overlay (MNT_DETACH so it unmounts even if busy).
    pub fn unmount_overlay(merged: &Path) -> Result<()> {
        umount2(merged, MntFlags::MNT_DETACH)
            .context("Failed to unmount overlayfs")?;
        Ok(())
    }

    /// Phase 5: Register a qemu-user-static binary with the kernel's binfmt_misc.
    ///
    /// After registration, the kernel transparently invokes the QEMU emulator for
    /// any foreign-arch ELF binary — no explicit `qemu-arch-static ./binary` needed.
    ///
    /// Requires write access to /proc/sys/fs/binfmt_misc/register (usually root).
    /// Falls back gracefully: if binfmt_misc isn't available, the explicit-prefix
    /// approach in run_namespaced() still works.
    pub fn register_binfmt_qemu(target_arch: &str, qemu_path: &Path) -> Result<()> {
        let register_path = Path::new("/proc/sys/fs/binfmt_misc/register");
        if !register_path.exists() {
            return Err(anyhow::anyhow!(
                "/proc/sys/fs/binfmt_misc/register not found. \
                 Mount binfmt_misc: mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc"
            ));
        }

        // binfmt_misc registration format:
        //   :name:type:offset:magic:mask:interpreter:flags
        // We register by ELF magic bytes for each architecture.
        // F flag = fix binary (allows use across mount namespaces).
        let (name, magic, mask) = match target_arch {
            "aarch64" | "arm64" => (
                "qemu-aarch64",
                r"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\xb7\x00",
                r"\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff",
            ),
            "arm" | "armv7" => (
                "qemu-arm",
                r"\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\x28\x00",
                r"\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff",
            ),
            "riscv64" => (
                "qemu-riscv64",
                r"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\xf3\x00",
                r"\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff",
            ),
            other => return Err(anyhow::anyhow!(
                "No binfmt_misc magic defined for arch '{}'. \
                 Supported: aarch64, arm, riscv64", other
            )),
        };

        let registration = format!(
            ":{name}:M:0:{magic}:{mask}:{interpreter}:F",
            name = name,
            magic = magic,
            mask = mask,
            interpreter = qemu_path.display(),
        );

        std::fs::write(register_path, registration.as_bytes())
            .with_context(|| format!(
                "Failed to register binfmt_misc for {}. \
                 This requires root or CAP_SYS_ADMIN. \
                 Zenith will still emulate via explicit QEMU prefix.", target_arch
            ))?;

        info!("binfmt_misc: registered {} → {:?}", target_arch, qemu_path);
        Ok(())
    }

    /// Run `cmd` inside new PID + mount + network namespaces.
    ///
    /// If `qemu_binary` is provided (cross-arch), the command is wrapped as:
    ///   qemu-<arch>-static /bin/sh -c "<cmd>"
    /// so that foreign-arch binaries inside the workspace execute transparently.
    pub fn run_namespaced(
        workspace: &Path,
        qemu_binary: Option<&Path>,
        cmd: &str,
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>
    ) -> Result<()> {
        info!("Launching in isolated Linux namespace (PID + mount + net)...");

        unshare(
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET,
        )
        .context("Failed to create new namespaces. Tip: some environments (WSL1, restricted containers) do not allow unshare.")?;

        let mut base_workspace = workspace.to_path_buf();
        if let Some(wd) = working_directory {
            base_workspace = base_workspace.join(wd);
        }

        let mut command = if let Some(qemu) = qemu_binary {
            // Cross-arch: use qemu-user-static to transparently emulate the binary
            let mut c = process::Command::new(qemu);
            c.arg("/bin/sh").arg("-c").arg(cmd);
            c
        } else {
            // Native arch: run directly
            let mut c = process::Command::new("/bin/sh");
            c.arg("-c").arg(cmd);
            c
        };

        command
            .current_dir(&base_workspace)
            .env_clear()
            .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
            .env("HOME", "/root")
            .env("ZENITH_SANDBOX", "1");

        if let Some(env_vars) = env {
            for (k, v) in env_vars {
                command.env(k, v);
            }
        }

        let status = command
            .status()
            .context(format!("Failed to exec in namespace: {}", cmd))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Namespaced command failed with status {}: {}", status, cmd));
        }
        Ok(())
    }
}
