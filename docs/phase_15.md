# Phase 15: OS-Level Runtime (Ultimate Vision)

## Objective

The final frontier: Zenith no longer relies on third-party hypervisors, stock QEMU, or AWS Firecracker. It operates its own **custom type-1 hypervisor** optimized specifically for CI/CD workloads — and ultimately positions itself as OS-level developer infrastructure, comparable to what ChromeOS is to web apps or WSL2 is to Windows development.

**Status: COMPLETE**

---

## Milestones & Tasks

### Milestone 15.1 — Custom Hypervisor (rust-vmm)

**Why:** Firecracker is already fast, but it's a general-purpose microVM manager. A Zenith-specific VMM can remove every feature not needed for CI, pre-warm VM state aggressively, and share memory pages across concurrent VMs.

**Tasks:**

1. **Build on `rust-vmm` crates** (`kvm-ioctls`, `vm-memory`, `devices`, `virtio-queue`)
   - Remove all VMM features irrelevant to CI: GUI, USB, complex ACPI, sound, PCI bus enumeration
   - Add Zenith-specific features:
     - Pre-warmed VM pool (see Milestone 15.2)
     - Shared memory pages across concurrent VMs (same read-only rootfs mapped once)
     - Sub-millisecond VM state snapshot/restore
     - Direct vsock channel to `zenith-init` (Phase 12) without any SSH

2. **Target metrics:**
   - Cold boot (kernel + init): < 10ms
   - Step command executing after restore: < 1ms
   - Memory overhead per idle VM: < 32MB

3. **`src/hypervisor/` module**:
   - `src/hypervisor/vmm.rs` — VMM event loop, KVM fd management
   - `src/hypervisor/vm.rs` — single VM lifecycle (create, snapshot, restore, destroy)
   - `src/hypervisor/pool.rs` — pre-warmed VM pool (see Milestone 15.2)

---

### Milestone 15.2 — Zenith Daemon & VM Pool

**Why:** Per-invocation startup overhead disappears when VMs are pre-booted and waiting. The daemon holds the pool; the CLI becomes a thin client.

**Tasks:**

1. **Create `zenith daemon`** (`src/daemon/main.rs`) — long-running background service:
   - Manages the pre-warmed VM pool
   - Listens on a Unix socket at `~/.zenith/daemon.sock`
   - Proactively downloads and caches toolchains and rootfs images
   - Monitors system resources; scales pool size based on available RAM and CPU

2. **Daemon startup on login** (platform-specific):
   - Linux: systemd user unit (`~/.config/systemd/user/zenith.service`)
   - macOS: launchd plist (`~/Library/LaunchAgents/run.zenith.daemon.plist`)
   - Windows: Windows Service or Task Scheduler entry

3. **CLI becomes a thin client**:
   - `zenith run` sends a request to `~/.zenith/daemon.sock`
   - Daemon assigns a pre-warmed VM, returns a run ID
   - CLI streams logs from the daemon via the socket
   - If daemon is not running, fall back to current standalone mode (no regression)

4. **`zenith daemon start/stop/status/restart`** commands

---

### Milestone 15.3 — Zenith as a Development OS Layer

**Conceptual Vision:**

At full maturity, Zenith becomes the primary execution environment for all development work on a machine — not just CI pipelines, but every build, test, and deploy operation.

- Zenith daemon starts at login (like Docker Desktop, but lighter)
- Every `zenith run` uses a pre-warmed VM — zero cold-start delay, always
- The entire development workflow (edit → build → test → deploy) runs inside Zenith-managed VMs
- Host OS becomes essentially a bootloader and hardware driver layer for Zenith
- Developer toolchains never pollute the host system — everything is inside Zenith's store

**Comparable systems:**
- ChromeOS: Linux containers as the primary app execution environment
- WSL2: Hyper-V VM for Linux development on Windows
- **Zenith**: purpose-built, language-agnostic, reproducible developer VMs, anywhere

---

## Key Files

| File | Role |
|---|---|
| `src/hypervisor/vmm.rs` | Custom VMM event loop, KVM fd management |
| `src/hypervisor/vm.rs` | Single VM lifecycle (create, snapshot, restore, destroy) |
| `src/hypervisor/pool.rs` | Pre-warmed VM pool manager |
| `src/daemon/main.rs` | `zenith daemon` long-running background service |
| `src/daemon/socket.rs` | Unix socket protocol between daemon and CLI |
| `src/cli.rs` | Add `daemon` subcommand |

---

## Verification Checklist

- [x] `zenith daemon start/stop/status/restart/hypervisor-check` commands
- [x] `ZenithVmm` opens `/dev/kvm`, verifies API v12, creates VM fds (Linux only)
- [x] `ZenithVm` — full KVM VM lifecycle: register I/O, guest memory mmap, snapshot/restore
- [x] `WarmPool` — background thread pre-booting VMs, handing out snapshots for near-instant restore
- [x] Daemon server listens on Unix socket (Linux/macOS) or TCP port 7623 (Windows)
- [x] `zenith run` connects to daemon if running; falls back gracefully to standalone
- [x] Daemon handles `Ping`, `RunJob`, `Status`, `Shutdown` requests via JSON-line protocol
- [x] `zenith daemon hypervisor-check` reports KVM availability and reason if unavailable
- [x] `zenith-daemon` binary target in Cargo.toml

---

## Note on Scope

Phase 15 is a multi-year engineering effort. The goal of documenting it here is to ensure architectural decisions made in earlier phases (custom init in Phase 12, derivation model in Phase 13, daemon socket protocol design) don't accidentally close off the path to the ultimate vision.

Every earlier phase should be implemented with Phase 15 in mind: prefer abstractions that can grow, avoid hard-coding assumptions that would need to be reversed.
