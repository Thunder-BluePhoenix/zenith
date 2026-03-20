# Zenith User Guide

**You install Zenith. Zenith installs everything else.**

This guide covers the complete Zenith feature set — from first run through advanced caching, reproducible builds, remote execution, cloud offload, the web dashboard, and the background daemon.

---

## Table of Contents

1. [Installation](#installation)
2. [First Workflow](#first-workflow)
3. [Configuration Reference](#configuration-reference)
4. [Commands Reference](#commands-reference)
5. [Caching](#caching)
6. [Content-Addressable Build Store](#content-addressable-build-store)
7. [Parallel Step Execution](#parallel-step-execution)
8. [Toolchains](#toolchains)
9. [Sandbox Backends](#sandbox-backends)
10. [Matrix Builds](#matrix-builds)
11. [Lab Environments](#lab-environments)
12. [Remote Build Machines](#remote-build-machines)
13. [Cloud Service](#cloud-service)
14. [Dashboard & TUI](#dashboard--tui)
15. [Plugin System](#plugin-system)
16. [Background Daemon](#background-daemon)
17. [Low-Level Tools](#low-level-tools)
18. [Migrating to Schema v2](#migrating-to-schema-v2)
19. [Platform Notes](#platform-notes)
20. [License](#license)

---

## Installation

```bash
git clone <repo>
cd zenith
cargo install --path .
```

This produces three binaries: `zenith` (CLI), `zenith-agent` (remote runner), `zenith-daemon` (background service).

Verify:

```bash
zenith --version
zenith tools status
```

Zenith stores everything in `~/.zenith/` — nothing is written to system directories.

---

## First Workflow

Create `.zenith.yml` in your project root:

```yaml
version: "2"

jobs:
  build:
    runs-on: alpine
    steps:
      - name: Build
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/release/myapp]

      - name: Test
        run: cargo test
        depends_on: [Build]
```

Run it:

```bash
zenith run
```

Run it again — `Build` is restored from cache, `Test` re-runs (or is also cached if source unchanged):

```bash
zenith run
# [build] [CACHED] Build
# [build] Running: cargo test
```

---

## Configuration Reference

### Top-level fields

```yaml
version: "2"          # schema version: "1" (legacy) or "2" (all features)

env:                   # global toolchain declarations
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "stable"

cache:                 # top-level cache settings (v2 only)
  ttl_days: 14         # entries older than this are pruned (default: 7)
  remote: "https://cache.example.com"   # remote binary cache URL
  push: true           # auto-push build outputs after successful build

jobs:
  my-job: { ... }
```

### Job fields

| Field | Type | Default | Description |
|---|---|---|---|
| `runs-on` | string | `alpine` | OS image for the sandbox |
| `backend` | string | `container` | Isolation engine: `container`, `firecracker`, `wasm`, `wine`, or plugin name |
| `arch` | string | host arch | Target CPU: `x86_64`, `aarch64` — cross-arch auto-downloads QEMU |
| `env` | map | — | Environment variables for all steps in this job |
| `working_directory` | string | — | Default working directory for all steps |
| `strategy` | object | — | Matrix expansion config |
| `toolchain` | object | — | Per-job toolchain overrides (same fields as top-level `env:`) |
| `cache` | bool | `true` | Enable/disable step caching for all steps in this job |

### Step fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | — | Step label — also used as a dependency target in `depends_on` |
| `run` | string | required | Shell command to execute |
| `env` | map | — | Additional env vars for this step only |
| `working_directory` | string | — | Override working directory for this step |
| `cache` | bool | `true` | Enable/disable caching for this step |
| `cache_key` | string | — | Manual cache key (overrides auto-hash; useful for cross-OS sharing) |
| `watch` | list | `[]` | Glob patterns — cache is invalidated when matching files change |
| `outputs` | list | `[]` | Paths to archive on success and restore on cache hit |
| `allow_failure` | bool | `false` | Pipeline continues even if this step exits non-zero |
| `depends_on` | list | `[]` | Step names that must complete before this step starts |

### Full v2 example

```yaml
version: "2"

env:
  rust: stable

cache:
  ttl_days: 14
  remote: "https://cache.myteam.example.com"
  push: true

jobs:
  ci:
    runs-on: alpine
    backend: firecracker
    strategy:
      matrix:
        os: [alpine, ubuntu]
    steps:
      - name: Install deps
        run: apk add --no-cache git
        watch: []

      - name: Build
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml, Cargo.lock]
        outputs: [target/release/myapp]
        depends_on: [Install deps]

      - name: Unit tests
        run: cargo test --lib
        depends_on: [Build]

      - name: Integration tests
        run: cargo test --test '*'
        depends_on: [Build]
        allow_failure: true
```

`Unit tests` and `Integration tests` both depend only on `Build`, so they run in parallel.

---

## Commands Reference

### `zenith run`

Run the workflow defined in `.zenith.yml`.

```bash
zenith run                     # run the first/only job
zenith run --job test          # run a specific named job
zenith run --no-cache          # bypass cache, re-run all steps
zenith run --remote myserver   # dispatch to a registered remote machine
```

When the Zenith daemon is running, `zenith run` connects to it automatically for near-zero startup latency. Falls back to standalone mode if the daemon is unavailable.

### `zenith build`

Like `run`, but semantically for build jobs. Checks the content-addressable build store before executing.

```bash
zenith build                          # build the first job
zenith build --job compile            # build a specific job
zenith build --no-cache               # skip cache, rebuild from scratch
zenith build --derivation             # dry-run: print derivation JSON, do not execute
```

### `zenith migrate`

Upgrade a v1 `.zenith.yml` to schema v2.

```bash
zenith migrate             # print upgraded config to stdout
zenith migrate --write     # upgrade in-place
```

### `zenith cache`

```bash
zenith cache list                    # show all cache entries (hash, age, OS, arch, command)
zenith cache prune                   # remove entries older than TTL
zenith cache clean                   # delete all entries — next run rebuilds from scratch
zenith cache remote <url>            # configure remote binary cache URL
zenith cache remote <url> --push     # configure URL and enable auto-push
zenith cache remote --status         # show current remote cache configuration
```

### `zenith store`

Inspect the content-addressable build store at `~/.zenith/store/`.

```bash
zenith store list            # list all stored derivations with timestamps
zenith store info <id>       # show the derivation that produced a store entry
zenith store gc [days]       # remove entries older than N days (default: 30)
```

### `zenith env`

```bash
zenith env init      # download all toolchains declared in .zenith.yml env:
zenith env list      # show installed toolchains and their paths
zenith env shell     # open a shell with Zenith toolchains on PATH
zenith env clean     # remove all downloaded toolchains
```

### `zenith lab`

```bash
zenith lab list                   # list active lab environments
zenith lab create [os]            # provision a new sandbox (default: alpine)
zenith lab shell [os]             # open interactive shell in the sandbox
zenith lab run <os> <command>     # run a one-off command in the sandbox
zenith lab push [os]              # copy project files into the sandbox workspace
zenith lab destroy <os>           # teardown and delete the sandbox
```

### `zenith matrix`

```bash
zenith matrix run    # run all matrix combinations in parallel
zenith matrix list   # list all jobs and their matrix expansions
```

### `zenith shell`

```bash
zenith shell               # open host shell with Zenith toolchains on PATH
zenith shell --lab alpine  # open shell inside the alpine sandbox
```

### `zenith plugin`

```bash
zenith plugin list                   # list installed plugins
zenith plugin install ./my-dir       # install plugin from a local directory
zenith plugin install <name>         # install from the Zenith registry (Phase 14)
zenith plugin remove <name>          # uninstall a plugin
zenith plugin info <name>            # show full plugin manifest
zenith plugin search <query>         # search the hosted plugin registry
```

### `zenith remote`

```bash
zenith remote add <name> <user@host> [--port N] [--key /path/to/key]
zenith remote list
zenith remote remove <name>
zenith remote status <name>          # ping and show arch
```

### `zenith cloud`

```bash
zenith cloud login <api-key>         # save API key
zenith cloud logout                  # remove API key
zenith cloud run [--job name] [--watch]
zenith cloud status <run-id>
zenith cloud logs <run-id>
zenith cloud cancel <run-id>
zenith cloud list
```

### `zenith ui`

```bash
zenith ui             # start web dashboard on port 7622
zenith ui --port 9000 # use a custom port
```

Open `http://localhost:7622` in any browser.

### `zenith tui`

```bash
zenith tui            # open terminal (TUI) dashboard
```

Keys: `Tab` switch pane, `Enter` expand step logs, `r` refresh, `q` quit.

### `zenith tools`

```bash
zenith tools download-kernel   # download Zenith custom kernel → ~/.zenith/kernel/vmlinux-zenith
zenith tools download-rootfs   # download Zenith minimal rootfs → ~/.zenith/rootfs/zenith-minimal.tar.gz
zenith tools status            # show paths and sizes of all downloaded tools
```

### `zenith daemon`

```bash
zenith daemon start [--pool N]       # start the background daemon (N pre-warmed VMs)
zenith daemon stop                   # gracefully shut down the daemon
zenith daemon status                 # show pool sizes, active jobs, uptime
zenith daemon restart [--pool N]     # stop and restart
zenith daemon hypervisor-check       # check whether KVM is available on this machine
```

### `zenith benchmark`

```bash
zenith benchmark                     # run performance benchmarks, compare vs saved baseline
zenith benchmark --save-baseline     # run and save results as the new baseline
```

### `zenith docs`

```bash
zenith docs           # open the Zenith documentation site in the browser
```

---

## Caching

Zenith caches at the step level. A cache entry is keyed on a SHA-256 hash of:

- OS and architecture
- All environment variables for the step
- The `run` command text
- Contents of every file matched by `watch:` globs

### Cache hit behaviour

When a step's hash matches a prior run:
- The step is skipped (`[CACHED]`)
- If the step declared `outputs:`, those files are restored from the saved archive

### Watching files

```yaml
steps:
  - name: Install deps
    run: npm ci
    watch:
      - package.json
      - package-lock.json
```

### Saving build artifacts

```yaml
steps:
  - name: Compile
    run: cargo build --release
    watch: [src/**/*.rs, Cargo.toml]
    outputs:
      - target/release/myapp    # archived on success, restored on cache hit
```

### Remote binary cache

Share cache hits across machines. One developer's build warms CI — CI warms every other developer.

```bash
# Configure once:
zenith cache remote https://cache.myteam.example.com --push

# Or in .zenith.yml (v2):
cache:
  remote: "https://cache.myteam.example.com"
  push: true
```

Before each step execution, Zenith checks:
1. Local store hit → restore, skip execution
2. Remote cache hit → download, populate local store, skip execution
3. Execute → commit to local store → optionally push to remote

---

## Content-Addressable Build Store

The build store (`~/.zenith/store/`) is Zenith's reproducibility engine. Every successful build step with `outputs:` is stored, keyed by a **derivation** — a hash of all inputs (command, env, OS, arch, watched files).

The same derivation on any machine always produces the same hash. Two projects whose build steps are identical share one store entry.

### Derivations

A derivation captures everything that uniquely identifies a build:

```bash
zenith build --derivation    # print the derivation JSON for each step
```

```json
{
  "name": "Build",
  "command": "cargo build --release",
  "os": "alpine",
  "arch": "x86_64",
  "inputs": ["src/main.rs", "Cargo.toml"],
  "outputs": ["target/release/myapp"],
  "env": {}
}
```

### Store commands

```bash
zenith store list            # all stored derivations with build timestamps
zenith store info <id>       # full derivation details for a store entry
zenith store gc 30           # remove entries not accessed in 30 days
```

---

## Parallel Step Execution

Steps with `depends_on:` run as soon as all named dependencies complete. Steps with no unfulfilled dependencies start immediately and run concurrently.

```yaml
steps:
  - name: Install
    run: npm install
    outputs: [node_modules/]

  - name: Lint
    run: npm run lint
    depends_on: [Install]

  - name: Test
    run: npm test
    depends_on: [Install]

  # Lint and Test run in parallel — both depend on Install but not each other

  - name: Build
    run: npm run build
    depends_on: [Lint, Test]   # waits for both
```

**Cycle detection:** If a cycle exists in `depends_on`, Zenith warns and aborts rather than deadlocking.

---

## Toolchains

Declare exact versions in `.zenith.yml` — Zenith downloads and caches them in `~/.zenith/toolchains/`. These binaries are prepended to `PATH` before every step.

```yaml
env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "stable"
```

So `node`, `python3`, `go`, `cargo` all resolve to the declared versions — no system installs needed, no version conflicts.

Per-job override (takes priority over the top-level `env:` block):

```yaml
jobs:
  legacy:
    toolchain:
      node: "16.0.0"
```

Download all toolchains upfront (useful at the start of a CI run):

```bash
zenith env init
```

---

## Sandbox Backends

### `container` (default)

Linux namespace isolation — no Docker, no root, no KVM required.

```yaml
backend: container
runs-on: alpine
```

### `firecracker`

Hardware-level microVM isolation using AWS Firecracker. Requires Linux with `/dev/kvm`.

```yaml
backend: firecracker
runs-on: alpine
```

With the Zenith custom kernel (`runs-on: zenith`), VMs boot in under 50 ms:

```yaml
backend: firecracker
runs-on: zenith   # uses zenith-init PID 1, custom kernel, minimal rootfs
```

Zenith downloads the Firecracker binary, kernel, and rootfs automatically.

### `wasm`

Run `.wasm` modules via wasmtime. Cross-platform, zero-OS overhead.

```yaml
backend: wasm
steps:
  - name: Run Wasm binary
    run: my-app.wasm --flag
```

### `wine`

Run Windows `.exe` binaries on Linux using an isolated Wine prefix.

```yaml
backend: wine
steps:
  - name: Run Windows tool
    run: mytool.exe --flag
```

### Custom plugin backend

Any installed plugin can act as a backend:

```yaml
backend: my-plugin-name
```

---

## Matrix Builds

Run the same job across multiple configurations in parallel. Every combination gets its own isolated workspace and cache entry.

```yaml
jobs:
  test:
    strategy:
      matrix:
        os: [alpine, ubuntu]
        node: ["18", "20", "22"]
    runs-on: ${{ matrix.os }}
    steps:
      - name: Test Node ${{ matrix.node }} on ${{ matrix.os }}
        run: node --version
        env:
          NODE_VERSION: ${{ matrix.node }}
```

This produces 6 parallel matrix instances (2 OS × 3 Node versions).

```bash
zenith matrix list    # see all expanded combinations
zenith matrix run     # run all combinations
```

---

## Lab Environments

Labs are persistent, interactive sandboxes for manual exploration or debugging.

```bash
# Create an alpine sandbox
zenith lab create alpine

# Copy project files into it
zenith lab push alpine

# Open an interactive shell
zenith lab shell alpine

# Run a one-off command
zenith lab run alpine "cat /etc/os-release"

# Clean up when done
zenith lab destroy alpine
```

On Linux, labs use OverlayFS — the base rootfs is never modified; each lab has its own writable upper layer.

---

## Remote Build Machines

Run workflows on registered SSH machines. Useful for hardware-specific builds (Linux from macOS, or a beefy build server).

```bash
# Register a remote
zenith remote add build-server deploy@192.168.1.10 --key ~/.ssh/id_rsa

# Check it's reachable
zenith remote status build-server

# Run on it
zenith run --remote build-server
```

Zenith automatically uploads your project, installs `zenith-agent` on the remote if needed, streams logs back in real time, and cleans up.

```bash
zenith remote list
zenith remote remove build-server
```

---

## Cloud Service

Submit workflows to the Zenith cloud service for fully managed remote execution.

```bash
# Authenticate
zenith cloud login <your-api-key>

# Submit and watch
zenith cloud run --watch

# Submit and monitor separately
zenith cloud run
zenith cloud status <run-id>
zenith cloud logs <run-id>

# List recent runs
zenith cloud list

# Cancel a run
zenith cloud cancel <run-id>

# Remove credentials
zenith cloud logout
```

---

## Dashboard & TUI

### Web Dashboard

```bash
zenith ui            # starts on http://localhost:7622
zenith ui --port N   # custom port
```

The dashboard shows:
- All past and live runs with step-level status
- Collapsible step log output
- Cache statistics
- Active lab environments

### Terminal Dashboard (TUI)

```bash
zenith tui
```

A fullscreen ratatui dashboard. Left pane: run list. Right pane: steps for the selected run.

| Key | Action |
|---|---|
| `Tab` | Switch pane focus |
| `↑ / ↓` | Navigate list |
| `Enter` | Expand/collapse step log |
| `r` | Refresh |
| `q` | Quit |

---

## Plugin System

Zenith's backend is extensible. Plugins are external binaries communicating over JSON-RPC on stdio — write them in any language.

### Installing plugins

```bash
# From a local directory
zenith plugin install ./path/to/plugin-dir

# Search the hosted registry
zenith plugin search <query>

# Install from registry by name
zenith plugin install <registry-name>
```

### Plugin manifest (`plugin.toml`)

```toml
[plugin]
name            = "my-backend"
version         = "1.0.0"
type            = "backend"
entrypoint      = "my-backend-bin"
description     = "Custom execution backend"
requires_zenith = ">=0.1.0"   # version constraint — enforced on install
```

`requires_zenith` is checked at install time. If the running Zenith version doesn't satisfy the constraint, installation is aborted with a clear error.

### Using a plugin as a backend

```yaml
backend: my-backend
```

### Managing plugins

```bash
zenith plugin list                   # list installed plugins
zenith plugin info <name>            # show manifest details
zenith plugin remove <name>          # uninstall
```

See [docs/plugin_authoring.md](docs/plugin_authoring.md) for the full protocol spec and a reference Rust implementation.

---

## Background Daemon

The Zenith daemon is a long-running background service that maintains a pool of pre-warmed VMs. When the daemon is running, `zenith run` connects to it instead of cold-booting — eliminating startup latency.

### Starting the daemon

```bash
zenith daemon start             # start with default pool (2 VMs)
zenith daemon start --pool 4    # start with 4 pre-warmed VMs
```

Zenith `run` and `build` automatically connect to the daemon if it's running. No changes to your workflow files are needed.

### Daemon commands

```bash
zenith daemon status            # pool sizes, active jobs, uptime
zenith daemon stop              # graceful shutdown
zenith daemon restart           # stop and restart
zenith daemon hypervisor-check  # check KVM availability
```

### KVM hypervisor

The daemon uses Zenith's custom KVM-based hypervisor on Linux (requires `/dev/kvm`):

```bash
zenith daemon hypervisor-check
# KVM hypervisor: AVAILABLE
# The Zenith custom VMM is supported on this machine.
```

On non-Linux hosts or machines without KVM, the daemon falls back to the Firecracker or container backend automatically.

### Warm VM pool

The pool maintains N VM snapshots in memory. When a job arrives:
1. A pre-warmed snapshot is restored (< 1 ms)
2. The job's command is dispatched to `zenith-init` over vsock
3. The pool immediately begins warming a replacement VM

If the pool is empty (cold start), a VM is booted normally while a replacement warms in the background.

---

## Low-Level Tools

### Zenith custom kernel

A stripped-down Linux kernel compiled specifically for CI workloads:
- Disabled: sound, USB, Bluetooth, wireless, most drivers
- Enabled: virtio, 9p, overlayfs, KVM guest, minimal network, vsock
- Boot time: < 50 ms from kernel start to first step executing

```bash
zenith tools download-kernel    # downloads to ~/.zenith/kernel/vmlinux-zenith
```

### zenith-init (PID 1)

A purpose-built PID 1 binary for CI VMs. It:
1. Mounts `/proc`, `/sys`, `/dev`
2. Opens a vsock channel to the host Zenith process
3. Receives the step command
4. Executes it with `execve`, forwarding stdout/stderr
5. Reports the exit code
6. Powers off the VM with `reboot(RB_POWER_OFF)`

No shell, no SSH, no extra processes — just the one command and clean shutdown.

### Minimal rootfs

A custom rootfs under 5 MB (smaller than Alpine's default 3 MB):

```bash
zenith tools download-rootfs    # downloads to ~/.zenith/rootfs/zenith-minimal.tar.gz
```

Use with `runs-on: zenith` to get the full performance stack.

### Tools status

```bash
zenith tools status
# Artefact                         Size  Path
# ───────────────────────────────────────────────────────────────────────
# Zenith custom kernel           8.2MB  ~/.zenith/kernel/vmlinux-zenith
# Zenith minimal rootfs          4.8MB  ~/.zenith/rootfs/zenith-minimal.tar.gz
# Firecracker VMM               26.1MB  ~/.zenith/bin/firecracker
```

---

## Migrating to Schema v2

Schema v2 (added in Phase 14) makes all features explicit and adds the top-level `cache:` block.

### Automatic migration

```bash
zenith migrate             # preview the upgraded config
zenith migrate --write     # upgrade .zenith.yml in-place
```

### What changes

| v1 | v2 |
|---|---|
| `version: "1"` | `version: "2"` |
| No `cache:` block | `cache: { ttl_days: 7 }` (default values) |
| `runs_on:` | `runs-on:` (consistent with GitHub Actions convention) |

All v1 configs are still valid — the default version is `"1"` and everything is backward compatible.

---

## Platform Notes

### Linux

Full feature support: namespace isolation, OverlayFS, Firecracker, custom kernel, `zenith-init`, QEMU cross-arch, Wine, daemon with KVM hypervisor, all toolchain backends.

### macOS

- Local execution and toolchain management work fully
- Namespace isolation falls back to a restricted subprocess
- Firecracker, QEMU cross-arch, and Wine are not available
- Daemon runs without the KVM hypervisor (pool is disabled; standalone mode used)
- Wasm backend works fully

### Windows

- Local execution and toolchain management work
- Node.js, Python, Go toolchains downloaded as Windows binaries
- Namespace isolation falls back to a restricted subprocess
- Firecracker, QEMU, and Wine are not available
- Daemon communicates over TCP port 7623 instead of a Unix socket
- Wasm backend works fully

---

## License

Zenith is dual-licensed:

- **MIT License** — https://opensource.org/licenses/MIT
- **Apache License 2.0** — https://www.apache.org/licenses/LICENSE-2.0

You may use Zenith under either license at your option.

Any contribution submitted for inclusion in Zenith shall be dual-licensed as above, without additional terms or conditions.
