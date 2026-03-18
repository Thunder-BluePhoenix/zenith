# Zenith Roadmap

Zenith is a Local Multi-OS Workflow Runtime aiming to be a universal developer runtime. It combines the speed of containers, the reproducibility of Nix, the multi-OS capability of VMs, and the workflow automation of GitHub Actions into a single cohesive CLI tool.

This roadmap maps out the project's evolution across logical phases, starting from a basic functional CLI to a complete OS-level developer runtime.

## Phase 0: CLI Core & Foundation
*   **Goal:** Create a robust, basic CLI tool capable of parsing configurations and running simple commands.
*   **Commands:** `zenith run`, `zenith lab`, `zenith matrix`, `zenith shell`
*   **Core Components:** CLI parser, config loader (YAML), simple command runner (exec), basic logging module.
*   **Tech Stack:** Rust or Go.

## Phase 1: Lab Environments (Sandbox Mode)
*   **Goal:** Provide lightweight, isolated sandbox environments locally.
*   **Commands:**
    *   `zenith lab create ubuntu`
    *   `zenith lab shell ubuntu`
    *   `zenith lab run ubuntu make test`
    *   `zenith lab destroy ubuntu`
*   **Implementation:** Using `chroot`, rudimentary containers (runc), or `overlayfs`.
*   **Features:** Automated `rootfs` fetching, project directory mounting into the sandbox.

## Phase 2: Workflow Engine (Local CI Mode)
*   **Goal:** Implement a GitHub Actions style declarative local workflow runner.
*   **Configuration (`.zenith.yml`):**
    ```yaml
    steps:
      - run: make build
      - run: make test
    ```
*   **Commands:** `zenith run`
*   **Features:** Sequential step execution, environment variable state management, streaming logs, exit status propogation.

## Phase 3: Matrix Runner (Multi-OS Pipeline)
*   **Goal:** Execute workflows across multiple operating systems locally and concurrently.
*   **Configuration:**
    ```yaml
    matrix:
      os: [ubuntu, alpine, debian]
    ```
*   **Commands:** `zenith matrix run`
*   **Features:** Parallel lab spawning, isolated process logs per OS, robust environment teardown.

## Phase 4: MicroVM Backend Support
*   **Goal:** Integrate hardware-level virtualized environments that boot in milliseconds.
*   **Backends:** Add support for Firecracker, QEMU, KVM.
*   **Features:** Real Linux kernel execution, sub-second boot times (<200ms), 5MB memory overhead per VM.

## Phase 5: Cross-OS and Cross-Arch Emulation
*   **Goal:** Support true multi-platform testing seamlessly from a single host (e.g., ARM tests on x86).
*   **Tech Integrations:**
    *   `qemu-user-static` (Cross-architecture mapping: ARM, RISC-V on x86).
    *   `wine` (Executing Windows `.exe` on Linux/macOS).
    *   `darling` (Executing macOS binaries on Linux).

## Phase 6: Build & Cache System
*   **Goal:** Implement caching for speed akin to layered Docker builds or Bazel.
*   **Commands:** `zenith build`, `zenith cache`.
*   **Features:** Hashing environment variables and toolchains, caching intermediate steps to skip redundant work.

## Phase 7: Environment & Package System
*   **Goal:** Provide reproducible, declarative setups without necessarily booting a VM (similar to Nix flakes or Devbox).
*   **Commands:** `zenith env init`, `zenith env shell`
*   **Features:** Resolving runtimes based on `.zenith.yml` (e.g., pulling exact Node.js or Python versions locally into an isolated scope).

## Phase 8: Plugin Architecture
*   **Goal:** Allow Zenith to be extended by third-party runners, VMs, or execution hooks.
*   **Commands:** `zenith plugin install firecracker`, `zenith plugin install wasm`

## Phase 9 & 10: Remote, Distributed, and Cloud Runtime
*   **Goal:** Use the same CLI to push workloads to remote servers or directly to a specialized cloud platform.
*   **Commands:** `zenith remote add server1`, `zenith run --remote`, `zenith cloud run`.

## Phase 11+: Developer Platform & Final Vision
*   **GUI / IDE Integration:** `zenith ui`, dashboard, VSCode/JetBrains extensions.
*   **Low-Level System Tuning:** Bespoke minimal `rootfs`, optimized `init`, fine-tuned KVM integrations.
*   **Ultimate Goal:** Establishing Zenith as an *OS-level Developer Runtime*, fundamentally modifying how engineers approach local development across varying systems.
