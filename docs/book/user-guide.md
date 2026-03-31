# Complete User Guide

**You install Zenith. Zenith installs everything else.**

This page is the single-page consolidated reference for Zenith. For deep dives into individual topics, see the dedicated chapters in the sidebar.

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

Run it again — `Build` is restored from cache, `Test` re-runs only if source changed:

```bash
zenith run
# [build] [CACHED] Build
# [build] Running: cargo test
```

---

## Configuration Reference

### Top-level fields

```yaml
version: "2"          # "1" (legacy) or "2" (all features)

env:                   # global toolchain versions
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "stable"

cache:                 # top-level cache settings (v2 only)
  ttl_days: 14
  remote: "https://cache.example.com"
  push: true

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
| `toolchain` | object | — | Per-job toolchain overrides |
| `cache` | bool | `true` | Enable/disable step caching for all steps |

### Step fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | — | Step label — also used as a `depends_on` target |
| `run` | string | required | Shell command to execute |
| `env` | map | — | Additional env vars for this step only |
| `working_directory` | string | — | Override working directory |
| `cache` | bool | `true` | Enable/disable caching for this step |
| `cache_key` | string | — | Manual cache key (overrides auto-hash) |
| `watch` | list | `[]` | Glob patterns — cache invalidated when these files change |
| `outputs` | list | `[]` | Paths to archive on success and restore on cache hit |
| `allow_failure` | bool | `false` | Pipeline continues even if this step exits non-zero |
| `depends_on` | list | `[]` | Step names that must complete before this step starts |

---

## Parallel Step Execution

Steps with `depends_on:` run as soon as all named dependencies complete. Steps with no pending dependencies start immediately and run concurrently.

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
  
  # Lint and Test run in parallel — both depend only on Install

  - name: Build
    run: npm run build
    depends_on: [Lint, Test]   # waits for both
```

**Cycle detection:** If a cycle exists in `depends_on`, Zenith warns and aborts rather than deadlocking.

---

## Caching

Zenith caches at the step level. The cache key is a SHA-256 hash of: OS, architecture, all env vars, the `run` command, and the contents of all `watch:` files.

### Cache hit behaviour

- Step is skipped (`[CACHED]`)
- If the step had `outputs:`, those files are restored from the saved archive

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
      - target/release/myapp
```

### Remote binary cache

```bash
zenith cache remote https://cache.myteam.example.com --push
```

Or in `.zenith.yml`:

```yaml
cache:
  remote: "https://cache.myteam.example.com"
  push: true
```

Before each step, Zenith checks: local store → remote cache → execute.

---

## Content-Addressable Build Store

Every successful step with `outputs:` is committed to `~/.zenith/store/` under a deterministic derivation ID. The same derivation on any machine always produces the same hash.

```bash
zenith build --derivation    # print derivation JSON, do not execute
zenith store list            # all stored derivations
zenith store info <id>       # full derivation details
zenith store gc 30           # remove entries older than 30 days
```

---

## Toolchains

Declare exact versions — Zenith downloads and injects them into `PATH` before every step.

```yaml
env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "stable"
```

Per-job override:

```yaml
jobs:
  legacy:
    toolchain:
      node: "16.0.0"
```

```bash
zenith env init       # download all declared toolchains
zenith env list       # show installed toolchains
zenith env shell      # open a shell with toolchains on PATH
zenith env clean      # remove all toolchains
```

---

## Sandbox Backends

### `container` (default)

Linux namespace isolation. No Docker, no root, no KVM.

### `firecracker`

Hardware-level microVM. Requires Linux + `/dev/kvm`.

```yaml
backend: firecracker
runs-on: zenith   # custom kernel + zenith-init for < 50ms boot
```

### `wasm`

Run `.wasm` modules via auto-downloaded wasmtime. Cross-platform.

### `wine`

Run Windows `.exe` files on Linux via an isolated Wine prefix.

---

## Matrix Builds

```yaml
strategy:
  matrix:
    os: [alpine, ubuntu]
    node: ["18", "20", "22"]
```

Produces 6 parallel instances. Each has its own workspace and cache entry.

```bash
zenith matrix run     # run all combinations
zenith matrix list    # preview all combinations
```

---

## Lab Environments

Persistent interactive sandboxes for manual exploration:

```bash
zenith lab create alpine
zenith lab push alpine
zenith lab shell alpine
zenith lab run alpine "cat /etc/os-release"
zenith lab destroy alpine
```

---

## Remote Build Machines

```bash
zenith remote add build-server deploy@192.168.1.10 --key ~/.ssh/id_rsa
zenith remote status build-server
zenith run --remote build-server
```

---

## Cloud Service

```bash
zenith cloud login <api-key>
zenith cloud run --watch
zenith cloud status <run-id>
zenith cloud logs <run-id>
zenith cloud list
zenith cloud cancel <run-id>
```

---

## Dashboard & TUI

```bash
zenith ui            # web dashboard on http://localhost:7622
zenith tui           # terminal dashboard
```

TUI keys: `Tab` switch pane · `↑↓` navigate · `Enter` expand logs · `r` refresh · `q` quit

---

## Plugin System

```bash
zenith plugin search <query>           # search hosted registry
zenith plugin install <name>           # install from registry
zenith plugin install ./local-dir      # install from local path
zenith plugin list
zenith plugin remove <name>
zenith plugin info <name>
```

Plugin manifest (`plugin.toml`) supports `requires_zenith = ">=0.1.0"` — enforced at install time.

Use a plugin as a backend:

```yaml
backend: my-plugin-name
```

---

## Background Daemon

```bash
zenith daemon start [--pool N]        # start with N pre-warmed VMs
zenith daemon stop
zenith daemon status
zenith daemon hypervisor-check        # check KVM availability
```

`zenith run` connects to the daemon automatically when it is running. Falls back to standalone mode if unavailable.

---

## Migrating to Schema v2

```bash
zenith migrate             # preview the upgraded config
zenith migrate --write     # upgrade .zenith.yml in-place
```

| v1 | v2 |
|---|---|
| `version: "1"` | `version: "2"` |
| No `cache:` block | `cache: { ttl_days: 7 }` |
| `runs_on:` | `runs-on:` |

---

## Commands Quick Reference

| Command | Description |
|---|---|
| `zenith run [--job X] [--no-cache] [--remote R]` | Run workflow |
| `zenith build [--job X] [--no-cache] [--derivation]` | Build job |
| `zenith migrate [--write]` | Upgrade config to v2 |
| `zenith cache list / prune / clean` | Manage step cache |
| `zenith cache remote <url> [--push] [--status]` | Configure remote cache |
| `zenith store list / gc / info <id>` | Manage build store |
| `zenith env init / list / shell / clean` | Manage toolchains |
| `zenith lab create / shell / run / push / destroy` | Lab sandboxes |
| `zenith matrix run / list` | Matrix builds |
| `zenith remote add / list / remove / status` | Remote machines |
| `zenith cloud login / run / status / logs / cancel / list` | Cloud service |
| `zenith plugin search / install / remove / list / info` | Plugin management |
| `zenith ui [--port N]` | Web dashboard |
| `zenith tui` | Terminal dashboard |
| `zenith daemon start / stop / status / restart / hypervisor-check` | Background daemon |
| `zenith tools download-kernel / download-rootfs / status` | Low-level tools |
| `zenith benchmark [--save-baseline]` | Performance benchmarks |
| `zenith docs` | Open documentation in browser |

---

## Platform Notes

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Namespace isolation | Full | Fallback subprocess | Fallback subprocess |
| Firecracker backend | ✓ | ✗ | ✗ |
| KVM daemon pool | ✓ | ✗ | ✗ |
| Wasm backend | ✓ | ✓ | ✓ |
| Wine backend | ✓ | ✗ | ✗ |
| Cross-arch via QEMU | ✓ | ✗ | ✗ |
| Toolchain management | ✓ | ✓ | ✓ |
| Remote / Cloud | ✓ | ✓ | ✓ |
| Daemon socket | Unix socket | Unix socket | TCP port 7623 |
