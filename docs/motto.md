# Zenith Motto: Zero External Dependencies

> **"You install Zenith. Zenith installs everything else."**

---

## The Principle

Every tool Zenith needs — Firecracker binaries, QEMU user-mode emulators, wasmtime runtimes, Linux rootfs images, language toolchains — is downloaded, cached, and managed by Zenith itself.

The user never runs:
```
apt install firecracker
brew install qemu
npm install -g node
pip install python
```

Zenith handles all of it.

---

## How It Works in Practice

| What Zenith needs | How Zenith gets it | Where it is cached |
|---|---|---|
| Alpine / Ubuntu rootfs | Downloads from official CDN | `~/.zenith/rootfs/<os>/` |
| Firecracker VMM binary | Downloads from AWS GitHub releases | `~/.zenith/bin/firecracker` |
| QEMU user-mode static | Downloads prebuilt static binary | `~/.zenith/bin/qemu-<arch>-static` |
| wasmtime CLI | Downloads from Bytecode Alliance GitHub | `~/.zenith/bin/wasmtime` |
| Node.js, Python, Go, Rust | Downloads official release tarballs | `~/.zenith/toolchains/<name>/<version>/` |
| Plugin binaries | Downloaded via `zenith plugin install` | `~/.zenith/plugins/<name>/` |

Everything lives under `~/.zenith/`. To fully uninstall Zenith, delete that directory.

---

## Implementation Pattern

Every external tool follows the same pattern, implemented in `src/tools.rs`:

```rust
// Before using firecracker:
let fc_path = tools::ensure_tool("firecracker", FIRECRACKER_VERSION, FIRECRACKER_URL).await?;

// Before cross-arch execution:
let qemu_path = tools::ensure_tool("qemu-aarch64-static", QEMU_VERSION, QEMU_URL).await?;

// Before running .wasm:
let wasm_path = tools::ensure_tool("wasmtime", WASMTIME_VERSION, WASMTIME_URL).await?;
```

The `ensure_tool` function:
1. Checks if the binary exists at `~/.zenith/bin/<name>`
2. If yes: returns the path immediately (zero overhead)
3. If no: downloads, extracts, marks executable, returns path

---

## Why This Matters

1. **Zero setup friction** — `cargo install zenith`, done. Run a workflow. Everything else is automatic.
2. **Reproducibility** — Zenith pins exact versions of its tool dependencies. Two developers using the same Zenith version use the same Firecracker version.
3. **Clean uninstall** — `rm -rf ~/.zenith` removes every trace of Zenith's installations.
4. **No privilege escalation** — everything installs in user home directory, no `sudo` required.
5. **Offline after first run** — once cached, Zenith works completely offline.

---

## Platform Behavior

| Platform | Firecracker | QEMU user-mode | wasmtime | Toolchains |
|---|---|---|---|---|
| Linux x86_64 | Full support (KVM) | Full support | Full support | Full support |
| Linux aarch64 | Full support (KVM) | Full support | Full support | Full support |
| Windows | Not supported (no KVM) | Not supported | Full support | Full support |
| macOS | Partial (no KVM) | Not supported | Full support | Full support |

On Windows and macOS, Zenith falls back to namespace-based isolation for the container backend and uses wasmtime for WASM targets. Firecracker and QEMU require Linux with KVM.
