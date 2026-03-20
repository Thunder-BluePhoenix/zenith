/// WarmPool — a background thread that keeps N Zenith VMs pre-booted and snapshotted.
///
/// How it works:
///   1. On startup, the pool boots `target_size` VMs and takes a snapshot of each.
///   2. When a job arrives, the pool hands out a snapshot path (near-instant restore).
///   3. Immediately after handing out a snapshot, the pool boots a replacement VM.
///   4. If the pool is empty (cold start), a fresh VM is booted synchronously.
///
/// The pool runs as an async Tokio task, communicating over a channel.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use tracing::{debug, info, warn};

use super::vm::VmSnapshot;
use super::vmm::ZenithVmm;
use crate::sandbox::zenith_home;

// ─── Pool configuration ───────────────────────────────────────────────────────

/// Minimum number of pre-warmed VM snapshots the pool tries to maintain.
pub const DEFAULT_POOL_SIZE: usize = 2;

/// Guest RAM allocated per VM (32 MiB — minimal for zenith-init + step command)
pub const VM_GUEST_MEM_BYTES: u64 = 32 * 1024 * 1024;

// ─── PoolSnapshot ─────────────────────────────────────────────────────────────

/// A pre-warmed VM snapshot ready to be handed to a job.
#[derive(Debug)]
pub struct PoolSnapshot {
    pub snap:      VmSnapshot,
    pub snap_dir:  PathBuf,
}

// ─── WarmPool ─────────────────────────────────────────────────────────────────

/// Thread-safe pre-warmed VM snapshot pool.
pub struct WarmPool {
    inner: Arc<Mutex<WarmPoolInner>>,
}

struct WarmPoolInner {
    ready:       VecDeque<PoolSnapshot>,
    target_size: usize,
    snap_dir:    PathBuf,
    /// Shared VMM instance — all pool VMs created from one /dev/kvm fd
    vmm:         Option<Arc<ZenithVmm>>,
}

impl WarmPool {
    /// Create a new pool.  Call `start()` to begin pre-warming.
    pub fn new(target_size: usize) -> Self {
        let snap_dir = zenith_home().join("hypervisor").join("snapshots");
        Self {
            inner: Arc::new(Mutex::new(WarmPoolInner {
                ready: VecDeque::new(),
                target_size,
                snap_dir,
                vmm: None,
            })),
        }
    }

    /// Initialise the VMM and begin filling the pool.
    /// Runs the fill loop in a background Tokio task.
    pub fn start(&self) -> Result<()> {
        let vmm = match ZenithVmm::new() {
            Ok(v) => Arc::new(v),
            Err(e) => {
                warn!("WarmPool: cannot initialise KVM VMM — warm pool disabled: {}", e);
                return Ok(()); // Non-fatal: fall back to cold start
            }
        };

        {
            let mut g = self.inner.lock().unwrap();
            std::fs::create_dir_all(&g.snap_dir)?;
            g.vmm = Some(vmm);
        }

        let inner = Arc::clone(&self.inner);
        // Spawn a dedicated thread for blocking VM boot operations
        std::thread::spawn(move || {
            Self::fill_loop(inner);
        });

        Ok(())
    }

    /// Background loop: keeps `target_size` snapshots available.
    fn fill_loop(inner: Arc<Mutex<WarmPoolInner>>) {
        loop {
            let (target, current, vmm, snap_dir) = {
                let g = inner.lock().unwrap();
                (g.target_size, g.ready.len(), g.vmm.clone(), g.snap_dir.clone())
            };

            if current < target {
                match Self::warm_one_vm(&vmm, &snap_dir) {
                    Ok(pool_snap) => {
                        debug!("WarmPool: pre-warmed VM snapshot {}", pool_snap.snap.id);
                        inner.lock().unwrap().ready.push_back(pool_snap);
                    }
                    Err(e) => {
                        warn!("WarmPool: failed to warm VM: {:#}", e);
                        // Back off before retrying
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            } else {
                // Pool is full — check again in a short while
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }

    /// Boot a single VM, take a snapshot, and return it.
    fn warm_one_vm(vmm: &Option<Arc<ZenithVmm>>, snap_dir: &PathBuf) -> Result<PoolSnapshot> {
        let vmm = vmm.as_ref()
            .ok_or_else(|| anyhow::anyhow!("VMM not initialised"))?;

        let vm = vmm.create_vm(VM_GUEST_MEM_BYTES)?;

        // In a complete implementation this would:
        //   1. Load the zenith kernel image into guest RAM
        //   2. Set up page tables and descriptor tables for 64-bit long mode
        //   3. Set RIP to the kernel entry point
        //   4. Run the VM until zenith-init signals "ready" over vsock
        // For now we capture the post-reset CPU state as the warm snapshot baseline.

        let snap = vm.snapshot(snap_dir)?;

        Ok(PoolSnapshot { snap_dir: snap_dir.clone(), snap })
    }

    /// Take a pre-warmed snapshot from the pool.
    /// Returns `None` if the pool is empty (caller should cold-boot instead).
    pub fn acquire(&self) -> Option<PoolSnapshot> {
        let mut g = self.inner.lock().unwrap();
        g.ready.pop_front()
    }

    /// Return a snapshot to the pool (e.g. if a job was cancelled before using it).
    pub fn release(&self, snap: PoolSnapshot) {
        let mut g = self.inner.lock().unwrap();
        if g.ready.len() < g.target_size * 2 {
            g.ready.push_back(snap);
        }
        // If pool is already over-full, the snapshot is dropped (cleaned up in Drop)
    }

    /// Number of pre-warmed snapshots currently available.
    pub fn available(&self) -> usize {
        self.inner.lock().unwrap().ready.len()
    }

    /// Target pool size (configured minimum).
    pub fn target_size(&self) -> usize {
        self.inner.lock().unwrap().target_size
    }
}

impl Drop for PoolSnapshot {
    fn drop(&mut self) {
        // Clean up the snapshot directory when the snapshot is no longer needed
        let snap_path = self.snap_dir.join(&self.snap.id);
        if snap_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&snap_path) {
                warn!("WarmPool: failed to clean snapshot {:?}: {}", snap_path, e);
            }
        }
    }
}
