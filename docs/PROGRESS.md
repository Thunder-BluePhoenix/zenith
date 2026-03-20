# Zenith — Build Progress Tracker

Last updated: 2026-03-20

---

## Motto

> **"You install Zenith. Zenith installs everything else."**

Every tool Zenith needs — Firecracker, QEMU, wasmtime, rootfs images, language toolchains —
is downloaded and cached by Zenith itself into `~/.zenith/`.
Users never run `apt install`, `brew install`, or any other system-level setup.

See [motto.md](motto.md) for the full principle and implementation pattern.

---

## Overall Progress

```
Phase 0   [##########] 100%  CLI Core & Foundation              COMPLETE
Phase 1   [##########] 100%  Lab Environments (Sandbox)         COMPLETE
Phase 2   [##########] 100%  Workflow Engine (Local CI)          COMPLETE
Phase 3   [##########] 100%  Matrix Runner (Multi-OS)            COMPLETE
Phase 4   [##########] 100%  MicroVM Backend Engine             COMPLETE
Phase 5   [##########] 100%  Cross-OS / Cross-Arch Runtime      COMPLETE
Phase 6   [###-------]  30%  Build & Cache System               IN PROGRESS
Phase 7   [----------]   0%  Env & Package System               NOT STARTED
Phase 8   [----------]   0%  Plugin System                      NOT STARTED
Phase 9   [----------]   0%  Remote Runner                      NOT STARTED
Phase 10  [----------]   0%  Cloud Runtime                      NOT STARTED
Phase 11  [----------]   0%  GUI & IDE Integration              NOT STARTED
Phase 12  [----------]   0%  Low-Level System Optimization      NOT STARTED
Phase 13  [----------]   0%  Reproducibility Engine             NOT STARTED
Phase 14  [----------]   0%  Full Developer Platform            NOT STARTED
Phase 15  [----------]   0%  OS-Level Runtime (Ultimate)        NOT STARTED
```

---

## Phase 0 — CLI Core & Foundation

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| Rust/Cargo project, all core deps | DONE | `Cargo.toml` |
| `clap`-based CLI: `run`, `lab`, `matrix`, `shell` | DONE | `src/cli.rs` |
| `.zenith.yml` parser: `ZenithConfig`, `Job`, `Step`, `Strategy` | DONE | `src/config.rs` |
| Local shell command runner | DONE | `src/runner.rs` |
| Structured logging via `tracing` | DONE | `src/main.rs` |

---

## Phase 1 — Lab Environments (Sandbox)

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| Rootfs download (Alpine CDN, no external tools) | DONE | `src/sandbox/mod.rs` |
| Rootfs extraction (tar.gz via `flate2`+`tar`) | DONE | `src/sandbox/mod.rs` |
| Rootfs cache at `~/.zenith/rootfs/` | DONE | `src/sandbox/mod.rs` |
| `zenith lab create/shell/run/push/destroy/list` | DONE | `src/sandbox/mod.rs` |
| Linux namespace isolation (PID + mount + net) | DONE | `src/sandbox/mod.rs` linux module |
| **OverlayFS upper/lower layer isolation** | DONE | `src/sandbox/mod.rs` — `mount_overlay` |
| Auto-fallback to workspace-copy when no CAP_SYS_ADMIN | DONE | `provision_lab()` try/warn/fallback |
| Overlay unmount on teardown | DONE | `teardown_lab()` — `unmount_overlay` |
| Overlay merged dir used in `exec_in_lab` | DONE | `exec_in_lab()` path resolution |
| Windows/macOS fallback (cleaned subprocess) | DONE | `src/sandbox/mod.rs` |

---

## Phase 2 — Workflow Engine (Local CI)

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `jobs`/`steps` YAML schema | DONE | `src/config.rs` |
| Sequential step executor | DONE | `src/runner.rs` |
| Job + step env var merge and propagation | DONE | `src/runner.rs` |
| Non-zero exit code halts pipeline | DONE | `src/runner.rs` |
| `allow_failure` per step | DONE | `src/config.rs`, `src/runner.rs` |
| `zenith run` / `zenith run --job <name>` | DONE | `src/cli.rs`, `src/main.rs` |
| Working directory per step/job | DONE | `src/config.rs`, `src/runner.rs` |
| Lab teardown always runs (even on failure) | DONE | `src/runner.rs` |

---

## Phase 3 — Matrix Runner (Multi-OS Pipelines)

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `strategy.matrix` YAML parsing | DONE | `src/config.rs` |
| Multi-dimensional matrix expansion | DONE | `src/runner.rs` — `expand_matrix()` |
| Parallel job execution with `JoinSet` | DONE | `src/runner.rs` |
| `${{ matrix.key }}` placeholder resolution | DONE | `src/runner.rs` — `resolve_placeholders()` |
| Unique UUID session IDs per matrix node | DONE | `src/runner.rs` |
| Overall pass/fail aggregation | DONE | `src/runner.rs` |
| Per-instance log prefix `[job-os]` | DONE | `src/runner.rs` |

---

## Phase 4 — MicroVM Backend Engine

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `Backend` trait abstraction | DONE | `src/sandbox/backend.rs` |
| `ContainerBackend` — namespace isolation | DONE | `src/sandbox/container.rs` |
| `FirecrackerBackend` struct | DONE | `src/sandbox/firecracker.rs` |
| **Motto: Firecracker binary auto-downloaded** | DONE | `src/tools.rs` — `ensure_firecracker()` |
| **Motto: vmlinux kernel auto-downloaded** | DONE | `src/tools.rs` — `ensure_fc_kernel()` |
| **Motto: ext4 rootfs auto-downloaded** | DONE | `src/tools.rs` — `ensure_fc_rootfs()` |
| KVM availability check with actionable errors | DONE | `src/sandbox/firecracker.rs` — `check_kvm()` |
| Firecracker process launch (api-sock) | DONE | `src/sandbox/firecracker.rs` — `execute()` |
| REST API config (boot-source, drive, machine) | DONE | `src/sandbox/firecracker.rs` — `fc_configure_vm()` |
| Raw HTTP/1.1 client over UnixStream (no extra dep) | DONE | `src/sandbox/firecracker.rs` — `fc_api()` |
| Socket wait with timeout | DONE | `src/sandbox/firecracker.rs` — `wait_for_socket()` |
| Command embedded in kernel boot cmdline | DONE | `boot_args` format in `execute()` |
| Serial console output stream + exit code detection | DONE | `src/sandbox/firecracker.rs` — `read_serial_output()` |
| Per-run rootfs snapshot (copy-on-write) | DONE | `execute()` — `rootfs_snap` |
| Backend factory + `fc` alias | DONE | `src/sandbox/mod.rs` — `get_backend()` |
| Clear error on Windows/macOS | DONE | `#[cfg(not(target_os = "linux"))]` blocks |

---

## Phase 5 — Cross-OS / Cross-Arch Runtime

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `arch` field in `Job` config | DONE | `src/config.rs` |
| Host vs target arch detection | DONE | `src/sandbox/mod.rs` |
| **Motto: QEMU user-static auto-downloaded** | DONE | `src/tools.rs` — `ensure_qemu_for_arch()` |
| QEMU binary passed into namespace runner | DONE | `src/sandbox/mod.rs` — `exec_in_lab` |
| Cross-arch QEMU prefix in `run_namespaced` | DONE | `src/sandbox/mod.rs` linux module |
| **binfmt_misc auto-registration** | DONE | `src/sandbox/mod.rs` — `register_binfmt_qemu()` |
| binfmt_misc fallback (non-fatal if no CAP) | DONE | `exec_in_lab()` — debug log + continue |
| `WasmBackend` struct | DONE | `src/sandbox/wasm.rs` |
| **Motto: wasmtime auto-downloaded** | DONE | `src/tools.rs` — `ensure_wasmtime()` |
| wasmtime CLI invocation for `.wasm` files | DONE | `src/sandbox/wasm.rs` |
| WASI `--dir` workspace mount | DONE | `src/sandbox/wasm.rs` |
| WASI env var pass-through via `--env` | DONE | `src/sandbox/wasm.rs` |
| **WineBackend** (Windows .exe on Linux) | DONE | `src/sandbox/wine.rs` |
| **Motto: Wine portable auto-downloaded** | DONE | `src/tools.rs` — `ensure_wine()` |
| Isolated WINEPREFIX per lab | DONE | `src/sandbox/wine.rs` — `provision()` |
| `wine` / `windows-wine` backend routing | DONE | `src/sandbox/mod.rs` — `get_backend()` |
| `alpine-arm64` rootfs source URL | DONE | `src/sandbox/mod.rs` — `ROOTFS_SOURCES` |
| Darling/Lima (macOS binary on Linux) | FUTURE | Phase 11+ |

---

## Phase 6 — Build & Cache System

**Status:** IN PROGRESS — basic step caching done. File-watching and artifact caching next.

| Task | Status | File |
|---|---|---|
| `CacheManager` struct | DONE | `src/sandbox/cache.rs` |
| SHA-256 step hash (cmd + env + os + arch) | DONE | `src/sandbox/cache.rs` |
| Cache hit check + skip step | DONE | `src/runner.rs` |
| Cache marker written on step success | DONE | `src/sandbox/cache.rs` |
| `glob` crate added to `Cargo.toml` | DONE | `Cargo.toml` |
| `serde_json` crate added to `Cargo.toml` | DONE | `Cargo.toml` |
| `watch: Vec<String>` field on `Step` | TODO | `src/config.rs` |
| File content hashing mixed into step hash | TODO | `src/sandbox/cache.rs` |
| Cache TTL / expiry with timestamp metadata | TODO | `src/sandbox/cache.rs` |
| `zenith cache list` command | TODO | `src/cli.rs` |
| `zenith cache clean` command | TODO | `src/cli.rs` |
| `outputs: Vec<String>` on `Step` | TODO | `src/config.rs` |
| Artifact tar/restore on cache hit | TODO | `src/sandbox/cache.rs`, `src/runner.rs` |
| `zenith build` command + `--no-cache` flag | TODO | `src/cli.rs`, `src/main.rs` |
| Cross-job cache sharing | TODO | `src/sandbox/cache.rs` |

---

## Phase 7 — Env & Package System

**Status:** NOT STARTED

| Task | Status |
|---|---|
| `EnvConfig` struct + `env` block in `.zenith.yml` | TODO |
| `src/toolchain/` module + `Toolchain` trait | TODO |
| Node.js auto-download + PATH inject | TODO |
| Python auto-download + PATH inject | TODO |
| Go auto-download + PATH inject | TODO |
| Rust toolchain auto-download + PATH inject | TODO |
| `zenith env init/shell/list/clean` commands | TODO |
| Toolchain availability inside sandbox | TODO |

---

## Phase 8 — Plugin System

**Status:** NOT STARTED

| Task | Status |
|---|---|
| `src/plugin/` module + `plugin.toml` manifest | TODO |
| Plugin discovery (`~/.zenith/plugins/`) | TODO |
| JSON-RPC over stdio protocol | TODO |
| `PluginBackend` implementing `Backend` trait | TODO |
| `get_backend()` plugin fallthrough | TODO |
| `zenith plugin install/list/remove` | TODO |
| Reference plugin example | TODO |

---

## Phase 9 — Remote Runner

**Status:** NOT STARTED

| Task | Status |
|---|---|
| `RemoteConfig` + `~/.zenith/remotes.toml` | TODO |
| `zenith remote add/list/remove/status` | TODO |
| `--remote` flag on `zenith run` | TODO |
| SSH transport (`openssh` crate) | TODO |
| Project upload + live log streaming | TODO |
| `zenith-agent` binary target | TODO |
| Agent auto-bootstrap on remote | TODO |

---

## Phase 10 — Cloud Runtime

**Status:** NOT STARTED

| Task | Status |
|---|---|
| Cloud API client (`src/cloud/`) | TODO |
| `zenith cloud login/run/status/logs/cancel` | TODO |
| SSE log streaming + auto-reconnect | TODO |
| Project tar.gz packager for upload | TODO |

---

## Phases 11–15 — Platform & OS-Level Runtime

**Status:** NOT STARTED — see [phase_11_15.md](phase_11_15.md)

---

## What to Build Next (Phase 6 Tasks)

Priority order for the next coding session:

1. **`watch` field + file-content hashing** (`src/config.rs`, `src/sandbox/cache.rs`)
   - Add `watch: Option<Vec<String>>` to `Step`
   - Walk glob patterns, SHA-256 hash file contents, mix into step hash

2. **Cache TTL + timestamp metadata** (`src/sandbox/cache.rs`)
   - Write JSON blob on cache hit instead of `"SUCCESS"` string
   - `is_cached()` rejects entries older than `ttl_days`

3. **`zenith cache clean` + `zenith cache list`** (`src/cli.rs`, `src/main.rs`)

4. **`outputs` field + artifact save/restore** (`src/config.rs`, `src/sandbox/cache.rs`, `src/runner.rs`)

5. **`zenith build --no-cache`** (`src/cli.rs`, `src/runner.rs`)

---

## Guide Files

| Phase | Guide |
|---|---|
| 0 | [phase_0.md](phase_0.md) |
| 1 | [phase_1.md](phase_1.md) |
| 2 | [phase_2.md](phase_2.md) |
| 3 | [phase_3.md](phase_3.md) |
| 4 | [phase_4.md](phase_4.md) |
| 5 | [phase_5.md](phase_5.md) |
| 6 | [phase_6.md](phase_6.md) |
| 7 | [phase_7.md](phase_7.md) |
| 8 | [phase_8.md](phase_8.md) |
| 9–10 | [phase_9_10.md](phase_9_10.md) |
| 11–15 | [phase_11_15.md](phase_11_15.md) |
| Motto | [motto.md](motto.md) |
