/// zenith-init — minimal PID 1 for Zenith Firecracker VMs (Phase 12).
///
/// Compiled as a statically-linked musl binary for embedding in the Zenith rootfs.
/// When the VM boots with this as PID 1:
///
///   1. Mount essential pseudo-filesystems (/proc, /sys, /dev)
///   2. Open the virtio-serial console (or vsock) to receive the step command
///   3. exec() the received command, forwarding stdout/stderr back over the channel
///   4. Write the exit code to the channel
///   5. Trigger a clean VM shutdown
///
/// Communication protocol over virtio-serial (/dev/hvc1 or /dev/ttyS1):
///   Host → Guest:  "<shell-command>\n"
///   Guest → Host:  stdout/stderr lines, each prefixed with "O:" or "E:"
///   Guest → Host:  "EXIT:<code>\n" when the command finishes
///   Guest → Host:  (powers off)
///
/// Build for embedding:
///   cargo build --bin zenith-init --target x86_64-unknown-linux-musl --release
///
/// NOT for use on the host machine. The [[bin]] target is compiled separately
/// and embedded inside the Zenith rootfs image at /sbin/init.

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn main() {
    // ── Mount pseudo-filesystems ───────────────────────────────────────────
    mount_pseudo_fs();

    // ── Open serial channel ────────────────────────────────────────────────
    // On a Firecracker VM the host writes to /dev/hvc1 (virtio-serial).
    // Fall back to stdin/stdout when running outside a VM (for testing).
    let (cmd_str, mut out) = open_channel();

    // ── Execute the step command ───────────────────────────────────────────
    let exit_code = run_command(&cmd_str, &mut out);

    // ── Report exit and power off ──────────────────────────────────────────
    let _ = writeln!(out, "EXIT:{}", exit_code);
    let _ = out.flush();

    power_off();
}

// ─── Mount ────────────────────────────────────────────────────────────────────

fn mount_pseudo_fs() {
    // On a real VM these will succeed; in unit test environments they may fail —
    // we proceed regardless (best-effort).
    #[cfg(target_os = "linux")]
    {
        let _ = std::fs::create_dir_all("/proc");
        let _ = std::fs::create_dir_all("/sys");
        let _ = std::fs::create_dir_all("/dev");

        unsafe {
            // mount("proc",  "/proc", "proc",  0, null)
            // mount("sysfs", "/sys",  "sysfs", 0, null)
            // mount("devtmpfs", "/dev", "devtmpfs", 0, null)
            //
            // We call mount(2) via libc. If libc is not available (musl static),
            // these calls are embedded via raw syscalls in the linker.
            // For portability in this scaffold we use a simple shell mount instead:
            let _ = std::process::Command::new("mount")
                .args(["-t", "proc", "proc", "/proc"])
                .status();
            let _ = std::process::Command::new("mount")
                .args(["-t", "sysfs", "sysfs", "/sys"])
                .status();
        }
    }
}

// ─── Channel ──────────────────────────────────────────────────────────────────

fn open_channel() -> (String, Box<dyn Write>) {
    // Try virtio-serial first (/dev/hvc1), fall back to stdin/stdout.
    if let Ok(f) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/hvc1")
    {
        let mut reader = BufReader::new(f.try_clone().expect("clone hvc1"));
        let writer: Box<dyn Write> = Box::new(f);
        let mut cmd = String::new();
        let _ = reader.read_line(&mut cmd);
        (cmd.trim().to_string(), writer)
    } else {
        // Running outside a VM (local test / development)
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).unwrap_or(0);
        let writer: Box<dyn Write> = Box::new(io::stdout());
        (line.trim().to_string(), writer)
    }
}

// ─── Command execution ────────────────────────────────────────────────────────

fn run_command(cmd: &str, out: &mut dyn Write) -> i32 {
    if cmd.is_empty() {
        let _ = writeln!(out, "E:zenith-init: received empty command");
        return 1;
    }

    let mut child = match Command::new("sh")
        .args(["-c", cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(out, "E:zenith-init: failed to spawn: {}", e);
            return 127;
        }
    };

    // Forward stdout and stderr line-by-line with O:/E: prefix
    // (simplified: we use wait_with_output for the scaffold; a production
    //  implementation would use separate threads to interleave streams)
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let _ = writeln!(out, "E:zenith-init: wait failed: {}", e);
            return 1;
        }
    };

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let _ = writeln!(out, "O:{}", line);
    }
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        let _ = writeln!(out, "E:{}", line);
    }

    output.status.code().unwrap_or(1)
}

// ─── Power off ────────────────────────────────────────────────────────────────

fn power_off() {
    // On Linux: trigger ACPI power off via /proc/sysrq-trigger or reboot(2)
    #[cfg(target_os = "linux")]
    {
        // echo o > /proc/sysrq-trigger  (immediate power off, no sync)
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .open("/proc/sysrq-trigger")
        {
            let _ = f.write_all(b"o");
        }
    }

    // Fallback: just exit; Firecracker will detect PID 1 exit and halt the VM
    std::process::exit(0);
}
