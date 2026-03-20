# Zenith — Build Progress Tracker

Last updated: 2026-03-21 (Phases 12 + 13 complete — Phase 14 next)

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
Phase 6   [##########] 100%  Build & Cache System               COMPLETE
Phase 7   [##########] 100%  Env & Package System               COMPLETE
Phase 8   [##########] 100%  Plugin System                      COMPLETE
Phase 9   [##########] 100%  Remote Runner                      COMPLETE
Phase 10  [##########] 100%  Cloud Runtime                      COMPLETE
Phase 11  [##########] 100%  GUI & IDE Integration              COMPLETE
Phase 12  [##########] 100%  Low-Level System Optimization      COMPLETE
Phase 13  [##########] 100%  Reproducibility Engine             COMPLETE
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

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `CacheManager` struct | DONE | `src/sandbox/cache.rs` |
| SHA-256 step hash (cmd + env + os + arch) | DONE | `src/sandbox/cache.rs` |
| Cache hit check + skip step | DONE | `src/runner.rs` |
| Cache marker written on step success | DONE | `src/sandbox/cache.rs` |
| `glob` crate added to `Cargo.toml` | DONE | `Cargo.toml` |
| `serde_json` crate added to `Cargo.toml` | DONE | `Cargo.toml` |
| `watch: Vec<String>` field on `Step` | DONE | `src/config.rs` |
| File content hashing mixed into step hash | DONE | `src/sandbox/cache.rs` — `hash_watched_files()` |
| Cache TTL / expiry with JSON timestamp metadata | DONE | `src/sandbox/cache.rs` — `is_cached()`, `meta.json` |
| `zenith cache list` command | DONE | `src/cli.rs`, `src/main.rs` — `handle_cache()` |
| `zenith cache clean` / `zenith cache prune` | DONE | `src/main.rs` — `clean_all()`, `clean_expired()` |
| `outputs: Vec<String>` on `Step` | DONE | `src/config.rs` |
| Artifact tar.gz archive + restore on cache hit | DONE | `src/sandbox/cache.rs` — `archive_artifacts()`, `restore_artifacts()` |
| `zenith build` command + `--no-cache` flag | DONE | `src/cli.rs`, `src/main.rs`, `src/runner.rs` |
| `cache_key` manual override on `Step` | DONE | `src/config.rs`, `src/sandbox/cache.rs` |
| Cross-job cache sharing (shared `~/.zenith/cache/`) | DONE | `src/sandbox/cache.rs` — shared dir keyed by hash |

---

## Phase 7 — Env & Package System

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `EnvConfig` struct + `env:` block in `.zenith.yml` | DONE | `src/config.rs` |
| `toolchain:` per-job override in `Job` | DONE | `src/config.rs` |
| `src/toolchain/mod.rs` + `Toolchain` trait | DONE | `src/toolchain/mod.rs` |
| `resolve_toolchain_env()` — builds PATH prefix | DONE | `src/toolchain/mod.rs` |
| `list_installed()` / `clean_all()` management | DONE | `src/toolchain/mod.rs` |
| **Motto: Node.js auto-download + PATH inject** | DONE | `src/toolchain/node.rs` |
| **Motto: Python standalone auto-download + PATH inject** | DONE | `src/toolchain/python.rs` |
| **Motto: Go auto-download + PATH inject** | DONE | `src/toolchain/go.rs` |
| **Motto: Rust via rustup-init — isolated CARGO_HOME** | DONE | `src/toolchain/rust_tc.rs` |
| Toolchain PATH injected into step execution | DONE | `src/runner.rs` — `tool_env` merge |
| `zenith env init` — install all declared toolchains | DONE | `src/main.rs` — `handle_env()` |
| `zenith env shell` — spawn $SHELL with toolchain PATH | DONE | `src/main.rs` — `handle_env()` |
| `zenith env list` — show installed toolchains | DONE | `src/main.rs`, `src/toolchain/mod.rs` |
| `zenith env clean` — remove all toolchains | DONE | `src/main.rs`, `src/toolchain/mod.rs` |
| Toolchain availability inside sandbox (PATH bind) | FUTURE | Phase 11+ (container bind-mount) |

---

## Phase 8 — Plugin System

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| Architecture decision: external process + JSON-RPC | DONE | `src/plugin/mod.rs` — design comment |
| `PluginManifest` struct + `plugin.toml` parsing | DONE | `src/plugin/manifest.rs` |
| Plugin discovery (`~/.zenith/plugins/`) | DONE | `src/plugin/registry.rs` — `discover_plugins()` |
| `find_plugin(name)` lookup | DONE | `src/plugin/registry.rs` |
| `install_from_path()` — copy + validate + smoke test | DONE | `src/plugin/registry.rs` |
| `remove_plugin(name)` | DONE | `src/plugin/registry.rs` |
| `RpcRequest` / `RpcResponse` types | DONE | `src/plugin/protocol.rs` |
| `PluginBackend` implementing `Backend` trait | DONE | `src/plugin/client.rs` |
| JSON-RPC call over stdio (spawn → write → read → kill) | DONE | `src/plugin/client.rs` — `call()` |
| `smoke_test()` — calls `name` RPC on install | DONE | `src/plugin/client.rs` |
| `get_backend()` plugin fallthrough | DONE | `src/sandbox/mod.rs` |
| `zenith plugin list/install/remove/info` CLI | DONE | `src/cli.rs`, `src/main.rs` |
| Reference plugin in Rust | DONE | `examples/plugin-example/` |
| Plugin authoring guide | DONE | `docs/plugin_authoring.md` |

---

## Phase 9 — Remote Runner

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `RemoteEntry` struct + `~/.zenith/remotes.toml` | DONE | `src/remote/config.rs` |
| `add/list/remove/get_remote()` | DONE | `src/remote/config.rs` |
| `zenith remote add/list/remove/status` | DONE | `src/cli.rs`, `src/main.rs` |
| `--remote <name>` flag on `zenith run` | DONE | `src/main.rs` — wired to `runner::execute_remote` |
| SSH transport via system `ssh` binary | DONE | `src/remote/transport.rs` |
| `ping()` — check reachability + arch | DONE | `src/remote/transport.rs` |
| `package_project()` — tar.gz excluding noise dirs | DONE | `src/remote/transport.rs` |
| `upload_project()` — pipe tarball via SSH | DONE | `src/remote/transport.rs` |
| `bootstrap_agent()` — auto-install agent on remote | DONE | `src/remote/transport.rs` |
| `run_agent()` — stream logs back with `[remote:name]` prefix | DONE | `src/remote/transport.rs` |
| `zenith-agent` binary target | DONE | `src/agent/main.rs`, `Cargo.toml` [[bin]] |
| Shared library target (`src/lib.rs`) for dual-binary crate | DONE | `src/lib.rs` |
| `default-run = "zenith"` in Cargo.toml | DONE | `Cargo.toml` |

---

## Phase 10 — Cloud Runtime

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `RunStatus`, `RunInfo`, `CloudConfig` types | DONE | `src/cloud/types.rs` |
| `CloudClient` — full HTTP API surface | DONE | `src/cloud/client.rs` |
| `submit_run()` — multipart project upload | DONE | `src/cloud/client.rs` |
| `get_status()` / `list_runs()` / `cancel_run()` | DONE | `src/cloud/client.rs` |
| `stream_logs()` — SSE parsing + live print | DONE | `src/cloud/client.rs` |
| `load_cloud_config()` / `save_api_key()` / `clear_api_key()` | DONE | `src/cloud/client.rs` |
| `package_project()` — tar.gz builder | DONE | `src/cloud/packager.rs` |
| `zenith cloud login/logout/run/status/logs/cancel/list` | DONE | `src/cli.rs`, `src/main.rs` |
| `futures-util` + `reqwest multipart` deps | DONE | `Cargo.toml` |

---

---

## Phase 11 — GUI & IDE Integration

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `RunLogger` — persist run history to `~/.zenith/logs/<run-id>/` | DONE | `src/ui/history.rs` |
| `RunSummary` / `RunOutcome` / `StepEvent` / `StepStatus` types | DONE | `src/ui/history.rs` |
| `list_runs()` / `get_steps()` / `get_run()` reader API | DONE | `src/ui/history.rs` |
| Runner wired to write history on every `zenith run` | DONE | `src/runner.rs` — `RunLogger` integrated |
| Shell command output captured + stored as log lines | DONE | `src/runner.rs` — `run_shell_command()` returns lines |
| Axum HTTP server on port 7622 | DONE | `src/ui/server.rs` |
| REST endpoints: `/api/runs`, `/api/runs/:id`, `/api/runs/:id/steps` | DONE | `src/ui/api.rs` |
| SSE endpoint: `/api/runs/:id/stream` (replays history) | DONE | `src/ui/api.rs` |
| `/api/cache` and `/api/labs` endpoints | DONE | `src/ui/api.rs` |
| Embedded dark-theme single-page dashboard (HTML/JS) | DONE | `src/ui/dashboard.html` |
| Dashboard: run list + step detail, auto-refresh every 10 s | DONE | `src/ui/dashboard.html` |
| Dashboard: collapsible step log lines | DONE | `src/ui/dashboard.html` |
| `zenith ui [--port N]` CLI command | DONE | `src/cli.rs`, `src/main.rs` |
| ratatui two-pane TUI — run list (left) + steps (right) | DONE | `src/tui/mod.rs` |
| TUI: color-coded status dots + step tag badges | DONE | `src/tui/mod.rs` |
| TUI: expandable log pane (Enter), Tab focus switch, r refresh, q quit | DONE | `src/tui/mod.rs` |
| `zenith tui` CLI command | DONE | `src/cli.rs`, `src/main.rs` |
| VSCode extension — `package.json` with commands + menus | DONE | `vscode-zenith/package.json` |
| VSCode extension — TypeScript `extension.ts` | DONE | `vscode-zenith/src/extension.ts` |
| VSCode: `Zenith: Run`, `Run Job`, `Open Dashboard`, `Open TUI`, `Clean Cache` | DONE | `vscode-zenith/src/extension.ts` |
| VSCode: output channel + status bar item with run state | DONE | `vscode-zenith/src/extension.ts` |
| VSCode: embedded WebView dashboard proxy panel | DONE | `vscode-zenith/src/extension.ts` |
| VSCode: YAML diagnostics (warns when no `jobs:` block) | DONE | `vscode-zenith/src/extension.ts` |
| JSON Schema for `.zenith.yml` (autocomplete + validation) | DONE | `vscode-zenith/schemas/zenith-schema.json` |
| Language configuration for `.zenith.yml` | DONE | `vscode-zenith/language-configuration.json` |

---

---

## Phase 12 — Low-Level System Optimization

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `zenith-init` PID 1 binary — mounts pseudo-fs, reads command from serial, exec's it, reports exit, powers off | DONE | `src/init/main.rs` |
| `[[bin]] zenith-init` target in Cargo.toml | DONE | `Cargo.toml` |
| `ensure_zenith_kernel()` — download custom kernel to `~/.zenith/kernel/vmlinux-zenith` | DONE | `src/tools.rs` |
| `ensure_zenith_rootfs()` — download minimal rootfs to `~/.zenith/rootfs/zenith-minimal.tar.gz` | DONE | `src/tools.rs` |
| `LayerStore` — content-addressable rootfs layer store at `~/.zenith/layers/<sha256>/` | DONE | `src/sandbox/layer_store.rs` |
| `LayerStore::store_layer()` / `extract_layer()` / `prune()` / `list_layers()` | DONE | `src/sandbox/layer_store.rs` |
| 6 layer store unit tests | DONE | `src/sandbox/layer_store.rs` |
| `FirecrackerBackend` prefers custom kernel if present, falls back to stock | DONE | `src/sandbox/firecracker.rs` |
| Serial protocol `O:`/`E:`/`EXIT:` parsing + legacy `__ZENITH_EXIT__:` fallback | DONE | `src/sandbox/firecracker.rs` — `read_serial_output()` |
| **zenith-init stdin-pipe dispatch** — command sent via FC stdin (not boot cmdline) when `base_os == "zenith"` | DONE | `src/sandbox/firecracker.rs` — `execute()` |
| **VM snapshot warm-pool** — cold boot takes snapshot; subsequent runs restore in < 50ms | DONE | `src/sandbox/firecracker.rs` — `WARM_POOL`, `restore_vm_snapshot_piped()` |
| `create_vm_snapshot()` / `restore_vm_snapshot()` / `fc_resume_vm()` | DONE | `src/sandbox/firecracker.rs` |
| Custom Linux kernel config (`kernel/zenith.config`) — documented for manual build | DONE | `kernel/zenith.config` |
| Custom rootfs < 5MB — CDN artifact (`ensure_zenith_rootfs()` downloads it) | DONE | `src/tools.rs` |

---

## Phase 13 — Reproducibility Engine

**Status:** COMPLETE

| Task | Status | File |
|---|---|---|
| `depends_on: Vec<String>` field on `Step` | DONE | `src/config.rs` |
| `Derivation` model — Nix-style content-addressed build identity | DONE | `src/build/derivation.rs` |
| `Derivation::id()` — SHA-256 of deterministic JSON | DONE | `src/build/derivation.rs` |
| `Derivation::from_step()` + `with_deps()` | DONE | `src/build/derivation.rs` |
| 8 derivation unit tests | DONE | `src/build/derivation.rs` |
| `BuildStore` — content-addressable store at `~/.zenith/store/<drv-id>/` | DONE | `src/build/store.rs` |
| `BuildStore::commit()` / `restore()` / `gc()` / `list()` / `outputs_dir()` | DONE | `src/build/store.rs` |
| 5 build store unit tests | DONE | `src/build/store.rs` |
| `pub mod build` in `src/lib.rs` | DONE | `src/lib.rs` |
| Dependency-aware **parallel step executor** (JoinSet + dep graph) | DONE | `src/runner.rs` — `execute_single_job()` |
| Cycle / missing dep detection with warning | DONE | `src/runner.rs` — loop exit guard |
| `zenith build --derivation` dry-run (print derivation JSON) | DONE | `src/cli.rs`, `src/main.rs` — `print_derivations()` |
| `zenith store list/gc/info` CLI | DONE | `src/cli.rs`, `src/main.rs` — `handle_store()` |
| Runner: local store hit → restore outputs, skip execution | DONE | `src/runner.rs` — STORE HIT path |
| Runner: stage outputs + `BuildStore::commit()` on step success | DONE | `src/runner.rs` — `stage_step_outputs()` |
| `RemoteCacheClient` — HEAD / GET / PUT over HTTPS | DONE | `src/build/remote_cache.rs` |
| Runner: remote cache pull on local miss → populate local store | DONE | `src/runner.rs` — REMOTE HIT path |
| Runner: push to remote cache after local commit when `push = true` | DONE | `src/runner.rs` |
| `zenith cache remote [url] [--push] [--status]` CLI | DONE | `src/cli.rs`, `src/main.rs` |
| `RemoteCacheConfig` persisted to `~/.zenith/config.toml` `[cache]` section | DONE | `src/build/remote_cache.rs` |
| 2 remote cache unit tests (roundtrip pack/unpack, default config) | DONE | `src/build/remote_cache.rs` |

---

## Phases 14–15 — Platform & OS-Level Runtime

**Status:** NOT STARTED — see individual phase files

---

## What to Build Next (Phase 14 — Full Developer Platform)

Phase 13 is complete. Priority order for Phase 14:

1. **`zenith build --lock`** — write a `zenith.lock` file with all derivation IDs for a fully reproducible build graph snapshot
2. **`zenith publish`** — publish a build artifact to an OCI registry or package feed
3. **Secrets management** — `secrets:` block in `.zenith.yml`; values fetched from env, files, or Vault at runtime; never stored in the derivation hash
4. **Notification hooks** — `on_success:` / `on_failure:` webhook or Slack integration per job
5. **Resource limits** (Phase 12 remainder) — `resources: { cpu: 2, memory: 512m }` per job via cgroups

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
| Plugin authoring | [plugin_authoring.md](plugin_authoring.md) |
| 9 | [phase_9.md](phase_9.md) |
| 10 | [phase_10.md](phase_10.md) |
| 11 | [phase_11.md](phase_11.md) |
| 12 | [phase_12.md](phase_12.md) |
| 13 | [phase_13.md](phase_13.md) |
| 14 | [phase_14.md](phase_14.md) |
| 15 | [phase_15.md](phase_15.md) |
| Motto | [motto.md](motto.md) |
