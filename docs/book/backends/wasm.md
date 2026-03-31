# WebAssembly Backend

The Wasm backend runs `.wasm` modules via an auto-downloaded `wasmtime` runtime. It is the only backend that works identically on Linux, macOS, and Windows.

---

## Enabling

```yaml
jobs:
  test-wasm:
    backend: wasm
    steps:
      - name: Run module
        run: my-app.wasm --flag
```

---

## Auto-provisioning

Zenith downloads `wasmtime` automatically on first use into `~/.zenith/bin/wasmtime`. No system install required.

---

## WASI support

The backend uses WASI (WebAssembly System Interface) to give modules controlled access to:

- **Filesystem:** the step's working directory is mounted read-write; the rest of the host filesystem is not accessible
- **Environment variables:** all step env vars are passed through to the module via WASI `--env`
- **Stdio:** stdout and stderr flow back to Zenith's log output normally

---

## Example: compiled Rust module

```yaml
version: "2"

jobs:
  wasm-pipeline:
    backend: wasm
    steps:
      - name: Compile
        run: cargo build --release --target wasm32-wasi
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/wasm32-wasi/release/mymodule.wasm]

      - name: Test
        run: mymodule.wasm --test-mode
        depends_on: [Compile]
        env:
          TEST_SUITE: unit
```

> **Note:** The `Compile` step uses the host Rust toolchain (or the one declared in `env:`). Only the `Test` step runs inside the Wasm sandbox.

---

## Cross-platform behaviour

Because Wasm is architecture-neutral, the same `.wasm` binary runs on x86_64 and aarch64 without recompilation. This makes the Wasm backend useful for:

- Plugin testing across platforms
- Tools that must run on CI machines with mixed host architectures
- Distributing build tools as `.wasm` modules that Zenith executes directly

---

## Limitations

- No network access (WASI does not yet expose sockets in wasmtime's default config)
- No `fork()`/`exec()` — the module runs as a single process
- Only `.wasm` (WASI) modules; browser-targeted Wasm (`wasm32-unknown-unknown`) is not supported

---

## When to use

- Testing `.wasm` output from your build pipeline
- Running cross-platform build tools packaged as Wasm
- Lightweight sandboxing on macOS or Windows where Firecracker is unavailable
