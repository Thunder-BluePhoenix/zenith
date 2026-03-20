/// Daemon server — listens for incoming connections and dispatches job requests.
///
/// One async task per connection.  All heavy work (VM assignment, step execution)
/// is delegated to a Tokio task spawned per job.

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use anyhow::Result;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::protocol::{DaemonRequest, DaemonResponse};
use crate::config;
use crate::runner;

/// Default number of pre-warmed VM snapshots the daemon tries to maintain.
pub const DEFAULT_POOL_TARGET: usize = 2;

// ─── DaemonState ─────────────────────────────────────────────────────────────

pub struct DaemonState {
    pub version:     String,
    pub start_time:  Instant,
    pub active_jobs: AtomicUsize,
    pub pool_ready:  AtomicUsize,
    pub pool_target: usize,
}

impl DaemonState {
    pub fn new(pool_target: usize) -> Arc<Self> {
        Arc::new(Self {
            version:     env!("CARGO_PKG_VERSION").to_string(),
            start_time:  Instant::now(),
            active_jobs: AtomicUsize::new(0),
            pool_ready:  AtomicUsize::new(0),
            pool_target,
        })
    }
}

// ─── Server entry points ──────────────────────────────────────────────────────

/// Start the daemon server.  Blocks indefinitely.
///
/// On Unix: listens on `~/.zenith/daemon.sock`
/// On Windows: listens on `127.0.0.1:7623`
pub async fn serve(state: Arc<DaemonState>) -> Result<()> {
    // Write PID file
    let pid = std::process::id();
    let pid_path = super::pid_file();
    std::fs::create_dir_all(pid_path.parent().unwrap())?;
    std::fs::write(&pid_path, pid.to_string())?;

    info!("Zenith daemon v{} starting (PID {})", state.version, pid);

    #[cfg(unix)]
    {
        serve_unix(state).await
    }
    #[cfg(not(unix))]
    {
        serve_tcp(state).await
    }
}

#[cfg(unix)]
async fn serve_unix(state: Arc<DaemonState>) -> Result<()> {
    use tokio::net::UnixListener;

    let sock_path = super::socket_path();

    // Remove stale socket file
    if sock_path.exists() {
        std::fs::remove_file(&sock_path)?;
    }

    let listener = UnixListener::bind(&sock_path)?;
    info!("Daemon listening on {:?}", sock_path);

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    let (reader, writer) = stream.into_split();
                    if let Err(e) = handle_connection(
                        BufReader::new(reader), writer, state).await {
                        warn!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => error!("Accept error: {}", e),
        }
    }
}

async fn serve_tcp(state: Arc<DaemonState>) -> Result<()> {
    use tokio::net::TcpListener;
    let listener = TcpListener::bind(("127.0.0.1", super::TCP_PORT)).await?;
    info!("Daemon listening on 127.0.0.1:{}", super::TCP_PORT);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                debug!("Connection from {}", addr);
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    let (reader, writer) = stream.into_split();
                    if let Err(e) = handle_connection(
                        BufReader::new(reader), writer, state).await {
                        warn!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => error!("Accept error: {}", e),
        }
    }
}

// ─── Connection handler ───────────────────────────────────────────────────────

async fn handle_connection<R, W>(
    mut reader: BufReader<R>,
    mut writer: W,
    state: Arc<DaemonState>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.trim().is_empty() { return Ok(()); }

    let request = match DaemonRequest::from_line(&line) {
        Ok(r) => r,
        Err(e) => {
            let resp = DaemonResponse::Error { message: format!("Bad request: {}", e) };
            writer.write_all(resp.to_line().as_bytes()).await?;
            return Ok(());
        }
    };

    match request {
        DaemonRequest::Ping => {
            let resp = DaemonResponse::Pong {
                version:     state.version.clone(),
                pool_ready:  state.pool_ready.load(Ordering::Relaxed),
                pool_target: state.pool_target,
                active_jobs: state.active_jobs.load(Ordering::Relaxed),
            };
            writer.write_all(resp.to_line().as_bytes()).await?;
        }

        DaemonRequest::Status => {
            let resp = DaemonResponse::StatusInfo {
                version:     state.version.clone(),
                pool_ready:  state.pool_ready.load(Ordering::Relaxed),
                pool_target: state.pool_target,
                active_jobs: state.active_jobs.load(Ordering::Relaxed),
                uptime_secs: state.start_time.elapsed().as_secs(),
            };
            writer.write_all(resp.to_line().as_bytes()).await?;
        }

        DaemonRequest::Shutdown => {
            info!("Daemon received Shutdown request — exiting.");
            let resp = DaemonResponse::Pong {
                version: state.version.clone(),
                pool_ready: 0, pool_target: 0, active_jobs: 0,
            };
            writer.write_all(resp.to_line().as_bytes()).await?;
            std::process::exit(0);
        }

        DaemonRequest::RunJob { config_yaml, job, work_dir, no_cache } => {
            let run_id = Uuid::new_v4().to_string();

            let accepted = DaemonResponse::RunAccepted { run_id: run_id.clone() };
            writer.write_all(accepted.to_line().as_bytes()).await?;

            state.active_jobs.fetch_add(1, Ordering::Relaxed);

            // Parse config
            let cfg = match serde_yaml::from_str::<config::ZenithConfig>(&config_yaml) {
                Ok(c) => c,
                Err(e) => {
                    let resp = DaemonResponse::Error { message: format!("Config parse error: {}", e) };
                    writer.write_all(resp.to_line().as_bytes()).await?;
                    state.active_jobs.fetch_sub(1, Ordering::Relaxed);
                    return Ok(());
                }
            };

            // Run the job in the caller's working directory
            let prev_dir = std::env::current_dir().unwrap_or_default();
            if let Ok(wd) = std::path::Path::new(&work_dir).canonicalize() {
                let _ = std::env::set_current_dir(&wd);
            }

            let run_id2 = run_id.clone();
            let success = match runner::execute_local(cfg, job, no_cache).await {
                Ok(())  => true,
                Err(e) => {
                    let err_resp = DaemonResponse::LogLine {
                        run_id: run_id2.clone(),
                        line: format!("[daemon] job error: {}", e),
                    };
                    writer.write_all(err_resp.to_line().as_bytes()).await?;
                    false
                }
            };

            let _ = std::env::set_current_dir(prev_dir);
            state.active_jobs.fetch_sub(1, Ordering::Relaxed);

            let done = DaemonResponse::RunComplete { run_id: run_id.clone(), success };
            writer.write_all(done.to_line().as_bytes()).await?;
        }
    }

    Ok(())
}
