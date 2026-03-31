# Use Cases

Real-world scenarios where Zenith replaces or improves existing workflows. Each case shows the problem, the Zenith solution, and a minimal `.zenith.yml` you can copy and run.

---

## 1. Local CI Without Docker

**Problem:** Running GitHub Actions locally requires Docker and `act` — slow, heavyweight, doesn't match production exactly. Teams skip local testing and discover failures only after pushing.

**Solution:** Zenith runs the same `.zenith.yml` locally and in CI without Docker, using Linux namespace isolation or Firecracker microVMs.

```yaml
version: "2"
jobs:
  test:
    runs-on: alpine
    steps:
      - name: Build
        run: make
        watch: [src/**/*.c, Makefile]
        outputs: [build/myapp]
      - name: Test
        run: make test
        depends_on: [Build]
```

```bash
zenith run   # identical to CI — before every push
```

---

## 2. Hermetic Builds

**Problem:** Build outputs differ between machines because developers have different `gcc`, `node`, or `python` versions installed.

**Solution:** Declare exact toolchain versions — Zenith downloads them and prepends them to `PATH` before every step.

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
      - name: Build frontend
        run: npm ci && npm run build
        watch: [frontend/src/**/*.ts, package-lock.json]
        outputs: [frontend/dist/]
      - name: Build backend
        run: go build -o bin/server ./cmd/server
        watch: [cmd/**/*.go, go.sum]
        outputs: [bin/server]
```

Every developer and every CI machine downloads the same binaries. Changing a version in `.zenith.yml` updates everyone on the next run.

---

## 3. Multi-Version Testing (Matrix)

**Problem:** A library needs to work on multiple runtimes. Testing each version manually is error-prone; configuring CI matrix jobs is tedious and doesn't run locally.

**Solution:** `strategy.matrix` expands one job into N parallel instances.

```yaml
version: "2"
jobs:
  test:
    runs-on: alpine
    strategy:
      matrix:
        python: ["3.10", "3.11", "3.12"]
        os:     [alpine, ubuntu]
    steps:
      - name: Setup
        run: pip install -e ".[test]"
        watch: [pyproject.toml]
      - name: Test
        run: pytest tests/ -v
        depends_on: [Setup]
```

```bash
zenith matrix run     # 6 parallel jobs (3 Python × 2 OS)
```

---

## 4. Cross-Platform Builds from One Machine

**Problem:** A team on Apple Silicon needs to ship Linux x86_64 and arm64 binaries. Setting up cross-compilation toolchains manually takes a day.

**Solution:** Set `arch:` — Zenith auto-downloads `qemu-user-static` and runs the build transparently.

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

---

## 5. Reproducible Release Artifacts

**Problem:** Release builds are hard to audit. Nobody knows exactly what inputs produced a given binary — a compliance requirement in finance, healthcare, and government.

**Solution:** Every step with `outputs:` is stored under a deterministic derivation hash. Same inputs → same hash → same outputs, on any machine.

```yaml
version: "2"
jobs:
  release:
    runs-on: alpine
    backend: firecracker
    steps:
      - name: Build
        run: cargo build --release --locked && strip target/release/myapp
        watch: [src/**/*.rs, Cargo.toml, Cargo.lock]
        outputs: [target/release/myapp]
      - name: Checksum
        run: sha256sum target/release/myapp > target/release/myapp.sha256
        outputs: [target/release/myapp.sha256]
        depends_on: [Build]
```

```bash
zenith build --derivation    # inspect exactly what inputs will be hashed
zenith store list            # verify a prior build is still cached identically
```

---

## 6. Remote Build Offload

**Problem:** A laptop has 8 cores. The team has a 64-core build server sitting idle.

**Solution:** Register the server as a remote. `zenith run --remote` uploads the project, streams logs back, and restores outputs locally.

```bash
zenith remote add buildbox deploy@192.168.1.50 --key ~/.ssh/id_rsa
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

---

## 7. Cloud CI Without Extra Config Files

**Problem:** GitHub Actions, GitLab CI, and CircleCI all require their own YAML syntax, cloud accounts, and secrets configuration. Smaller projects don't want the overhead.

**Solution:** Submit the same local `.zenith.yml` to the Zenith cloud service — no extra files, no new syntax.

```bash
zenith cloud login <api-key>
zenith cloud run --watch          # stream logs live
zenith cloud list                 # review past runs
```

---

## 8. Windows Tool Testing on Linux

**Problem:** A team maintains a Windows CLI (`.exe`) and wants to run its test suite in Linux CI without spinning up Windows VMs.

**Solution:** The `wine` backend runs `.exe` files in an isolated Wine prefix. Zenith downloads Wine automatically.

```yaml
version: "2"
jobs:
  test-windows:
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

---

## 9. WebAssembly Workloads

**Problem:** Testing Wasm output requires wasmtime or wasmer installed locally — yet another tool to manage.

**Solution:** The `wasm` backend runs `.wasm` files via auto-downloaded wasmtime with WASI filesystem and env passthrough.

```yaml
version: "2"
jobs:
  test-wasm:
    backend: wasm
    steps:
      - name: Compile to Wasm
        run: cargo build --release --target wasm32-wasi
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/wasm32-wasi/release/myplugin.wasm]
      - name: Test
        run: myplugin.wasm --test
        depends_on: [Compile to Wasm]
        env:
          PLUGIN_MODE: test
```

Same job runs on Linux, macOS, and Windows.

---

## 10. Parallel Dependency Pipelines

**Problem:** A large application has independent build stages (frontend, backend, docs) that are needlessly serialised. Total CI time is 3× longer than it should be.

**Solution:** Express the real dependency graph with `depends_on:`. Independent steps start immediately and run concurrently.

```yaml
version: "2"
jobs:
  full-build:
    runs-on: alpine
    steps:
      - name: Install Node deps
        run: npm ci
        outputs: [node_modules/]

      - name: Install Rust deps
        run: cargo fetch

      - name: Build frontend   # runs in parallel with Build backend and Build docs
        run: npm run build
        outputs: [frontend/dist/]
        depends_on: [Install Node deps]

      - name: Build backend    # runs in parallel with Build frontend and Build docs
        run: cargo build --release
        outputs: [target/release/server]
        depends_on: [Install Rust deps]

      - name: Build docs       # runs in parallel with Build frontend and Build backend
        run: npm run docs
        outputs: [docs/dist/]
        depends_on: [Install Node deps]

      - name: Integration tests
        run: ./scripts/integration-test.sh
        depends_on: [Build frontend, Build backend, Build docs]
```

Wall-clock time drops from the sum of all steps to the longest parallel chain.

---

## 11. Shared Remote Build Cache

**Problem:** A monorepo with 50 developers rebuilds `node_modules/` 50 times per day — all identical. Every CI machine does the same.

**Solution:** Point all machines at a shared remote binary cache. The first build pushes; every subsequent build on any machine gets an instant cache hit.

```yaml
version: "2"
cache:
  remote: "https://cache.mycompany.internal"
  push: true
  ttl_days: 30
```

```bash
zenith run
# [ci] [REMOTE HIT] Install deps  — restored in 200ms instead of 4m20s
# [ci] [REMOTE HIT] Build frontend — restored in 800ms instead of 4m20s
```

The derivation ID is the cache key — there are no stale hits because the key is a hash of all inputs.

---

## 12. Interactive Debugging Sandbox

**Problem:** A bug only reproduces inside a specific container environment. Developers need to poke around interactively without polluting the host machine.

**Solution:** Labs are persistent sandboxes with OverlayFS isolation — the base rootfs is never modified.

```bash
zenith lab create ubuntu
zenith lab push ubuntu           # copy project files in
zenith lab shell ubuntu          # interactive shell
zenith lab run ubuntu "python3 reproduce_bug.py"
zenith lab destroy ubuntu        # clean up
```

Run multiple labs simultaneously:

```bash
zenith lab create alpine
zenith lab create ubuntu
zenith lab list
```

---

## 13. Polyglot Monorepo

**Problem:** A monorepo contains a Rust backend, Python ML pipeline, Node.js frontend, and Go CLI. Maintaining separate Docker images for each is expensive.

**Solution:** One `.zenith.yml` with per-job toolchain overrides. Zenith manages every version independently.

```yaml
version: "2"
cache:
  ttl_days: 14

jobs:
  rust-backend:
    runs-on: alpine
    toolchain: { rust: stable }
    steps:
      - name: Build
        run: cargo build --release
        watch: [backend/src/**/*.rs]
        outputs: [backend/target/release/api-server]

  python-ml:
    runs-on: ubuntu
    toolchain: { python: "3.12.3" }
    steps:
      - name: Install
        run: pip install -r ml/requirements.txt
        watch: [ml/requirements.txt]
      - name: Test
        run: pytest ml/tests/
        depends_on: [Install]

  node-frontend:
    runs-on: alpine
    toolchain: { node: "20.11.0" }
    steps:
      - name: Install
        run: npm ci
        outputs: [frontend/node_modules/]
      - name: Build
        run: npm run build
        outputs: [frontend/dist/]
        depends_on: [Install]

  go-cli:
    runs-on: alpine
    toolchain: { go: "1.22.1" }
    steps:
      - name: Build
        run: go build -o bin/zcli ./cmd/zcli
        watch: [cli/**/*.go, go.sum]
        outputs: [bin/zcli]
```

---

## 14. Security Auditing in an Isolated VM

**Problem:** Running untrusted code (fuzz corpus, dependency audits, third-party packages) as part of the build pipeline risks the host machine.

**Solution:** `backend: firecracker` provides hardware-level VM isolation. The host filesystem is never accessible from inside the VM.

```yaml
version: "2"
jobs:
  audit:
    runs-on: alpine
    backend: firecracker
    steps:
      - name: Dependency audit
        run: cargo audit
        watch: [Cargo.lock]
      - name: Fuzz (60 second run)
        run: cargo fuzz run fuzz_target_1 -- -max_total_time=60
        allow_failure: true
        depends_on: [Dependency audit]
      - name: SBOM generation
        run: cyclonedx-bom > sbom.json
        outputs: [sbom.json]
        depends_on: [Dependency audit]
```

Each run gets a fresh copy-on-write rootfs snapshot — side effects from one run never persist to the next.

---

## 15. Plugin-Extended Pipelines

**Problem:** A team has a proprietary deployment tool that doesn't fit the standard shell-command model.

**Solution:** Write a plugin that implements the JSON-RPC Backend protocol over stdio. Install once; use as `backend:`.

```toml
# deploy-plugin/plugin.toml
[plugin]
name            = "k8s-deploy"
version         = "1.2.0"
type            = "backend"
entrypoint      = "k8s-deploy-bin"
requires_zenith = ">=0.1.0"
```

```bash
zenith plugin install ./deploy-plugin
# or search the registry:
zenith plugin search kubernetes
zenith plugin install terraform-runner
```

```yaml
version: "2"
jobs:
  deploy:
    backend: k8s-deploy
    steps:
      - name: Deploy to staging
        run: deploy --env staging --image myapp:latest
        env:
          KUBECONFIG: /secrets/kubeconfig
```

---

## 16. Near-Zero-Latency Dev Builds with the Daemon

**Problem:** Even fast VMs have a cold-boot cost. For iterative edit→build→test loops that happen dozens of times per hour, startup overhead accumulates.

**Solution:** `zenith daemon` pre-boots a pool of VMs and holds them as snapshots. Every `zenith run` restores a snapshot in under 1ms rather than cold-booting.

```bash
zenith daemon start --pool 4
zenith daemon status
# Pool: 4 warm VMs ready

zenith run     # < 1ms startup — snapshot restore, not cold boot
zenith run     # still < 1ms — pool replenished in background
zenith run     # still < 1ms

zenith daemon stop
```

| Mode | Startup | Total (cargo check) |
|---|---|---|
| Cold standalone | ~80ms | ~4.2s |
| Warm daemon | < 1ms | ~4.1s |
| Cached (store hit) | < 1ms | ~0.05s |

The daemon falls back gracefully to standalone mode when KVM is unavailable.

---

## Summary Table

| # | Use Case | Key Feature |
|---|---|---|
| 1 | Local CI without Docker | Namespace / Firecracker isolation |
| 2 | Hermetic builds | Auto toolchain download + PATH injection |
| 3 | Multi-version testing | `strategy.matrix` parallel expansion |
| 4 | Cross-platform builds | `arch:` + auto QEMU |
| 5 | Reproducible releases | Build store + derivation IDs |
| 6 | Remote build offload | `zenith remote` + `--remote` |
| 7 | Cloud CI | `zenith cloud run` |
| 8 | Windows tools on Linux | `backend: wine` |
| 9 | Wasm workloads | `backend: wasm` |
| 10 | Parallel pipelines | `depends_on:` dependency graph |
| 11 | Shared build cache | Remote binary cache + auto-push |
| 12 | Interactive debugging | `zenith lab create/shell/run` |
| 13 | Polyglot monorepo | Per-job `toolchain:` overrides |
| 14 | Security isolation | `backend: firecracker` |
| 15 | Custom backends | Plugin system |
| 16 | Zero-latency dev loop | `zenith daemon` warm VM pool |
