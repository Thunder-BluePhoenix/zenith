# Zenith

**Zenith** is a local multi-OS workflow runtime. You install Zenith — Zenith installs everything else.

Run your CI/CD pipelines on your own machine, in isolated sandboxes, with full caching and reproducible builds. No Docker daemon required (though it is supported). No cloud account needed (though that's supported too).

## Why Zenith?

| Feature | Zenith | act | Docker Compose |
|---|---|---|---|
| Firecracker VM sandboxing | ✓ | ✗ | ✗ |
| Content-addressable build store | ✓ | ✗ | ✗ |
| Parallel step execution with dep graph | ✓ | partial | ✗ |
| Automatic toolchain provisioning | ✓ | ✗ | manual |
| Remote binary cache | ✓ | ✗ | ✗ |
| Plugin ecosystem | ✓ | ✗ | ✗ |

## Quick example

```yaml
version: "2"

env:
  rust: stable

jobs:
  ci:
    runs-on: alpine
    backend: firecracker
    steps:
      - name: Build
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/release/myapp]

      - name: Test
        run: cargo test
        depends_on: [Build]
```

```
zenith run
```

That's it. Zenith provisions Rust, boots a Firecracker microVM, runs your steps in parallel, caches the outputs — and does it all in under 50ms.
