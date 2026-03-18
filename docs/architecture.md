# Zenith Architecture

Zenith operates as a stratified runtime, designed to seamlessly translate declarative workflow files into low-level sandboxed execution. This architecture eliminates the heavy requirements of traditional VMs while providing far more isolation and compatibility than standard containers.

## System Topology & Layers

The core of Zenith relies on a modular, interface-driven approach allowing multiple "backends" to serve the same workflow pipeline.

### 1. CLI Layer & UX
*   **Responsibility:** Command parsing, user interaction, logging outputs, and formatting.
*   **Primary CLI commands:** `run`, `lab`, `matrix`, `shell`.
*   **Config Parser:** Loads `.zenith.yml` and resolves variable interpolation, matrices, and environmental overrides.

### 2. Workflow Engine
*   **Responsibility:** Interpreting the parsed configuration and orchestrating execution.
*   **Job Scheduler:** Organizes jobs sequentially or in parallel (for matrix runs).
*   **State Management:** Passes artifacts, exit codes, and environment variables between sequential steps.
*   **I/O Multiplexer:** Streams stdout/stderr from multiple concurrent matrix VMs back to a unified TTY interface.

### 3. Sandbox / Lab Manager
*   **Responsibility:** Defining the boundaries and filesystem of the ephemeral environment prior to boot.
*   **Rootfs Controller:** Pulls and caches minimal OS images (Alpine, Ubuntu, Custom).
*   **Mount Manager:** Binds the developer's working directory into the ephemeral environment using `overlayfs`, `FUSE`, or `9p` file sharing protocols.

### 4. VM Engine / Backend Abstractor
*   **Responsibility:** Executing the lab. The backend is interchangeable based on the target OS/Arch and user preference.
*   **Supported Backends:**
    *   **MicroVM (Firecracker):** Provides a real Linux kernel with hypervisor-level isolation but boots in < 200 milliseconds.
    *   **Container/Chroot:** Uses Linux namespaces/cgroups for process-level isolation (fastest, but shares host kernel).
    *   **Emulator (QEMU):** Translates instructions for cross-architecture builds (e.g., executing an aarch64 binary on x86_64).
    *   **Compatibility Wrappers:** Automates `Wine` or `Darling` prefixes for testing Windows/macOS binaries on Linux.
    *   **Wasmtime:** Executes WebAssembly targets within a lightweight sandbox.

## Core Tech Stack Choice

To achieve sub-second execution speeds and portability, the typical tech stack would be:
*   **Language:** Rust or Go (Compiles to a single binary with no dependencies).
*   **Virtualization:** The KVM API integrated with the AWS Firecracker VM monitor.
*   **FS Operations:** `overlayfs` for filesystem layering, avoiding large disk usage.

## Logical Application Flow

1. Developer runs `zenith run`.
2. **Parser** validates `.zenith.yml` and generates the execution graph.
3. If matrix is defined, the **Workflow Engine** spans concurrent threads.
4. For a specific job (e.g., `ubuntu-latest`), the **Lab Manager** mounts an `ubuntu` rootfs + an `overlay` containing the host project folder.
5. The **VM Engine** spins up a Firecracker instances pointing to the overlay.
6. The test script executes natively.
7. Logs stream back via `tty` allocation to the host shell.
8. VM is forcefully destroyed (takes ms) ensuring ephemeral immutability.

## Directory Structure (Host Machine)

On the developer's local machine, Zenith stores state securely and minimally:

```text
~/.zenith/
  ├── config/        # Global settings, auth, backend preferences
  ├── cache/         # Cached rootfs images, WASM binaries, downloaded toolchains
  ├── labs/          # Ephemeral instance data, overlayfs layers
  └── logs/          # Retained logs from past workflow runs
```
