/// Daemon client — CLI-side code that connects to the daemon socket and sends requests.
///
/// Used by `zenith run` (and `zenith build`) to offload execution to the daemon
/// when it is running, achieving near-zero startup latency via the warm VM pool.

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::debug;

use super::protocol::{DaemonRequest, DaemonResponse};

// ─── High-level client operations ────────────────────────────────────────────

/// Try to connect to the daemon and submit a run job.
///
/// Returns `Ok(true)` on success, `Ok(false)` on job failure.
/// Returns `Err(...)` only when the daemon is unreachable (caller should fall back).
pub async fn try_run_via_daemon(
    config_yaml: &str,
    job:         Option<&str>,
    no_cache:    bool,
) -> Result<bool> {
    let mut conn = connect().await
        .context("Daemon unreachable")?;

    let work_dir = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let req = DaemonRequest::RunJob {
        config_yaml: config_yaml.to_string(),
        job:         job.map(|s| s.to_string()),
        work_dir,
        no_cache,
    };

    conn.send(&req).await?;

    // Stream responses until RunComplete or Error
    let mut success = false;
    loop {
        let resp = conn.recv().await?;
        match resp {
            DaemonResponse::RunAccepted { run_id } => {
                debug!(run_id, "Job accepted by daemon");
            }
            DaemonResponse::LogLine { line, .. } => {
                println!("{}", line);
            }
            DaemonResponse::StepStarted { step_name, .. } => {
                println!("[daemon] Step starting: {}", step_name);
            }
            DaemonResponse::StepDone { step_name, success: s, cached, .. } => {
                if cached {
                    println!("[daemon] ✓ {} (cached)", step_name);
                } else if s {
                    println!("[daemon] ✓ {}", step_name);
                } else {
                    println!("[daemon] ✗ {} (failed)", step_name);
                }
            }
            DaemonResponse::RunComplete { success: s, .. } => {
                success = s;
                break;
            }
            DaemonResponse::Error { message } => {
                return Err(anyhow::anyhow!("Daemon error: {}", message));
            }
            _ => {}
        }
    }

    Ok(success)
}

/// Send a Ping and return the daemon status, or Err if unreachable.
pub async fn ping() -> Result<DaemonResponse> {
    let mut conn = connect().await?;
    conn.send(&DaemonRequest::Ping).await?;
    conn.recv().await
}

/// Send a Shutdown request to the daemon.
pub async fn shutdown() -> Result<()> {
    let mut conn = connect().await?;
    conn.send(&DaemonRequest::Shutdown).await?;
    let _ = conn.recv().await; // best-effort
    Ok(())
}

// ─── Connection abstraction ───────────────────────────────────────────────────

struct DaemonConn {
    reader: tokio::io::Lines<BufReader<Box<dyn tokio::io::AsyncRead + Unpin + Send>>>,
    writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
}

impl DaemonConn {
    async fn send(&mut self, req: &DaemonRequest) -> Result<()> {
        self.writer.write_all(req.to_line().as_bytes()).await
            .context("Failed to write to daemon socket")?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<DaemonResponse> {
        let line = self.reader.next_line().await
            .context("Daemon connection closed")?
            .ok_or_else(|| anyhow::anyhow!("Daemon closed connection unexpectedly"))?;
        DaemonResponse::from_line(&line)
    }
}

/// Connect to the daemon via Unix socket (Unix) or TCP (Windows).
async fn connect() -> Result<DaemonConn> {
    #[cfg(unix)]
    {
        use tokio::net::UnixStream;
        let sock_path = super::socket_path();
        let stream = UnixStream::connect(&sock_path).await
            .with_context(|| format!("Cannot connect to daemon socket {:?}", sock_path))?;
        let (read_half, write_half) = stream.into_split();
        let reader: Box<dyn tokio::io::AsyncRead + Unpin + Send> = Box::new(read_half);
        let writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = Box::new(write_half);
        Ok(DaemonConn {
            reader: BufReader::new(reader).lines(),
            writer,
        })
    }
    #[cfg(not(unix))]
    {
        use tokio::net::TcpStream;
        let stream = TcpStream::connect(("127.0.0.1", super::TCP_PORT)).await
            .context("Cannot connect to daemon TCP port")?;
        let (read_half, write_half) = stream.into_split();
        let reader: Box<dyn tokio::io::AsyncRead + Unpin + Send> = Box::new(read_half);
        let writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = Box::new(write_half);
        Ok(DaemonConn {
            reader: BufReader::new(reader).lines(),
            writer,
        })
    }
}
