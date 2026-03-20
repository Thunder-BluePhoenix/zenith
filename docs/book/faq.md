# FAQ

## Do I need Docker?

No. Zenith has its own container backend that uses Linux namespaces directly. Docker is not required. The Firecracker backend uses real microVMs. The Wasm backend runs steps inside WebAssembly sandboxes.

## How is Zenith different from `act`?

`act` re-runs GitHub Actions workflows locally using Docker. Zenith is a native runtime with first-class caching, parallel step execution, content-addressable builds, remote binary caches, and custom VM/Wasm backends.

## Can I use Zenith in CI?

Yes. Zenith is designed to run identically locally and in CI. The remote binary cache means a local developer's build warms the CI cache automatically.

## What does "cold start in under 50ms" mean?

When using the Firecracker backend with `runs-on: zenith`, Zenith boots a custom minimal Linux kernel (stripped to ~500 options) directly into a `zenith-init` PID 1 binary — no systemd, no shell, no SSH. From kernel start to your step executing in under 50ms.

## Where is the cache stored?

- Step cache: `~/.zenith/cache/`
- Build store: `~/.zenith/store/`
- Toolchains: `~/.zenith/toolchains/`
- Kernel/rootfs: `~/.zenith/kernel/`, `~/.zenith/rootfs/`

## How do I reset the cache?

```
zenith cache clean       # clear step cache
zenith store gc 0        # clear all build store entries
```
