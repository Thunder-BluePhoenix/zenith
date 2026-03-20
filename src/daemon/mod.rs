/// Zenith Daemon (Phase 15 — Milestone 15.2)
///
/// A long-running background service that:
///   - Maintains a pool of pre-warmed VMs (via src/hypervisor/pool.rs)
///   - Listens on a Unix/TCP socket for job requests from the `zenith` CLI
///   - Provides zero-cold-start job execution by assigning pre-warmed VMs
///   - Falls back gracefully to the Firecracker or container backend on non-Linux hosts
///
/// Architecture:
///   CLI sends DaemonRequest → daemon socket → daemon dispatches job → streams logs back
///
/// Socket location:
///   Linux / macOS: ~/.zenith/daemon.sock (Unix domain socket)
///   Windows:       127.0.0.1:7623        (TCP fallback)

pub mod protocol;
pub mod server;
pub mod client;

use std::path::PathBuf;
use crate::sandbox::zenith_home;

/// The Unix socket path used by the daemon on Unix systems.
pub fn socket_path() -> PathBuf {
    zenith_home().join("daemon.sock")
}

/// TCP port used by the daemon on Windows (or when Unix sockets are unavailable).
pub const TCP_PORT: u16 = 7623;

/// PID file — written by the daemon on startup so `zenith daemon status` can find it.
pub fn pid_file() -> PathBuf {
    zenith_home().join("daemon.pid")
}

/// Return true if the daemon appears to be running (PID file exists and process is live).
pub fn is_running() -> bool {
    let pid_path = pid_file();
    if !pid_path.exists() {
        return false;
    }
    let Ok(pid_str) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };

    // Check if the process with this PID is alive
    #[cfg(unix)]
    {
        // kill(pid, 0) returns 0 if the process exists
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        // On Windows, check if the socket is connectable
        let _ = pid;
        std::net::TcpStream::connect(("127.0.0.1", TCP_PORT)).is_ok()
    }
}
