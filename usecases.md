# Zenith — Use Cases

A collection of real-world scenarios where Zenith replaces or improves on existing workflows. Each case shows the problem, how Zenith solves it, and a minimal `.zenith.yml` you can copy and run immediately.

---

## Table of Contents

1. [Local CI Without Docker](#1-local-ci-without-docker)
2. [Hermetic Builds — No More "Works on My Machine"](#2-hermetic-builds--no-more-works-on-my-machine)
3. [Multi-Version Testing (Matrix Builds)](#3-multi-version-testing-matrix-builds)
4. [Cross-Platform Builds from a Single Machine](#4-cross-platform-builds-from-a-single-machine)
5. [Reproducible Release Artifacts](#5-reproducible-release-artifacts)
6. [Remote Build Offload](#6-remote-build-offload)
7. [Cloud CI Without Configuration Files](#7-cloud-ci-without-configuration-files)
8. [Windows Tool Testing on Linux](#8-windows-tool-testing-on-linux)
9. [WebAssembly Workloads](#9-webassembly-workloads)
10. [Parallel Dependency Pipelines](#10-parallel-dependency-pipelines)
11. [Shared Remote Build Cache](#11-shared-remote-build-cache)
12. [Interactive Debugging Sandbox](#12-interactive-debugging-sandbox)
13. [Polyglot Monorepo](#13-polyglot-monorepo)
14. [Security Auditing in an Isolated VM](#14-security-auditing-in-an-isolated-vm)
15. [Plugin-Extended Pipelines](#15-plugin-extended-pipelines)
16. [Near-Zero-Latency Local Builds with the Daemon](#16-near-zero-latency-local-builds-with-the-daemon)

---

## 1. Local CI Without Docker

**Problem:** Running GitHub Actions locally requires Docker and `act`, which is slow, heavyweight, and doesn't match production behaviour exactly. Teams skip local CI testing because it's too painful, so they discover failures only after pushing.

**Zenith solution:** Zenith runs the same `.zenith.yml` locally and in CI without Docker. It uses Linux namespace isolation or Firecracker microVMs — no daemon, no image pulls, no root access required.

```yaml
version: "2"

jobs:
  test:
    runs-on: alpine
    steps:
      - name: Install deps
        run: apk add --no-cache make gcc musl-dev
        watch: []

      - name: Build
        run: make
        watch: [src/**/*.c, Makefile]
        outputs: [build/myapp]
        depends_on: [Install deps]

      - name: Test
        run: make test
        depends_on: [Build]
```

```bash
# Developer runs this before pushing — identical to CI
zenith run
```

**Result:** No Docker, no VM setup, no root needed. The same workflow file runs on the developer's laptop and in CI, closing the "it passed locally" gap.

---

## 2. Hermetic Builds — No More "Works on My Machine"

**Problem:** Build outputs differ between machines because developers have different versions of `gcc`, `node`, or `python` installed at the system level. Debugging environment differences wastes hours.

**Zenith solution:** Declare exact toolchain versions in `.zenith.yml`. Zenith downloads them into `~/.zenith/toolchains/` and prepends them to `PATH` before every step — no system installs, no version managers.

```yaml
version: "2"

env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.1"

jobs:
  build:
    runs-on: alpine
    steps:
      - name: Verify versions
        run: |
          node --version
          python3 --version
          go version

      - name: Build frontend
        run: npm ci && npm run build
        watch: [frontend/src/**/*.ts, package-lock.json]
        outputs: [frontend/dist/]

      - name: Build backend
        run: go build -o bin/server ./cmd/server
        watch: [cmd/**/*.go, internal/**/*.go, go.sum]
        outputs: [bin/server]
```

Every developer and every CI machine downloads the exact same binaries. If you change `node: "20.11.0"` to `"22.0.0"`, every machine picks it up on the next run.

---

## 3. Multi-Version Testing (Matrix Builds)

**Problem:** A library needs to work on multiple language runtimes. Testing each version manually is error-prone; CI matrix jobs are slow to configure and don't run locally.

**Zenith solution:** The `strategy.matrix` block expands one job into N parallel instances, each with its own isolated workspace and cache entry.

```yaml
version: "2"

jobs:
  test-matrix:
    runs-on: alpine
    strategy:
      matrix:
        python: ["3.10", "3.11", "3.12"]
        os:     [alpine, ubuntu]
    env:
      PYTHON_VERSION: ${{ matrix.python }}
    steps:
      - name: Setup
        run: pip install -e ".[test]"
        watch: [pyproject.toml, requirements*.txt]

      - name: Test
        run: pytest tests/ -v
        depends_on: [Setup]
        allow_failure: false
```

```bash
zenith matrix run     # 6 parallel jobs (3 Python × 2 OS)
zenith matrix list    # preview all combinations before running
```

Cache entries are keyed per-combination, so a re-run skips combinations whose inputs haven't changed.

---

## 4. Cross-Platform Builds from a Single Machine

**Problem:** A team on Apple Silicon (aarch64) needs to ship Linux x86_64 and Linux arm64 binaries. Setting up cross-compilation toolchains manually takes a day; maintaining them takes weeks.

**Zenith solution:** Set `arch:` in the job config. Zenith auto-downloads `qemu-user-static` for the target architecture, registers `binfmt_misc`, and runs the build inside the right environment transparently.

```yaml
version: "2"

jobs:
  build-x86:
    runs-on: alpine
    arch: x86_64
    steps:
      - name: Build
        run: cargo build --release --target x86_64-unknown-linux-musl
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/x86_64-unknown-linux-musl/release/myapp]

  build-arm64:
    runs-on: alpine
    arch: aarch64
    steps:
      - name: Build
        run: cargo build --release --target aarch64-unknown-linux-musl
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/aarch64-unknown-linux-musl/release/myapp]
```

```bash
zenith run --job build-x86
zenith run --job build-arm64
```

No cross-compilation toolchain setup. No Docker buildx. The same commands work on x86_64 and Apple Silicon hosts.

---

## 5. Reproducible Release Artifacts

**Problem:** Release builds are hard to audit because nobody knows exactly what inputs produced a given binary. Reproducibility is a compliance requirement in some industries (finance, healthcare, government).

**Zenith solution:** Every step with `outputs:` is stored in the content-addressable build store (`~/.zenith/store/`). The store key is a deterministic derivation hash of all inputs — command, environment, OS, arch, and watched file contents. The same inputs always produce the same hash and the same cached output.

```yaml
version: "2"

jobs:
  release:
    runs-on: alpine
    backend: firecracker   # hardware isolation for maximum reproducibility
    steps:
      - name: Build release binary
        run: |
          cargo build --release --locked
          strip target/release/myapp
        watch:
          - src/**/*.rs
          - Cargo.toml
          - Cargo.lock
        outputs:
          - target/release/myapp

      - name: Checksum
        run: sha256sum target/release/myapp > target/release/myapp.sha256
        outputs: [target/release/myapp.sha256]
        depends_on: [Build release binary]
```

```bash
# See exactly what inputs produced this build
zenith build --derivation

# Verify a prior build is still cached with the same hash
zenith store list
zenith store info <drv-id>
```

Two developers building the same commit on different machines get identical derivation IDs. If the IDs match, the outputs are identical.

---

## 6. Remote Build Offload

**Problem:** A developer laptop has 8 cores and limited RAM. Compilation jobs take 10+ minutes locally. The team has a powerful 64-core build server sitting idle most of the time.

**Zenith solution:** Register the build server as a remote and run jobs on it with one flag. Zenith uploads the project, runs the workflow, streams logs back in real time, and cleans up.

```bash
# One-time setup
zenith remote add buildbox deploy@192.168.1.50 --key ~/.ssh/id_rsa
zenith remote status buildbox
# buildbox: reachable — arch=x86_64, zenith-agent installed

# Every subsequent build
zenith run --remote buildbox --job compile
```

```yaml
version: "2"

jobs:
  compile:
    runs-on: alpine
    steps:
      - name: Build
        run: cargo build --release -j64
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/release/myapp]
```

The developer sees logs live. The binary lands in their local workspace via the output restore path. The build server's cache is populated and shared across the team.

---

## 7. Cloud CI Without Configuration Files

**Problem:** Setting up GitHub Actions, GitLab CI, or CircleCI requires learning a new YAML DSL, dealing with secrets management, and waiting for cloud runners. Smaller projects don't want the overhead.

**Zenith solution:** Submit the same local `.zenith.yml` to the Zenith cloud service with a single command.

```bash
# Authenticate once
zenith cloud login <api-key>

# Submit and watch live
zenith cloud run --watch

# Or check status async
zenith cloud run
zenith cloud status <run-id>
zenith cloud logs <run-id>
```

No separate CI configuration files. No new YAML syntax to learn. The same file that runs locally runs in the cloud. The cloud cache is shared with your local store via the remote binary cache.

---

## 8. Windows Tool Testing on Linux

**Problem:** A team maintains a Windows CLI tool (`.exe`) and wants to run its test suite in CI on Linux runners without spinning up Windows VMs.

**Zenith solution:** The `wine` backend runs `.exe` files inside an isolated Wine prefix. Each job gets its own `WINEPREFIX` so Wine state never leaks between runs.

```yaml
version: "2"

jobs:
  test-windows-tool:
    runs-on: alpine
    backend: wine
    steps:
      - name: Build Windows binary
        run: cargo build --release --target x86_64-pc-windows-gnu
        watch: [src/**/*.rs]
        outputs: [target/x86_64-pc-windows-gnu/release/mytool.exe]

      - name: Run tests via Wine
        run: mytool.exe --run-tests
        depends_on: [Build Windows binary]
```

Zenith downloads Wine automatically into `~/.zenith/wine/`. No `apt install wine`. The test suite runs on Linux with the Windows binary, catching Windows-specific regressions before anyone touches a Windows machine.

---

## 9. WebAssembly Workloads

**Problem:** A team is building a plugin system where plugins are compiled to WebAssembly for sandboxed execution. Testing the Wasm output requires wasmtime or wasmer installed locally.

**Zenith solution:** The `wasm` backend runs `.wasm` files via Zenith's auto-downloaded wasmtime with WASI filesystem and env passthrough.

```yaml
version: "2"

jobs:
  test-wasm:
    runs-on: local
    backend: wasm
    steps:
      - name: Compile to Wasm
        run: cargo build --release --target wasm32-wasi
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/wasm32-wasi/release/myplugin.wasm]

      - name: Test Wasm binary
        run: myplugin.wasm --test
        depends_on: [Compile to Wasm]
        env:
          PLUGIN_MODE: test
```

Zenith downloads `wasmtime` automatically. The same job runs on Linux, macOS, and Windows — the only platform-native step is the compile (which uses the host toolchain or a cross-compiler).

---

## 10. Parallel Dependency Pipelines

**Problem:** A large application has independent build stages (frontend, backend, documentation) that are unnecessarily serialized. The total CI time is 3× longer than it needs to be.

**Zenith solution:** Use `depends_on:` to express the actual dependency graph. Steps with satisfied dependencies start immediately and run in parallel.

```yaml
version: "2"

env:
  node: "20.11.0"
  rust: stable

jobs:
  full-build:
    runs-on: alpine
    steps:
      - name: Install Node deps
        run: npm ci
        watch: [package-lock.json]
        outputs: [node_modules/]

      - name: Install Rust deps
        run: cargo fetch
        watch: [Cargo.lock]

      # These three run in parallel once their dependencies are met:
      - name: Build frontend
        run: npm run build
        watch: [frontend/src/**]
        outputs: [frontend/dist/]
        depends_on: [Install Node deps]

      - name: Build backend
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/release/server]
        depends_on: [Install Rust deps]

      - name: Build docs
        run: npm run docs
        watch: [docs/**/*.md]
        outputs: [docs/dist/]
        depends_on: [Install Node deps]

      # Only starts when all three above are done:
      - name: Integration tests
        run: ./scripts/integration-test.sh
        depends_on: [Build frontend, Build backend, Build docs]

      - name: Package release
        run: ./scripts/package.sh
        outputs: [dist/release.tar.gz]
        depends_on: [Integration tests]
```

`Build frontend`, `Build backend`, and `Build docs` run simultaneously. `Integration tests` waits for all three. Wall-clock time drops from the sum of all steps to the longest parallel chain.

---

## 11. Shared Remote Build Cache

**Problem:** Every developer and every CI machine rebuilds the same artifacts from scratch. A monorepo with 50 developers rebuilds `node_modules/` 50 times per day — all identical.

**Zenith solution:** Point all machines at a shared remote binary cache. When any machine builds a derivation for the first time, it pushes the result. Every subsequent build on any machine gets an instant cache hit.

```yaml
version: "2"

cache:
  remote: "https://cache.mycompany.internal"
  push: true       # CI pushes; developers pull
  ttl_days: 30
```

```bash
# CI machine (pushes):
# cache.push = true in .zenith.yml — automatic after each build

# Developer laptop (pulls):
zenith run
# [ci] [REMOTE HIT] Install deps — restored from cache.mycompany.internal
# [ci] [REMOTE HIT] Build frontend — restored in 800ms instead of 4m20s
```

The derivation ID is the cache key. Because it's a hash of all inputs, there are no stale cache hits — a key either matches exactly or it doesn't.

---

## 12. Interactive Debugging Sandbox

**Problem:** A bug only reproduces inside a specific container environment. Developers need to poke around interactively but don't want to pollute their local machine or the project's Docker setup.

**Zenith solution:** Labs are persistent, isolated sandboxes. Push your project into one and get an interactive shell. The base rootfs is never modified — each lab has its own OverlayFS upper layer.

```bash
# Spin up an Ubuntu sandbox
zenith lab create ubuntu

# Copy your project into it
zenith lab push ubuntu

# Open an interactive shell — poke around freely
zenith lab shell ubuntu

# Or run a one-off reproduction command
zenith lab run ubuntu "python3 reproduce_bug.py"

# Tear down cleanly when done
zenith lab destroy ubuntu
```

You can create multiple labs with different OS images simultaneously:

```bash
zenith lab create alpine
zenith lab create ubuntu
zenith lab list
# alpine   running   /home/user/.zenith/labs/alpine/
# ubuntu   running   /home/user/.zenith/labs/ubuntu/
```

---

## 13. Polyglot Monorepo

**Problem:** A monorepo contains a Rust backend, Python ML pipeline, Node.js frontend, and Go CLI tool. Each component has different build tools, versions, and CI requirements. Maintaining separate Docker images for each is expensive.

**Zenith solution:** Define one job per component in a single `.zenith.yml` with per-job toolchain overrides. Zenith downloads and manages every toolchain version independently.

```yaml
version: "2"

cache:
  ttl_days: 14

jobs:
  rust-backend:
    runs-on: alpine
    toolchain:
      rust: stable
    steps:
      - name: Build
        run: cargo build --release
        watch: [backend/src/**/*.rs, backend/Cargo.toml]
        outputs: [backend/target/release/api-server]

  python-ml:
    runs-on: ubuntu
    toolchain:
      python: "3.12.3"
    steps:
      - name: Install
        run: pip install -r ml/requirements.txt
        watch: [ml/requirements.txt]
      - name: Test
        run: pytest ml/tests/
        depends_on: [Install]

  node-frontend:
    runs-on: alpine
    toolchain:
      node: "20.11.0"
    steps:
      - name: Install
        run: npm ci
        watch: [frontend/package-lock.json]
        outputs: [frontend/node_modules/]
      - name: Build
        run: npm run build
        watch: [frontend/src/**]
        outputs: [frontend/dist/]
        depends_on: [Install]

  go-cli:
    runs-on: alpine
    toolchain:
      go: "1.22.1"
    steps:
      - name: Build
        run: go build -o bin/zcli ./cmd/zcli
        watch: [cli/**/*.go, go.sum]
        outputs: [bin/zcli]
```

```bash
zenith run --job rust-backend
zenith run --job python-ml
zenith run --job node-frontend
zenith run --job go-cli
```

Each job downloads its own toolchain independently. Changing the Rust version has zero effect on the Python job's cache.

---

## 14. Security Auditing in an Isolated VM

**Problem:** A security team needs to run untrusted code (third-party packages, fuzz corpus, dependency audits) as part of the build pipeline without risking the host machine.

**Zenith solution:** Use `backend: firecracker` for hardware-level VM isolation. The workload runs inside a KVM microVM with its own kernel, memory space, and network namespace. The host filesystem is never accessible.

```yaml
version: "2"

jobs:
  audit:
    runs-on: alpine
    backend: firecracker   # hardware VM isolation
    steps:
      - name: Dependency audit
        run: cargo audit
        watch: [Cargo.lock]

      - name: Fuzz (short run)
        run: cargo fuzz run fuzz_target_1 -- -max_total_time=60
        allow_failure: true   # fuzzing may find a crash — log it, don't block
        depends_on: [Dependency audit]

      - name: SBOM generation
        run: cyclonedx-bom > sbom.json
        outputs: [sbom.json]
        depends_on: [Dependency audit]
```

Each run gets a fresh copy-on-write rootfs snapshot. Even if the untrusted code escapes the process sandbox, it's still inside the Firecracker VM boundary.

---

## 15. Plugin-Extended Pipelines

**Problem:** A team has a proprietary deployment tool that doesn't fit the standard shell-command model. They want it to act as a first-class Zenith backend without modifying Zenith itself.

**Zenith solution:** Write a plugin binary that implements the JSON-RPC `Backend` protocol over stdio. Install it once; use it as a `backend:` value like any built-in.

**Plugin manifest (`deploy-plugin/plugin.toml`):**

```toml
[plugin]
name            = "k8s-deploy"
version         = "1.2.0"
type            = "backend"
entrypoint      = "k8s-deploy-bin"
description     = "Kubernetes deployment backend for Zenith"
requires_zenith = ">=0.1.0"
```

```bash
zenith plugin install ./deploy-plugin
```

```yaml
version: "2"

jobs:
  deploy:
    backend: k8s-deploy     # custom plugin backend
    steps:
      - name: Deploy to staging
        run: deploy --env staging --image myapp:${{ matrix.version }}
        env:
          KUBECONFIG: /secrets/kubeconfig
```

Search for community plugins:

```bash
zenith plugin search kubernetes
zenith plugin search terraform
zenith plugin install terraform-runner
```

---

## 16. Near-Zero-Latency Local Builds with the Daemon

**Problem:** Even fast VMs have a cold-boot cost. For iterative development (edit → build → test loops that happen dozens of times per hour), even 100ms of startup overhead adds up.

**Zenith solution:** Start `zenith daemon` once at the beginning of your work session. It pre-boots a pool of VMs and holds them in memory as snapshots. Every `zenith run` after that restores a snapshot in under 1ms rather than cold-booting.

```bash
# Start the daemon at the beginning of your session
zenith daemon start --pool 4
zenith daemon status
# Pool: 4 warm VMs ready
# Daemon uptime: 00:02:11

# Every build now connects to the daemon automatically
zenith run          # < 1ms startup — no cold boot
zenith run          # still < 1ms — pool replenished in background
zenith run          # still < 1ms

# Check KVM availability
zenith daemon hypervisor-check
# KVM hypervisor: AVAILABLE
# The Zenith custom VMM is supported on this machine.

# Shut down at end of day
zenith daemon stop
```

The daemon falls back gracefully to standalone mode if KVM is unavailable (macOS, Windows, VMs without nested virtualization) — the `zenith run` command is identical in either case.

**Measured impact for a typical Rust project (edit → `cargo check` loop):**

| Mode | Startup | Total time |
|---|---|---|
| Cold standalone | ~80ms | ~4.2s |
| Warm daemon | < 1ms | ~4.1s |
| Cached hit | < 1ms | ~0.05s |

The daemon is most impactful when combined with a remote binary cache — startup is near-zero and cache hits are near-instant.

---

## Summary

| Use Case | Key Feature |
|---|---|
| Local CI parity | Namespace / Firecracker isolation |
| Hermetic builds | Automatic toolchain download + PATH injection |
| Multi-version testing | `strategy.matrix` parallel expansion |
| Cross-platform builds | `arch:` + auto QEMU |
| Reproducible releases | Content-addressable build store + derivations |
| Remote build offload | `zenith remote add` + `--remote` flag |
| Cloud CI | `zenith cloud run` |
| Windows tools on Linux | `backend: wine` |
| Wasm workloads | `backend: wasm` |
| Parallel pipelines | `depends_on:` dependency graph |
| Shared build cache | Remote binary cache + auto-push |
| Interactive debugging | `zenith lab create/shell/run` |
| Polyglot monorepo | Per-job `toolchain:` overrides |
| Security isolation | `backend: firecracker` hardware VM |
| Custom backends | Plugin system JSON-RPC protocol |
| Zero-latency dev loop | `zenith daemon` warm VM pool |
