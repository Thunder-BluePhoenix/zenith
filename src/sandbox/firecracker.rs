/// Firecracker MicroVM Backend — Phase 4 Complete Implementation
///
/// Motto: "You install Zenith. Zenith installs everything else."
///
/// What Zenith manages automatically (no user installs):
///   ~/.zenith/bin/firecracker     — Firecracker VMM binary (from AWS GitHub)
///   ~/.zenith/kernel/vmlinux      — Linux kernel for Firecracker (from AWS S3)
///   ~/.zenith/rootfs-fc/<os>.ext4 — ext4 disk image for the VM (from AWS S3)
///
/// Execution model (Phase 4):
///   The workflow step command is embedded in the Linux kernel boot cmdline
///   as `init=/bin/sh -c "<command>"`. The VM boots, runs the command as
///   PID 1, and exits — output streams via the serial console (ttyS0 = Firecracker stdout).
///   This is clean, requires no custom init binary, and works with any standard rootfs.
///
/// Requires: Linux host with /dev/kvm enabled.
/// Windows/macOS: use 'backend: container' or 'backend: wasm'.

use super::backend::Backend;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use async_trait::async_trait;
use tracing::{info, warn, error, debug};

// ─── Warm-pool ────────────────────────────────────────────────────────────────

/// Per-OS snapshot directory cache.
/// After a cold boot with zenith-init, a VM snapshot is saved here.
/// Subsequent runs restore from the snapshot instead of cold-booting — typically
/// < 50ms vs 500ms–2s for a cold boot.
#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref WARM_POOL: std::sync::Mutex<HashMap<String, PathBuf>> =
        std::sync::Mutex::new(HashMap::new());
}

pub struct FirecrackerBackend;

#[async_trait]
impl Backend for FirecrackerBackend {
    fn name(&self) -> &str { "firecracker" }

    async fn provision(&self, lab_id: &str, base_os: &str, target_arch: &str) -> Result<()> {
        info!("[FC] Provisioning MicroVM — lab: {}, OS: {}, arch: {}", lab_id, base_os, target_arch);

        #[cfg(target_os = "linux")]
        {
            check_kvm()?;

            // Motto: Zenith downloads everything — firecracker binary, kernel, rootfs
            let _fc     = crate::tools::ensure_firecracker().await?;
            let _kernel = crate::tools::ensure_fc_kernel().await?;
            let _rootfs = crate::tools::ensure_fc_rootfs(base_os).await?;

            // Set up per-lab temp directory
            let lab_dir = super::lab_state_dir(lab_id);
            std::fs::create_dir_all(&lab_dir).context("Failed to create lab dir")?;

            // Copy the user's project into the lab dir so we can reference it
            let workspace = lab_dir.join("workspace");
            std::fs::create_dir_all(&workspace)?;
            let current = std::env::current_dir()?;
            super::copy_dir_all(&current, &workspace)?;

            info!("[FC] MicroVM resources ready for lab '{}'.", lab_id);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(anyhow::anyhow!(
                "Firecracker/KVM requires a Linux host.\n\
                 On Windows/macOS: use 'backend: container' or 'backend: wasm' instead."
            ))
        }
    }

    async fn execute(
        &self,
        lab_id: &str,
        base_os: &str,
        _target_arch: &str,
        cmd: &str,
        env: Option<HashMap<String, String>>,
        working_directory: Option<String>,
    ) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            info!("[FC] Booting MicroVM for lab '{}', running: {}", lab_id, cmd);

            let fc_bin = crate::tools::ensure_firecracker().await?;

            // Phase 12: prefer the Zenith custom kernel when it exists.
            let zenith_kernel = crate::sandbox::zenith_home().join("kernel").join("vmlinux-zenith");
            let kernel = if zenith_kernel.exists() {
                info!("[FC] Using Zenith custom kernel: {:?}", zenith_kernel);
                zenith_kernel
            } else {
                crate::tools::ensure_fc_kernel().await?
            };

            // Phase 12: zenith-init protocol when using the Zenith minimal rootfs.
            // The minimal rootfs has /zenith-init as its PID 1. The host writes the
            // command to FC's stdin (which maps to ttyS0/the serial console in the VM).
            // This is cleaner and more secure than embedding the command in the boot cmdline.
            let zenith_rootfs = crate::sandbox::zenith_home().join("rootfs").join("zenith-minimal.tar.gz");
            let use_zenith_init = base_os == "zenith" && zenith_rootfs.exists();

            let rootfs_src = if use_zenith_init {
                info!("[FC] Using Zenith minimal rootfs with zenith-init");
                zenith_rootfs
            } else {
                crate::tools::ensure_fc_rootfs(base_os).await?
            };

            let lab_dir = super::lab_state_dir(lab_id);
            let rootfs_snap = lab_dir.join("rootfs.ext4");
            std::fs::copy(&rootfs_src, &rootfs_snap)
                .context("Failed to create rootfs snapshot")?;

            let socket_path = lab_dir.join("api.sock");
            let _ = std::fs::remove_file(&socket_path);

            // ── Phase 12: Warm-pool path ────────────────────────────────────
            // If a snapshot of this OS already exists (from a previous cold boot),
            // restore it instead of cold-booting — typically < 50ms startup.
            let snap_base = crate::sandbox::zenith_home().join("snapshots").join(base_os);
            let has_warm = use_zenith_init && {
                WARM_POOL.lock().unwrap().contains_key(base_os)
            };

            let mut fc_process = if has_warm {
                info!("[FC] Restoring from warm snapshot for '{}'", base_os);
                // Clone snapshot files to a per-run dir (preserves the master snapshot)
                let snap_run = lab_dir.join("snap");
                std::fs::create_dir_all(&snap_run)?;
                std::fs::copy(snap_base.join("mem.snap"),   snap_run.join("mem.snap"))?;
                std::fs::copy(snap_base.join("state.snap"), snap_run.join("state.snap"))?;

                let proc = restore_vm_snapshot_piped(&fc_bin, &socket_path, &snap_run)?;
                wait_for_socket(&socket_path, Duration::from_secs(2))?;
                fc_resume_vm(&socket_path)?;
                proc
            } else {
                // ── Cold boot ───────────────────────────────────────────────
                let (boot_args, stdin_mode) = if use_zenith_init {
                    // zenith-init reads the command from stdin — no cmdline embedding
                    let args = "console=ttyS0 reboot=k panic=1 pci=off nomodule \
                                init=/zenith-init".to_string();
                    (args, std::process::Stdio::piped())
                } else {
                    // Legacy: embed command in boot cmdline
                    let env_prefix = build_env_prefix(&env);
                    let wd_cd = working_directory.as_deref()
                        .map(|d| format!("cd {} && ", shell_escape(d)))
                        .unwrap_or_default();
                    let full_cmd = format!(
                        "{}{}{}; echo __ZENITH_EXIT__:$?; poweroff -f",
                        env_prefix, wd_cd, cmd
                    );
                    let args = format!(
                        "console=ttyS0 reboot=k panic=1 pci=off nomodule \
                         init=/bin/sh -- -c \"{}\"",
                        full_cmd.replace('"', "\\\"")
                    );
                    (args, std::process::Stdio::null())
                };

                let proc = std::process::Command::new(&fc_bin)
                    .arg("--api-sock").arg(&socket_path)
                    .arg("--log-level").arg("Warning")
                    .stdin(stdin_mode)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .context("Failed to launch Firecracker process")?;

                wait_for_socket(&socket_path, Duration::from_secs(3))?;
                fc_configure_vm(&socket_path, &kernel, &rootfs_snap, &boot_args, lab_id)?;
                fc_start_vm(&socket_path)?;
                info!("[FC] MicroVM booted.");

                if use_zenith_init {
                    // Give zenith-init time to mount filesystems and block on read().
                    std::thread::sleep(Duration::from_millis(120));
                    // Pause + snapshot so subsequent runs can skip the cold boot.
                    if let Err(e) = fc_api(&socket_path, "PATCH", "/vm", r#"{"state":"Paused"}"#) {
                        warn!("[FC] Could not pause VM for snapshot: {}", e);
                    } else {
                        match create_vm_snapshot(&socket_path, &snap_base) {
                            Ok(()) => {
                                WARM_POOL.lock().unwrap()
                                    .insert(base_os.to_string(), snap_base.clone());
                                info!("[FC] Warm snapshot saved for '{}'", base_os);
                            }
                            Err(e) => warn!("[FC] Snapshot failed (non-fatal): {}", e),
                        }
                        if let Err(e) = fc_resume_vm(&socket_path) {
                            warn!("[FC] Could not resume VM after snapshot: {}", e);
                        }
                    }
                }

                proc
            };

            // ── Send command to zenith-init via stdin ───────────────────────
            // (Both warm-restore and cold-boot paths use the same dispatch.)
            if use_zenith_init {
                if let Some(mut stdin_pipe) = fc_process.stdin.take() {
                    let env_prefix = build_env_prefix(&env);
                    let wd_cd = working_directory.as_deref()
                        .map(|d| format!("cd {} && ", shell_escape(d)))
                        .unwrap_or_default();
                    let full_cmd = format!("{}{}{}", env_prefix, wd_cd, cmd);
                    if let Err(e) = writeln!(stdin_pipe, "{}", full_cmd) {
                        error!("[FC] Failed to send command to zenith-init: {}", e);
                    }
                    // Closing stdin signals EOF; zenith-init stops reading.
                }
            }

            // ── Collect output ──────────────────────────────────────────────
            let stdout = fc_process.stdout.take().expect("Firecracker stdout pipe missing");
            info!("[FC] Streaming serial console output...");
            let exit_code = read_serial_output(stdout);
            let _ = fc_process.wait();

            match exit_code {
                Ok(0) => {
                    info!("[FC] MicroVM command completed successfully.");
                    Ok(())
                }
                Ok(code) => Err(anyhow::anyhow!(
                    "MicroVM command exited with code {}: {}", code, cmd
                )),
                Err(e) => Err(e),
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(anyhow::anyhow!("Firecracker requires Linux + KVM."))
        }
    }

    async fn teardown(&self, lab_id: &str) -> Result<()> {
        info!("[FC] Tearing down MicroVM session '{}'.", lab_id);
        super::teardown_lab(lab_id).await
    }
}

// ─── Firecracker REST API helpers ────────────────────────────────────────────

/// Minimal synchronous HTTP/1.1 client over a Unix domain socket.
/// Firecracker's API is simple enough that we don't need a full HTTP library.
#[cfg(target_os = "linux")]
fn fc_api(socket: &Path, method: &str, path: &str, body: &str) -> Result<()> {
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket)
        .with_context(|| format!("Cannot connect to Firecracker API socket {:?}", socket))?;

    let request = format!(
        "{} {} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Accept: application/json\r\n\
         \r\n\
         {}",
        method, path, body.len(), body
    );

    stream.write_all(request.as_bytes()).context("FC API write failed")?;

    // Read the status line to detect errors
    let mut response = String::new();
    let mut reader = BufReader::new(&stream);
    reader.read_line(&mut response).context("FC API read failed")?;

    // 2xx = success, anything else = error
    let status: u16 = response
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    if status < 200 || status >= 300 {
        // Read body for error detail
        let mut body_resp = String::new();
        let _ = reader.read_line(&mut body_resp);
        return Err(anyhow::anyhow!(
            "Firecracker API {} {} returned HTTP {}: {}",
            method, path, status, body_resp.trim()
        ));
    }

    debug!("[FC] API {} {} → HTTP {}", method, path, status);
    Ok(())
}

/// Configure the VM: boot source (kernel + boot args), root drive, machine config.
#[cfg(target_os = "linux")]
fn fc_configure_vm(
    socket: &Path,
    kernel: &Path,
    rootfs: &Path,
    boot_args: &str,
    _lab_id: &str,
) -> Result<()> {
    // 1 — Boot source
    let boot_body = format!(
        r#"{{"kernel_image_path":"{}","boot_args":"{}"}}"#,
        kernel.display(),
        boot_args.replace('"', "\\\"")
    );
    fc_api(socket, "PUT", "/boot-source", &boot_body)
        .context("Failed to configure Firecracker boot source")?;

    // 2 — Root drive (read-write snapshot)
    let drive_body = format!(
        r#"{{"drive_id":"rootfs","path_on_host":"{}","is_root_device":true,"is_read_only":false}}"#,
        rootfs.display()
    );
    fc_api(socket, "PUT", "/drives/rootfs", &drive_body)
        .context("Failed to configure Firecracker root drive")?;

    // 3 — Machine configuration (1 vCPU, 128 MiB RAM — minimal for CI steps)
    fc_api(socket, "PUT", "/machine-config",
        r#"{"vcpu_count":1,"mem_size_mib":128}"#)
        .context("Failed to configure Firecracker machine")?;

    Ok(())
}

/// Send InstanceStart action to boot the VM.
#[cfg(target_os = "linux")]
fn fc_start_vm(socket: &Path) -> Result<()> {
    fc_api(socket, "PUT", "/actions", r#"{"action_type":"InstanceStart"}"#)
        .context("Failed to start Firecracker VM")?;
    Ok(())
}

/// Block until the Unix socket file appears (Firecracker ready) or timeout.
#[cfg(target_os = "linux")]
fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<()> {
    let start = std::time::Instant::now();
    while !socket.exists() {
        if start.elapsed() >= timeout {
            return Err(anyhow::anyhow!(
                "Firecracker API socket did not appear within {:?}. \
                 Check that /dev/kvm is accessible and Firecracker started correctly.",
                timeout
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Read lines from the VM's serial console (Firecracker stdout).
///
/// Supports two protocols:
///
/// **Legacy (Phase 4)**: any line prefixed `__ZENITH_EXIT__:<code>` terminates.
///
/// **zenith-init (Phase 12)**: lines are prefixed with:
///   `O:<text>`   — stdout from the step command  → forward to host stdout
///   `E:<text>`   — stderr from the step command  → forward to host stderr
///   `EXIT:<code>` — step exit code               → return value
///
/// All other lines (kernel boot messages, init messages) are forwarded to
/// the host terminal unchanged for debugging visibility.
#[cfg(target_os = "linux")]
fn read_serial_output(stdout: std::process::ChildStdout) -> Result<i32> {
    use std::io::Write as _;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.context("Error reading serial console")?;

        // Phase 12: zenith-init protocol
        if let Some(rest) = line.strip_prefix("EXIT:") {
            let code: i32 = rest.trim().parse().unwrap_or(1);
            return Ok(code);
        }
        if let Some(rest) = line.strip_prefix("O:") {
            println!("{}", rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("E:") {
            eprintln!("{}", rest);
            continue;
        }

        // Phase 4 legacy: __ZENITH_EXIT__:<code>
        if let Some(rest) = line.strip_prefix("__ZENITH_EXIT__:") {
            let code: i32 = rest.trim().parse().unwrap_or(1);
            return Ok(code);
        }

        // Kernel / boot messages — show them for debugging
        debug!("[FC serial] {}", line);
    }
    Ok(0)
}

// ─── Phase 12: VM snapshot / restore ─────────────────────────────────────────

/// Snapshot a running Firecracker VM's memory + state to disk.
///
/// Uses the Firecracker REST API `CreateSnapshot` action.
/// The resulting files can be passed to `restore_vm_snapshot()` to resume
/// the VM in < 1ms instead of cold-booting.
///
/// Snapshot files written:
///   `<snap_dir>/mem.snap`   — guest RAM dump
///   `<snap_dir>/state.snap` — VM device + CPU state
#[cfg(target_os = "linux")]
pub fn create_vm_snapshot(socket: &Path, snap_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(snap_dir)
        .context("Cannot create snapshot directory")?;

    let mem_path   = snap_dir.join("mem.snap");
    let state_path = snap_dir.join("state.snap");

    let body = format!(
        r#"{{"snapshot_type":"Full","snapshot_path":"{}","mem_file_path":"{}"}}"#,
        state_path.display(),
        mem_path.display(),
    );

    fc_api(socket, "PUT", "/snapshot/create", &body)
        .context("Failed to create Firecracker VM snapshot")?;

    info!("[FC] Snapshot saved to {:?}", snap_dir);
    Ok(())
}

/// Restore a Firecracker VM from a snapshot created by `create_vm_snapshot()`.
///
/// The restored VM is paused immediately after restore; call `fc_resume_vm()`
/// to let it continue executing. This pattern allows a "warm pool" of pre-booted
/// VMs to be maintained and assigned to incoming workflow steps on demand.
#[cfg(target_os = "linux")]
pub fn restore_vm_snapshot(fc_bin: &Path, socket: &Path, snap_dir: &Path) -> Result<std::process::Child> {
    let mem_path   = snap_dir.join("mem.snap");
    let state_path = snap_dir.join("state.snap");

    if !mem_path.exists() || !state_path.exists() {
        return Err(anyhow::anyhow!(
            "Snapshot files not found in {:?}. Run create_vm_snapshot() first.", snap_dir
        ));
    }

    // Launch Firecracker in restore mode
    let child = std::process::Command::new(fc_bin)
        .args([
            "--api-sock", &socket.to_string_lossy(),
            "--config-file", "/dev/null",
            "--restore-snapshot",
            &format!(
                r#"{{"snapshot_path":"{}","mem_file_path":"{}","enable_diff_snapshots":false}}"#,
                state_path.display(), mem_path.display()
            ),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to launch Firecracker in restore mode")?;

    info!("[FC] VM restored from snapshot at {:?}", snap_dir);
    Ok(child)
}

/// Like `restore_vm_snapshot` but with stdin piped so the host can send the
/// step command to zenith-init over the serial console.
#[cfg(target_os = "linux")]
fn restore_vm_snapshot_piped(
    fc_bin: &Path,
    socket: &Path,
    snap_dir: &Path,
) -> Result<std::process::Child> {
    let mem_path   = snap_dir.join("mem.snap");
    let state_path = snap_dir.join("state.snap");

    if !mem_path.exists() || !state_path.exists() {
        return Err(anyhow::anyhow!(
            "Snapshot files not found in {:?}. Warm pool miss — falling back to cold boot.",
            snap_dir
        ));
    }

    let child = std::process::Command::new(fc_bin)
        .args([
            "--api-sock", &socket.to_string_lossy(),
            "--config-file", "/dev/null",
            "--restore-snapshot",
            &format!(
                r#"{{"snapshot_path":"{}","mem_file_path":"{}","enable_diff_snapshots":false}}"#,
                state_path.display(), mem_path.display()
            ),
        ])
        .stdin(std::process::Stdio::piped())   // so we can send the command
        .stdout(std::process::Stdio::piped())  // serial console output
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to launch Firecracker in restore mode")?;

    info!("[FC] VM restored from snapshot at {:?}", snap_dir);
    Ok(child)
}

/// Resume a paused (restored) Firecracker VM so it continues execution.
#[cfg(target_os = "linux")]
pub fn fc_resume_vm(socket: &Path) -> Result<()> {
    fc_api(socket, "PATCH", "/vm", r#"{"state":"Resumed"}"#)
        .context("Failed to resume Firecracker VM")?;
    Ok(())
}

// ─── KVM check ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn check_kvm() -> Result<()> {
    if !Path::new("/dev/kvm").exists() {
        return Err(anyhow::anyhow!(
            "/dev/kvm not found. Firecracker requires KVM.\n\
             Check:\n\
             1. CPU virtualization enabled in BIOS (Intel VT-x / AMD-V)\n\
             2. KVM module loaded: sudo modprobe kvm_intel  (or kvm_amd)\n\
             3. /dev/kvm is readable: sudo chmod 666 /dev/kvm\n\n\
             Use 'backend: container' in .zenith.yml for namespace isolation instead."
        ));
    }
    Ok(())
}

// ─── Command helpers ──────────────────────────────────────────────────────────

/// Build `KEY=VALUE KEY2=VALUE2 ` prefix for the shell command inside the VM.
fn build_env_prefix(env: &Option<HashMap<String, String>>) -> String {
    match env {
        None => String::new(),
        Some(map) if map.is_empty() => String::new(),
        Some(map) => {
            let mut parts: Vec<String> = map.iter()
                .map(|(k, v)| format!("{}={}", k, shell_escape(v)))
                .collect();
            parts.sort(); // deterministic
            parts.push(String::new()); // trailing space
            parts.join(" ")
        }
    }
}

/// Minimal shell escaping: wrap in single quotes, escape embedded single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
