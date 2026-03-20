/// Daemon wire protocol — JSON-line encoded messages over Unix/TCP socket.
///
/// One JSON object per line. Lines ending with `\n`.
/// The stream is bidirectional: client sends one `DaemonRequest`, daemon sends
/// one or more `DaemonResponse` frames, terminated by `RunComplete` or `Error`.

use serde::{Deserialize, Serialize};

// ─── Client → Daemon ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    /// Health check — daemon replies immediately with `Pong`.
    Ping,

    /// Submit a job for execution.
    RunJob {
        /// Raw contents of `.zenith.yml`
        config_yaml: String,
        /// Specific job name to run (None = first/only job)
        job:         Option<String>,
        /// Caller's working directory (for resolving relative paths)
        work_dir:    String,
        /// Force re-run all steps, ignore cache
        no_cache:    bool,
    },

    /// Request current daemon status (pool sizes, active jobs, etc.)
    Status,

    /// Gracefully shut down the daemon.
    Shutdown,
}

// ─── Daemon → Client ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Response to `Ping`.
    Pong {
        version:        String,
        pool_ready:     usize,
        pool_target:    usize,
        active_jobs:    usize,
    },

    /// Acknowledgement that a job has been accepted.
    RunAccepted {
        run_id: String,
    },

    /// A log line from a running job (stdout/stderr of a step).
    LogLine {
        run_id: String,
        line:   String,
    },

    /// A step within the job has started.
    StepStarted {
        run_id:    String,
        step_name: String,
    },

    /// A step has completed.
    StepDone {
        run_id:    String,
        step_name: String,
        success:   bool,
        cached:    bool,
    },

    /// The entire job has completed.  This is the final message for this request.
    RunComplete {
        run_id:  String,
        success: bool,
    },

    /// Daemon status snapshot.
    StatusInfo {
        version:     String,
        pool_ready:  usize,
        pool_target: usize,
        active_jobs: usize,
        uptime_secs: u64,
    },

    /// Fatal error. The connection will be closed after this.
    Error {
        message: String,
    },
}

impl DaemonRequest {
    /// Serialise to a JSON line (with trailing `\n`).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default() + "\n"
    }

    /// Parse from a JSON line.
    pub fn from_line(line: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(line.trim())?)
    }
}

impl DaemonResponse {
    /// Serialise to a JSON line (with trailing `\n`).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default() + "\n"
    }

    /// Parse from a JSON line.
    pub fn from_line(line: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(line.trim())?)
    }
}
