/// Zenith Custom Hypervisor (Phase 15 — Milestone 15.1)
///
/// A purpose-built KVM-based microVM manager optimised exclusively for CI workloads.
/// Replaces the Firecracker dependency for maximum performance and control.
///
/// Goals:
///   - Cold boot: < 10 ms kernel-to-init
///   - Warm restore (from snapshot): < 1 ms
///   - Memory overhead per idle VM: < 32 MB
///
/// Architecture:
///   ZenithVmm         — owns the /dev/kvm fd; creates VM fds
///   ZenithVm          — single VM lifecycle (create, snapshot, restore, destroy)
///   WarmPool          — background thread that keeps N VMs pre-booted and snapshotted
///
/// Platform: Linux only (KVM requires Linux kernel 4.0+)

#[cfg(target_os = "linux")]
pub mod vmm;

#[cfg(target_os = "linux")]
pub mod vm;

#[cfg(target_os = "linux")]
pub mod pool;

// ─── Cross-platform public API ────────────────────────────────────────────────

/// Returns true if the Zenith hypervisor is supported on the current platform.
pub fn is_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/dev/kvm").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Human-readable reason why the hypervisor is unavailable (for error messages).
pub fn unavailable_reason() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if !std::path::Path::new("/dev/kvm").exists() {
            "KVM device (/dev/kvm) not found — enable KVM in your kernel and BIOS"
        } else {
            "Unknown error initialising KVM"
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        "The Zenith custom hypervisor requires Linux with KVM support"
    }
}
