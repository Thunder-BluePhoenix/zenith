# Zenith User Guide

**You install Zenith. Zenith installs everything else.**

This guide covers everything you need to run workflows with Zenith — from a first run to advanced caching, toolchain management, and writing plugins.

---

## Table of Contents

1. [Installation](#installation)
2. [First Workflow](#first-workflow)
3. [Configuration Reference](#configuration-reference)
4. [Commands](#commands)
5. [Caching](#caching)
6. [Toolchains](#toolchains)
7. [Sandbox Backends](#sandbox-backends)
8. [Matrix Builds](#matrix-builds)
9. [Lab Environments](#lab-environments)
10. [Plugin System](#plugin-system)
11. [Platform Notes](#platform-notes)
12. [License](#license)

---

## Installation

```bash
git clone <repo>
cd zenith
cargo install --path .
```

Verify:

```bash
zenith --version
```

Zenith stores all its data in `~/.zenith/` — nothing is written to system directories.

---

## First Workflow

Create `.zenith.yml` in your project root:

```yaml
version: "1"

jobs:
  hello:
    runs_on: local
    steps:
      - name: Say hello
        run: echo "Hello from Zenith"
```

Run it:

```bash
zenith run
```

Run it again — the step will be skipped from cache:

```bash
zenith run
# [hello] [CACHED] Step 1: Say hello
```

---

## Configuration Reference

### Top-level fields

```yaml
version: "1"          # required

env:                   # optional — declare language toolchain versions
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "1.78.0"

jobs:
  my-job: { ... }
```

### Job fields

| Field | Type | Default | Description |
|---|---|---|---|
| `runs_on` | string | `local` | Execution target. `local` = host machine. Other values provision a sandbox. |
| `backend` | string | `container` | Isolation engine: `container`, `firecracker`, `wasm`, `wine`, or a plugin name. |
| `arch` | string | host arch | Target CPU: `x86_64`, `aarch64`. Cross-arch auto-downloads QEMU. |
| `env` | map | — | Environment variables for all steps in this job. |
| `working_directory` | string | — | Default working directory for all steps. |
| `strategy` | object | — | Matrix expansion config. |
| `toolchain` | object | — | Per-job toolchain version overrides (same fields as top-level `env:`). |
| `cache` | bool | `true` | Enable/disable caching for all steps in this job. |

### Step fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | — | Human-readable step label. |
| `run` | string | required | Shell command to execute. |
| `env` | map | — | Additional env vars for this step only. |
| `working_directory` | string | — | Override working directory for this step. |
| `cache` | bool | `true` | Override caching for this step. |
| `cache_key` | string | — | Manual cache key (overrides auto hash). |
| `watch` | list | `[]` | Glob patterns — cache is invalidated when matching files change. |
| `outputs` | list | `[]` | Paths to archive as artifacts on success, restored on cache hit. |
| `allow_failure` | bool | `false` | If true, pipeline continues even if this step fails. |

### Matrix

```yaml
strategy:
  matrix:
    os: [alpine, ubuntu]
    version: ["18", "20"]
```

Each combination spawns a parallel job instance. Reference values with `${{ matrix.os }}`.

---

## Commands

### `zenith run`

Run the workflow in `.zenith.yml`.

```bash
zenith run                    # run the first/only job
zenith run --job test         # run a specific job
zenith run --no-cache         # bypass cache, re-run all steps
zenith run --remote myserver  # run on a registered remote (Phase 9)
```

### `zenith build`

Like `run`, but semantically for build jobs. Respects `outputs:` for artifact caching.

```bash
zenith build
zenith build --job compile --no-cache
```

### `zenith cache`

```bash
zenith cache list    # show all cache entries (hash, age, OS, arch, command)
zenith cache prune   # remove expired entries (older than TTL, default 7 days)
zenith cache clean   # remove all entries — next run rebuilds from scratch
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
zenith lab list                  # list active lab environments
zenith lab create [os]           # provision a new sandbox (default: alpine)
zenith lab shell [os]            # open interactive shell in the sandbox
zenith lab run <os> <command>    # run a single command in the sandbox
zenith lab push [os]             # copy project files into the sandbox workspace
zenith lab destroy <os>          # teardown and delete the sandbox
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
zenith plugin list               # list installed plugins
zenith plugin install ./my-dir   # install plugin from local directory
zenith plugin remove <name>      # uninstall a plugin
zenith plugin info <name>        # show plugin manifest details
```

---

## Caching

Zenith caches at the step level. A cache entry is keyed on a SHA-256 hash of:

- OS / architecture
- All environment variables for the step
- The `run` command text
- Contents of any files matched by `watch:` globs

### Cache hit behaviour

When a step's hash matches a prior run:
- The step is skipped entirely (`[CACHED]`)
- If the step had `outputs:`, those files are restored from the saved archive

### Forcing a rebuild

```bash
zenith run --no-cache     # skip all cache checks this run
```

Or at the step level:

```yaml
steps:
  - name: Always runs
    run: date
    cache: false
```

### Watching files

```yaml
steps:
  - name: Install dependencies
    run: npm ci
    watch:
      - package-lock.json    # re-run when lockfile changes
```

### Saving build artifacts

```yaml
steps:
  - name: Compile
    run: cargo build --release
    outputs:
      - target/release/my-binary    # archived on success, restored on cache hit
```

---

## Toolchains

Declare exact versions in `.zenith.yml` — Zenith downloads and caches them in `~/.zenith/toolchains/`.

```yaml
env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "1.78.0"
```

These binaries are prepended to `PATH` before every step, so `node`, `python3`, `go`, `cargo` all resolve to the exact versions declared — no system installs needed.

Per-job override:

```yaml
jobs:
  legacy:
    toolchain:
      node: "16.0.0"    # overrides top-level env: for this job only
```

Download all toolchains upfront (useful in CI prep):

```bash
zenith env init
```

---

## Sandbox Backends

### `container` (default)

Linux namespace isolation — no Docker, no root, no KVM required. Works on any Linux machine.

```yaml
backend: container
runs_on: alpine
```

### `firecracker`

Hardware-level MicroVM isolation using AWS Firecracker. Requires Linux with `/dev/kvm`.

```yaml
backend: firecracker
runs_on: alpine
```

Zenith downloads the Firecracker binary, a vmlinux kernel, and Alpine ext4 rootfs automatically.

### `wasm`

Run `.wasm` modules via wasmtime. Cross-platform, zero-OS overhead.

```yaml
backend: wasm
steps:
  - name: Run wasm binary
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

### Custom plugin

Any installed plugin can be used as a backend:

```yaml
backend: my-plugin-name
```

---

## Matrix Builds

Run the same job across multiple configurations in parallel:

```yaml
jobs:
  test:
    runs_on: local
    strategy:
      matrix:
        node_version: ["18", "20", "22"]
        os: [linux, windows]
    steps:
      - name: Test on ${{ matrix.os }} / Node ${{ matrix.node_version }}
        run: echo "Testing node=${{ matrix.node_version }} os=${{ matrix.os }}"
```

This produces 6 parallel instances. Each gets its own isolated workspace and cache entry.

List all defined matrix jobs:

```bash
zenith matrix list
```

---

## Lab Environments

Labs are named, persistent sandbox environments you can interact with manually.

```bash
# Create an alpine sandbox
zenith lab create alpine

# Push your project files into it
zenith lab push alpine

# Open an interactive shell
zenith lab shell alpine

# Run a one-off command
zenith lab run alpine "cat /etc/os-release"

# Clean up
zenith lab destroy alpine
```

Labs use OverlayFS on Linux so the base rootfs is never modified — each lab gets its own upper layer.

---

## Plugin System

Zenith's backend system is extensible. Plugins are external binaries that communicate over JSON-RPC on stdio — write them in any language.

Install a plugin:

```bash
zenith plugin install ./path/to/plugin-dir
```

The directory must contain:
- `plugin.toml` — manifest with name, version, type, entrypoint
- The entrypoint binary

Use the plugin as a backend:

```yaml
backend: my-plugin-name
```

See [docs/plugin_authoring.md](docs/plugin_authoring.md) for how to write a plugin, including the full protocol spec and a reference Rust implementation.

---

## Platform Notes

### Linux

Full feature support. Namespace isolation, OverlayFS, Firecracker, QEMU cross-arch, Wine, and all toolchain backends work natively.

### macOS

- Local execution and toolchain management work fully.
- Namespace isolation falls back to a restricted subprocess.
- Firecracker, QEMU cross-arch, and Wine are not available.
- WASM backend works fully.

### Windows

- Local execution and toolchain management work.
- Node.js and Go toolchains are downloaded as Windows binaries.
- Namespace isolation falls back to a restricted subprocess.
- Firecracker, QEMU, and Wine are not available.
- WASM backend works fully.

---

## License

Zenith is dual-licensed:

- **MIT License** — [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)
- **Apache License 2.0** — [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0)

You may use Zenith under either license at your option.

Any contribution submitted for inclusion in Zenith shall be dual-licensed as above, without additional terms or conditions.
