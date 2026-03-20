# Phase 12: Low-Level System Optimization

## Objective

Squeeze every millisecond of boot time and every byte of memory out of Zenith's VM layer. Build a custom minimal Linux environment tuned specifically for CI/CD workloads — no shell, no SSH, no bloat.

**Target:** Firecracker VM cold start → step executing in **under 50ms**.

**Status: COMPLETE**

---

## Milestones & Tasks

### Milestone 12.1 — Custom Minimal Kernel

**Tasks:**

1. **Build a stripped-down Linux kernel** specifically for Zenith VMs
   - Disable everything not needed for CI: sound, USB, most drivers, bluetooth, wireless
   - Enable: `virtio`, `9p`, `overlayfs`, KVM guest support, minimal network stack, `vsock`
   - Target boot time: under 50ms from kernel start to init
   - Store at `~/.zenith/kernel/vmlinux-zenith`

2. **Document the kernel `.config`** in `kernel/zenith.config` in the repo
   - Reproducible kernel builds from the documented config
   - CI job that rebuilds the kernel from config and compares checksums

3. **Auto-download on first use** (Zenith motto)
   - Add `ensure_zenith_kernel()` to `src/tools.rs`
   - Host pre-built kernel at a Zenith CDN URL; verify SHA-256

---

### Milestone 12.2 — Custom `init` Process (PID 1)

**Why:** Stock init systems (systemd, OpenRC, BusyBox init) are designed for general-purpose Linux. For CI, PID 1 needs to do exactly one thing: execute the step command and exit.

**Tasks:**

1. **Write `zenith-init` binary** in `src/init/main.rs`
   - Compiled as `x86_64-unknown-linux-musl` (fully static, zero dependencies)
   - Responsibilities as PID 1:
     - Mount `/proc`, `/sys`, `/dev`
     - Set up `vsock` or `virtio-serial` channel with the host Zenith process
     - Receive the step command over the channel
     - Execute it with `execve`, forwarding stdout/stderr back over the channel
     - Signal completion with exit code
     - Power off the VM cleanly: `reboot(RB_POWER_OFF)`
   - Result: no shell, no SSH, no extra processes — just the one command and immediate clean shutdown

2. **Integrate `zenith-init` with `FirecrackerBackend`**
   - `provision()` boots the Firecracker VM with `zenith-init` embedded in the rootfs as `/sbin/init`
   - `execute()` sends the command over vsock, receives exit code + log stream

3. **Add `zenith-init` as a third binary target in `Cargo.toml`**:
   ```toml
   [[bin]]
   name = "zenith-init"
   path = "src/init/main.rs"
   ```

---

### Milestone 12.3 — Rootfs Optimization

**Tasks:**

1. **Build a Zenith-specific minimal rootfs** (smaller than Alpine's 3MB)
   - Start from a musl-libc base (BusyBox + musl)
   - Add only: `sh`, `curl`, `git`, `make`, `tar` — bare minimum for CI steps
   - Target size: under 5MB uncompressed
   - Host as `zenith-rootfs-minimal.tar.gz` on CDN
   - Add `ensure_zenith_rootfs()` to `src/tools.rs`

2. **Implement rootfs deduplication (content-addressable store)**
   - Store rootfs layers as immutable snapshots keyed by content SHA-256
   - Multiple concurrent VMs share the same read-only base layer via `overlayfs`
   - Only the writable overlay layer is unique per VM run
   - This mirrors Docker image layers, but without Docker

3. **Snapshot-based sandbox restore (VM pool warm-up)**
   - After provisioning, take a memory snapshot of the idle VM state
   - Restore from snapshot instead of cold-booting for subsequent runs
   - Estimated benefit: cold boot ~200ms → restore ~10ms

---

## Key Files

| File | Role |
|---|---|
| `src/init/main.rs` | `zenith-init` PID 1 binary |
| `src/tools.rs` | Add `ensure_zenith_kernel()`, `ensure_zenith_rootfs()` |
| `src/sandbox/firecracker.rs` | Update to use custom kernel + init |
| `kernel/zenith.config` | Documented, reproducible kernel config |
| `Cargo.toml` | Add `[[bin]] zenith-init` target |

---

## Verification Checklist

- [ ] Custom kernel boots a Firecracker VM to a ready state in under 50ms
- [ ] `zenith-init` PID 1 executes a step command via vsock and powers off cleanly
- [ ] No shell or SSH process running inside the VM — only `zenith-init` and the step command
- [ ] Custom rootfs is under 5MB uncompressed
- [ ] Multiple concurrent VMs share the same read-only rootfs layer (no data duplication on disk)
- [ ] VM restore from snapshot is measurably faster than cold boot
