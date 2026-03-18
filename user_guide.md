# 📖 Zenith User Guide

Welcome to **Zenith**, the ultimate native-isolation workflow engine. Zenith allows you to execute complex builds, tests, and deployments in isolated sandboxes with no external dependencies like Docker.

---

## 🚀 Quick Start

1. **Initialize Your Project**: Create a `.zenith.yml` file in your root directory.
2. **Run Workflows**: Use the command `zenith run` to start your jobs in parallel.
3. **Manage Labs**: Use `zenith lab list` to see your active sandbox environments.

---

## 🛠️ Configuration (`.zenith.yml`)

Zenith uses a YAML-based configuration similar to GitHub Actions but optimized for local, high-performance execution.

### Basic Structure

```yaml
jobs:
  test:
    runs-on: alpine
    steps:
      - name: Build
        run: make build
```

### Full Specification

| Field | Description | Example |
| :--- | :--- | :--- |
| `jobs` | A map of job identifiers to job objects. | `jobs: { build: ... }` |
| `runs-on` | The base OS image to use (e.g., `alpine`, `ubuntu`). | `runs-on: alpine` |
| `arch` | Target CPU architecture (e.g., `x86_64`, `arm64`). | `arch: arm64` |
| `backend` | Isolation engine: `container`, `firecracker`, `wasm`. | `backend: container` |
| `strategy` | Matrix execution strategy (parallel runs). | `matrix: { os: [alpine, debian] }` |
| `env` | Key-value pairs of environment variables. | `env: { DEBUG: "true" }` |
| `cache` | Enable/disable SHA-256 step-level caching. | `cache: true` |

---

## 📦 Isolation Backends

Zenith supports multiple isolation layers depending on your security and performance needs.

- **Container (Default)**: Uses Linux namespaces (PID, Mount, Network) for lightweight, Docker-less isolation. Fast and efficient.
- **Firecracker**: Spawns an AWS Firecracker MicroVM for hardware-level isolation. Best for untrusted code (requires Linux KVM).
- **WebAssembly**: Executes `.wasm` modules natively via Wasmtime. Zero-OS overhead for lightweight server-side apps.

---

## 🏎️ Performance Features

### ⚡ Nix-style Caching
Zenith automatically hashes your step inputs (commands, environment, matrix). If the hash matches a previous run, Zenith skips the execution entirely.
- **Cache Hit**: `[CACHED] Step: Build (Skipping execution)`
- **Cache Invalidation**: Changing even a single character in your `run` command will trigger a fresh execution.

### 🧬 Parallel Matrix
Run dozens of job instances in parallel. Zenith orchestrates the concurrency automatically:
```yaml
strategy:
  matrix:
    os: [alpine, ubuntu]
    node_version: [18, 20]
```

---

## ⌨️ CLI Command Reference

### `zenith run`
Executes the workflow defined in `.zenith.yml`.
- `-f <file>`: Specify a different config file.
- `--jobs <n>`: Limit parallel concurrency.

### `zenith lab list`
Lists all provisioned sandbox environments and their status.

### `zenith lab clean`
Removes all ephemeral lab environments and clears the local cache.

---

## 🌍 Cross-Architecture Support

Zenith enables you to test ARM64 or RISC-V binaries on your x86 machine. Simply set `arch: arm64` in your job. 
> [!NOTE]
> On Linux, this uses `qemu-user-static`. On Windows/macOS, it provides intelligent guidance on using virtualization backends.

---

## 🛡️ Security

Zenith sandboxes are designed with **Total Isolation** in mind:
1. **Clean Environment**: All inherited $PATH and system variables are stripped.
2. **Restricted Filesystem**: Processes only see their provisioned rootfs and workspace.
3. **No Root**: Processes are restricted to non-privileged users inside the sandbox.
