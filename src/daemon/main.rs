/// zenith-daemon — Zenith background service (Phase 15, Milestone 15.2)
///
/// Manages a pre-warmed VM pool and handles job requests from the `zenith` CLI.
///
/// Launch with: zenith daemon start
/// Or directly: zenith-daemon

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    let pool_target = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(zenith::daemon::server::DEFAULT_POOL_TARGET);

    info!(
        "zenith-daemon v{} — pool target: {} VMs",
        env!("CARGO_PKG_VERSION"),
        pool_target
    );

    // Initialise the warm VM pool (Linux + KVM only; silently skipped otherwise)
    #[cfg(target_os = "linux")]
    {
        let pool = zenith::hypervisor::pool::WarmPool::new(pool_target);
        if let Err(e) = pool.start() {
            tracing::warn!("WarmPool start failed (KVM unavailable?): {}", e);
        }
    }

    let state = zenith::daemon::server::DaemonState::new(pool_target);
    zenith::daemon::server::serve(state).await
}
