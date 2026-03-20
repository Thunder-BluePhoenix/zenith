# Zenith

**Local Multi-OS Workflow Runtime — You install Zenith. Zenith installs everything else.**

Zenith is a developer-first CI/CD runner that executes workflows locally in total isolation — no Docker, no cloud, no system-level setup. It downloads every tool it needs automatically.

---

## Why Zenith?

Most CI runners require you to pre-install runtimes, configure containers, or push to a cloud service to test your pipeline. Zenith flips this:

- You write `.zenith.yml`. You run `zenith run`. Everything else is handled.
- Language toolchains (Node, Python, Go, Rust) are downloaded and version-pinned automatically.
- Sandboxes (Alpine rootfs, Firecracker MicroVMs, WASM) are provisioned on demand.
- Results are cached by SHA-256 content hash — unchanged steps are never re-run.

---

## Features

| Feature | Status |
|---|---|
| Local workflow runner (jobs, steps, env, working dir) | Complete |
| Parallel matrix execution (`strategy.matrix`) | Complete |
| Sandbox isolation — Linux namespaces, no Docker | Complete |
| OverlayFS copy-on-write workspaces | Complete |
| Firecracker MicroVM backend (requires Linux KVM) | Complete |
| WebAssembly / WASI backend via wasmtime | Complete |
| Cross-arch execution (ARM64 on x86) via QEMU | Complete |
| Wine backend (Windows .exe on Linux) | Complete |
| Build cache — SHA-256, TTL, artifact save/restore | Complete |
| `zenith build --no-cache` force rebuild | Complete |
| Language toolchains — Node.js, Python, Go, Rust | Complete |
| `zenith env init/shell/list/clean` | Complete |
| Plugin system — external process + JSON-RPC | Complete |
| `zenith plugin install/list/remove/info` | Complete |
| Remote runner (SSH transport) | Phase 9 — planned |
| Cloud runtime | Phase 10 — planned |

---

## Quick Start

### 1. Install

```bash
cargo install --path .
```

### 2. Create `.zenith.yml`

```yaml
version: "1"

env:
  node: "20.11.0"       # Zenith downloads Node automatically

jobs:
  test:
    runs_on: local
    steps:
      - name: Install deps
        run: npm ci
        watch:
          - package-lock.json   # cache invalidated when lockfile changes

      - name: Run tests
        run: npm test
```

### 3. Run

```bash
zenith run
```

---

## CLI Reference

```
zenith run [--job <name>] [--no-cache] [--remote <name>]
zenith build [--job <name>] [--no-cache]
zenith cache list | clean | prune
zenith lab create | shell | run | push | destroy | list
zenith env init | shell | list | clean
zenith matrix [run | list] [--no-cache]
zenith shell [--lab <os>]
zenith plugin list | install <path> | remove <name> | info <name>
```

---

## `.zenith.yml` Reference

```yaml
version: "1"

# Declare language toolchain versions — Zenith downloads them automatically
env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.0"
  rust:   "1.78.0"

jobs:
  my-job:
    runs_on: local              # "local" or a sandbox OS like "alpine"
    backend: container          # container | firecracker | wasm | wine | <plugin>
    arch: x86_64                # x86_64 | aarch64 (auto-downloads QEMU if needed)

    # Per-job toolchain override (takes precedence over top-level env:)
    toolchain:
      node: "18.0.0"

    env:
      CI: "true"

    working_directory: ./app

    # Matrix: expands into N parallel job instances
    strategy:
      matrix:
        os: [alpine, ubuntu]
        version: ["1.0", "2.0"]

    steps:
      - name: My step
        run: echo "os=${{ matrix.os }} version=${{ matrix.version }}"
        env:
          STEP_VAR: value
        working_directory: ./subdir

        # Cache control
        cache: true                     # default true; set false to always re-run
        cache_key: my-custom-key        # override auto-computed hash
        watch:                          # re-run if these files change
          - src/**/*.rs
          - Cargo.lock
        outputs:                        # archive these paths on success
          - target/release/my-binary

        allow_failure: false            # if true, pipeline continues on error
```

---

## Sandbox Backends

| Backend | Isolation | Platform | Use case |
|---|---|---|---|
| `container` | Linux namespaces | Linux (no KVM) | Default — fast, lightweight |
| `firecracker` | MicroVM (hardware) | Linux + KVM | Untrusted code, full OS isolation |
| `wasm` | WASI sandbox | All platforms | WebAssembly modules |
| `wine` | Wine prefix | Linux | Run Windows `.exe` binaries |
| `<plugin-name>` | Custom | Any | Your own backend via JSON-RPC |

---

## Toolchain Auto-Download

Zenith downloads exact versioned toolchains into `~/.zenith/toolchains/` and prepends them to `PATH` before every step. No `nvm`, `pyenv`, or `rustup` needed.

```
~/.zenith/
├── bin/              # qemu, wasmtime, rustup-init, ...
├── toolchains/
│   ├── node/20.11.0/
│   ├── python/3.12.3/
│   ├── go/1.22.0/
│   └── rust/1.78.0/
├── rootfs/           # Alpine, Ubuntu, ... (for sandbox labs)
├── rootfs-fc/        # ext4 images for Firecracker
├── kernel/           # vmlinux for Firecracker
├── cache/            # step cache entries + artifact archives
└── plugins/          # installed plugins
```

---

## Plugin System

Write a custom backend in any language and install it as a plugin.

```bash
# Install a plugin from a local directory
zenith plugin install ./my-plugin

# Use it in .zenith.yml
# backend: my-plugin-name
```

See [docs/plugin_authoring.md](docs/plugin_authoring.md) for the protocol spec and reference implementation.

---

## Platform Support

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Local workflow execution | Full | Full | Full |
| Toolchain auto-download | Full | Full | Full (Node, Go) |
| Container (namespace) isolation | Full | Fallback | Fallback |
| Firecracker MicroVM | Full (KVM required) | No | No |
| QEMU cross-arch | Full | No | No |
| Wine backend | Full | No | No |
| WASM/wasmtime | Full | Full | Full |
| Plugin system | Full | Full | Full |

---

## License

Licensed under either of:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Zenith shall be dual-licensed as above, without any additional terms or conditions.
