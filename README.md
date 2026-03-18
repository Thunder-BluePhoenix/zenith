# 🌌 Zenith

**The Ultimate Native Sandbox Workflow Engine**

Zenith is a high-performance, developer-first CI/CD runner designed to execute complex workflows in total isolation **without Docker**. It provides a native sandbox environment using Linux namespaces and cross-platform virtualization backends.

---

## 🚀 Key Features

- **📦 Docker-less Isolation**: Native sandbox runtime using Linux namespaces (PID, Mount, Network).
- **⚡ Parallel Matrix Execution**: High-concurrency job runner with `tokio`-powered orchestration.
- **🔌 Pluggable Backends**: Swap between `container` (native), `firecracker` (MicroVM), and `wasm` (WASI) isolation.
- **🌍 Cross-Architecture**: Transparently run ARM64 or RISC-V workloads on x86 hosts via QEMU integration.
- **🛡️ Secure-by-Default**: Automated rootfs provisioning, capability dropping, and environment clearing.
- **🔧 GitHub-Compatible YAML**: Familiar syntax for jobs, steps, matrix strategies, and environment management.

---

## 🛠️ Usage

### Define your workflow (`.zenith.yml`)

```yaml
jobs:
  build_cross_arch:
    strategy:
      matrix:
        os: ["alpine", "ubuntu"]
        arch: ["x86_64", "arm64"]
    runs-on: ${{ matrix.os }}
    arch: ${{ matrix.arch }}
    steps:
      - name: Build Component
        run: cargo build --release
```

### Run it locally

```bash
zenith run
```

---

## 🏗️ Architecture

Zenith is built in Rust for maximum safety and performance. Its modular architecture consists of:
- **Core Runner**: Orchestrates job expansion and parallel execution.
- **Lab Manager**: Handles ephemeral workspace provisioning and rootfs lifecycle.
- **Backend Abstractor**: Pluggable trait-based isolation (Namespaces, Firecracker, Wasmtime).

---

## 📂 Project Structure

- `src/runner.rs`: Parallel execution engine.
- `src/sandbox/`: Pluggable isolation backends.
- `src/config.rs`: YAML workflow schema.
- `docs/`: Multi-phase technical roadmaps.

---

## 📜 License

MIT License.
