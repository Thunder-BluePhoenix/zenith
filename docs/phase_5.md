# Phase 5: Cross-OS and Cross-Arch Runtime

## Objective
Fulfill the "Ultimate Multi-OS" promise by allowing developers to test and build software meant for entirely different system architectures and non-Linux operating systems, completely offline and locally.

## Technical Approach
We will utilize emulation and translation layers integrated into Zenith's backend abstractor.

## Milestones

1.  **Cross-Architecture Execution (QEMU User Mode)**
    *   Integrate `qemu-user-static`/`binfmt_misc`.
    *   This allows Zenith to run an ARM64 rootfs on an x86_64 host transparently.
    *   Example configuration target: `runs-on: ubuntu-latest-arm64`. Zenith pulls the ARM image, mounts it using Phase 1/4 components, and transparently executes binaries through QEMU translation.
2.  **Windows Executable Sandboxing (Wine integration)**
    *   To test a previously-built Windows `.exe` locally on a Linux host, Zenith will auto-provision a fresh, ephemeral Wine prefix.
    *   Target: `runs-on: windows-wine`. Zenith configures the prefix, hides host config, maps the working directory, and executes the `.exe` inside it.
3.  **macOS Binary Sandboxing (Darling/Lima)**
    *   Integrate `Darling` (macOS translation layer for Linux) or `Lima` instances.
    *   Provides developers working on Linux or Windows a mechanism to run macOS build chains and unit tests sequentially.
4.  **WebAssembly Runtime (Wasmtime)**
    *   Add WebAssembly as a primary target OS platform.
    *   Run compiled `.wasm` artifacts through Wasmtime directly within Zenith to validate WASI interfaces securely.

## Verification
*   A user on an x86 Ubuntu machine successfully runs a Rust `.yaml` test workflow targeting `aarch64` native binaries.
*   A Windows graphical/cli binary test executes locally via a `.zenith.yml` command on a macOS/Linux laptop without requiring a 60GB Windows VM.

## Next Steps
The core runtime functionality is complete. Output and execution are solid. Phase 6 and beyond will focus on performance improvements, specifically advanced reproducible Environment caching (similar to Nix) and Build caching.
