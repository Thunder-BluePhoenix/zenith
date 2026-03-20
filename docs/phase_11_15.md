# Phase 11–15: Developer Platform & OS-Level Runtime

## Overview

These phases represent Zenith's long-term evolution from a powerful CLI tool into a **Universal Developer Runtime** — a first-class platform with visual tooling, a custom hypervisor, reproducible packaging, and ultimately OS-level developer infrastructure.

Each phase builds directly on the last. They are presented here with enough detail to be actionable, but with the understanding that some will span months or years of work.

---

## Phase 11: GUI & IDE Integration

### Objective

Give Zenith a visual face. Developers should be able to see workflow status, browse logs, inspect sandbox state, and trigger runs without touching the terminal.

### Concept

```bash
zenith ui          # open web dashboard in browser
zenith ui --port 9000
```

### Milestones & Tasks

#### 11.1 — Embedded HTTP Dashboard

**Tasks:**

1. **Add a web server to the Zenith binary**
   - Add `axum = "0.7"` and `tower = "0.4"` to `Cargo.toml`
   - Create `src/ui/server.rs` — starts an `axum` HTTP server on `127.0.0.1:7622`
   - Add `zenith ui` command that starts the server and opens the browser

2. **Build API endpoints the dashboard will consume**:
   - `GET /api/runs` — list recent workflow runs (read from `~/.zenith/logs/`)
   - `GET /api/runs/{id}` — get run detail and step results
   - `GET /api/runs/{id}/logs` — stream logs via SSE
   - `GET /api/labs` — list active sandbox labs
   - `GET /api/cache` — cache stats (entries, total size, hit rate)

3. **Build a minimal web frontend** (embed as static files in the binary)
   - Use plain HTML + JavaScript or a lightweight framework (no heavy build toolchain)
   - Show: run history table, log viewer (auto-scrolling), status badges (green/red/running)
   - Embed into the binary using `include_str!` or the `rust-embed` crate

4. **Persist run history to `~/.zenith/logs/`**
   - Each run gets a directory: `~/.zenith/logs/<run-id>/`
   - Store `summary.json` (start time, status, job name) and `steps.jsonl` (one JSON per step with logs)
   - The runner in `src/runner.rs` writes these during execution

---

#### 11.2 — VSCode Extension

**Tasks:**

1. **Create a VSCode extension** in a new `vscode-zenith/` directory at repo root
   - Initialize with `yo code` or manually with `package.json` + `extension.ts`
   - Tech: TypeScript, `vscode` API, calls `zenith` CLI as a subprocess

2. **Extension features (Phase 11.2a — Basic)**:
   - Command palette: `Zenith: Run Workflow`, `Zenith: Open Dashboard`
   - Status bar item showing last run result (pass/fail/running)
   - Output panel for streaming `zenith run` output

3. **Extension features (Phase 11.2b — Advanced)**:
   - `.zenith.yml` YAML schema validation (JSON Schema for autocomplete)
   - Inline code lens: "Run this job" buttons above job definitions in the YAML
   - Side panel showing lab status and cache stats

---

#### 11.3 — TUI (Terminal User Interface) Upgrade

**Why:** Even without the web UI, the terminal experience should be polished. Replace raw log output with a proper TUI during matrix runs.

**Tasks:**

1. **Add `ratatui = "0.26"` to `Cargo.toml`** (successor to tui-rs)

2. **Create `src/tui/mod.rs`** — render a live dashboard in the terminal:
   - Split panes: left shows job/step list with status icons, right shows the log of the selected job
   - Updates in real time as steps complete
   - Activated when running a matrix with 2+ jobs

---

## Phase 12: Low-Level System Optimization

### Objective

Squeeze every millisecond of boot time and every byte of memory out of Zenith's VM layer. Build a custom minimal Linux environment tuned specifically for CI/CD workloads.

### Milestones & Tasks

#### 12.1 — Custom Minimal Kernel

**Tasks:**

1. **Build a stripped-down Linux kernel** specifically for Zenith VMs
   - Disable everything not needed for CI: sound, USB, most drivers
   - Enable: `virtio`, `9p`, `overlayfs`, KVM guest support, minimal network stack
   - Target boot time: under 50ms from kernel start to init
   - Store in `~/.zenith/kernel/vmlinux-zenith`

2. **Document the kernel `.config`** in `kernel/zenith.config` in the repo
   - This lets contributors reproduce and improve the kernel

#### 12.2 — Custom `init` Process (PID 1)

**Tasks:**

1. **Write a minimal `zenith-init` binary** in `src/init/main.rs`
   - Compiled as `x86_64-unknown-linux-musl` (static, zero dependencies)
   - Responsibilities as PID 1:
     - Mount `/proc`, `/sys`, `/dev`
     - Set up vsock or virtio-serial to accept commands from the host Zenith process
     - Execute the requested step command
     - Stream stdout/stderr back over the transport
     - Signal completion with exit code
     - Power off the VM cleanly (`reboot(RB_POWER_OFF)`)
   - Result: no shell, no `ssh`, no extra processes — just the one command and exit

2. **Integrate `zenith-init` with the Firecracker backend**
   - When `FirecrackerBackend::provision` runs, it boots the Firecracker VM with `zenith-init` as PID 1
   - Commands are sent via vsock; logs arrive back via vsock

#### 12.3 — Rootfs Optimization

**Tasks:**

1. **Build a Zenith-specific minimal rootfs** (smaller than Alpine)
   - Start from a musl-libc base (BusyBox + musl)
   - Add only: `sh`, `curl`, `git`, `make` — the bare minimum for CI
   - Target size: under 5MB uncompressed

2. **Implement rootfs deduplication (content-addressable store)**
   - Store rootfs layers as immutable snapshots keyed by content hash
   - Multiple VMs can share the same read-only base layer — only the overlay is unique per VM
   - This mirrors how Docker image layers work but without Docker

---

## Phase 13: Build System & Reproducibility Engine

### Objective

Go deeper than Phase 6's step caching. Build a full **content-addressable build system** where every output is uniquely identified by its inputs — like Nix derivations. Given the same inputs, Zenith always produces bit-for-bit identical outputs, forever.

### Milestones & Tasks

#### 13.1 — Derivation Model

**Tasks:**

1. **Introduce the `Derivation` concept** in `src/build/derivation.rs`:
   - A derivation is a pure function: `inputs (files + env + command) -> outputs`
   - Represented as a struct that serializes to a deterministic JSON form
   - Its hash is the build's unique identity

2. **`zenith build --derivation`** — evaluate and print a derivation without executing it
   - Lets users inspect exactly what Zenith considers as inputs before running

#### 13.2 — Binary Cache (Nix-style)

**Tasks:**

1. **Implement a local binary cache at `~/.zenith/store/<hash>/`**
   - Each successfully built derivation stores its output at this path
   - Multiple projects that produce the same artifact (same hash) share the store entry

2. **Implement a remote binary cache** (pull-through)
   - Configure a remote cache URL in `~/.zenith/config.toml`:
     `[cache] remote = "https://cache.zenith.run"`
   - Before building, check the remote cache for the derivation hash
   - On hit: download and restore — no local build required

---

## Phase 14: Full Developer Platform

### Objective

Unite all previous phases into a seamless **Universal Developer Runtime**. Phase 14 is integration work — making everything work together coherently and polish the user experience to production quality.

### Milestones & Tasks

#### 14.1 — Unified Config Schema v2

**Tasks:**

1. **Revise `.zenith.yml` to support all features introduced in Phases 6-13**:
   ```yaml
   version: 2

   env:
     node: "20"
     python: "3.12"

   cache:
     ttl_days: 14
     remote: "https://cache.zenith.run"

   jobs:
     test:
       runs-on: alpine
       backend: firecracker
       arch: aarch64
       strategy:
         matrix:
           os: [ubuntu, alpine]
       steps:
         - name: Build
           run: cargo build --release
           watch: [src/**/*.rs]
           outputs: [target/release/myapp]
         - name: Test
           run: cargo test
   ```

2. **Write a migration guide** from v1 to v2 schema (for backward compatibility)

#### 14.2 — Performance Benchmarking Suite

**Tasks:**

1. **Create `benches/` directory** with Rust criterion benchmarks
   - Measure: cold start VM boot time, matrix spawn time, cache hit latency, rootfs extraction
   - Run automatically in CI to catch regressions

2. **Create a `zenith benchmark` command** that runs the suite and prints a human-readable report

#### 14.3 — Documentation Site

**Tasks:**

1. **Set up a documentation site** using `mdBook` (Rust-native, compiles to static HTML)
   - Sections: Getting Started, Configuration Reference, Backend Guide, Plugin Authoring, Cloud
   - Auto-generated from docs markdown files in this repo

2. **Write interactive examples** — `.zenith.yml` snippets that users can copy and run immediately

---

## Phase 15: OS-Level Runtime (Ultimate Vision)

### Objective

The final frontier: Zenith no longer relies on Linux KVM, AWS Firecracker, or stock QEMU. It operates its own **custom type-1 hypervisor** optimized specifically for the CI/CD workload profile — and ultimately positions itself as OS-level developer infrastructure.

### Milestones & Tasks (Conceptual — Long Horizon)

#### 15.1 — Custom Hypervisor (rust-vmm)

**Tasks:**

1. **Fork/extend `rust-vmm` crates** (`kvm-ioctls`, `vm-memory`, `devices`) to build a Zenith-specific VMM
   - Remove all VMM features not needed for CI (GUI, USB, sound, complex ACPI)
   - Add Zenith-specific optimizations: pre-warmed VM pool, shared memory pages across concurrent VMs

2. **Implement a VM pool** — pre-boot N VMs at `zenith daemon` startup
   - When a workflow needs a VM, assign one from the warm pool (sub-10ms "boot" time)
   - Replenish the pool asynchronously as VMs are consumed

#### 15.2 — Zenith Daemon

**Tasks:**

1. **Create `zenith daemon`** — a long-running background service
   - Manages the VM pool
   - Caches toolchains and rootfs images proactively
   - Provides a Unix socket API for the CLI to communicate with
   - Replaces per-invocation startup overhead with a persistent smart scheduler

2. **CLI becomes a thin client** — `zenith run` sends a request to the daemon socket
   - Daemon schedules the job, returns a run ID
   - CLI streams logs from the daemon via the socket

#### 15.3 — Zenith as a Development OS Layer

**Conceptual vision:**

- Zenith daemon starts at login (like Docker Desktop)
- Every `zenith run` invocation uses a pre-warmed Firecracker VM — zero cold-start delay
- The entire development workflow (build, test, deploy) runs inside Zenith-managed VMs
- Host OS is essentially just a bootloader for Zenith
- Comparable to: ChromeOS (Linux containers) or WSL2 (Hyper-V VM) but purpose-built for developers

---

## Key Files Reference (Phase 11-15)

| File/Dir | Phase | Role |
|---|---|---|
| `src/ui/server.rs` | 11 | Axum HTTP server for web dashboard |
| `src/ui/api.rs` | 11 | REST API endpoints |
| `src/tui/mod.rs` | 11 | Ratatui terminal UI for matrix runs |
| `vscode-zenith/` | 11 | VSCode extension root |
| `kernel/zenith.config` | 12 | Custom Linux kernel config |
| `src/init/main.rs` | 12 | `zenith-init` PID 1 binary |
| `src/build/derivation.rs` | 13 | Nix-style derivation model |
| `src/build/store.rs` | 13 | Content-addressable build store |
| `benches/` | 14 | Criterion performance benchmarks |
| `docs/` | 14 | mdBook documentation site |
| `src/daemon/main.rs` | 15 | `zenith daemon` long-running service |
| `src/daemon/pool.rs` | 15 | Pre-warmed VM pool manager |
| `src/hypervisor/` | 15 | Custom rust-vmm-based hypervisor |

---

## Verification Checklist (Phase 11-15 Targets)

**Phase 11:**
- [ ] `zenith ui` opens a browser with a functional dashboard showing run history
- [ ] VSCode extension shows workflow status in the status bar
- [ ] Matrix runs display in a TUI with per-job log panes

**Phase 12:**
- [ ] Custom kernel boots Firecracker VM to a ready state in under 50ms
- [ ] `zenith-init` PID 1 binary executes a step command and powers off cleanly
- [ ] Custom rootfs is under 5MB

**Phase 13:**
- [ ] Same derivation hash from two different machines produces identical output
- [ ] `zenith build` fetches a pre-built artifact from the remote binary cache
- [ ] Cache hit means zero local compilation — just download and extract

**Phase 14:**
- [ ] `.zenith.yml` v2 schema passes validation with all feature combinations
- [ ] Criterion benchmarks run in CI and catch regressions
- [ ] Documentation site builds and is browsable

**Phase 15:**
- [ ] `zenith daemon` starts at boot and holds a pool of pre-warmed VMs
- [ ] `zenith run` with warm pool: first step starts executing in under 10ms
- [ ] CLI is a thin client — no heavy work happens in the CLI process
