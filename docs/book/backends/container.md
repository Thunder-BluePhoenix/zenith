# Container Backend

The container backend is Zenith's default isolation engine on Linux. It uses Linux kernel namespaces directly — no Docker daemon, no root access, no KVM required.

---

## Enabling

```yaml
jobs:
  build:
    runs-on: alpine
    backend: container    # this is the default; can be omitted
```

---

## How it works

Each step executes inside a set of Linux namespaces:

- **PID namespace** — the step process cannot see host processes
- **Mount namespace** — the step has its own filesystem view (OverlayFS over the rootfs)
- **Network namespace** — isolated network stack (no access to host network by default)

The rootfs image for the selected `runs-on` OS is downloaded once into `~/.zenith/rootfs/` and reused across all jobs via OverlayFS. The upper (writable) layer is per-job, so steps never modify the base image.

---

## Fallback on non-Linux

On macOS and Windows, full namespace isolation is not available. Zenith falls back to a cleaned subprocess: a child process started with a minimal environment (no host env vars leaked) running inside the project workspace. The behaviour is functionally equivalent for most workloads but provides no kernel-level isolation.

| Platform | Isolation |
|---|---|
| Linux | Full PID + mount + network namespace |
| macOS | Cleaned subprocess |
| Windows | Cleaned subprocess |

---

## OverlayFS upper layers

On Linux, each lab environment gets its own OverlayFS upper layer. Changes made during a step (installed packages, generated files) land in the upper layer and are discarded when the lab is destroyed. The lower layer (base rootfs) is read-only and shared.

---

## Supported OS images

| `runs-on` value | Image |
|---|---|
| `alpine` | Alpine Linux (default) |
| `ubuntu` | Ubuntu LTS |
| `debian` | Debian stable |
| `alpine-arm64` | Alpine Linux for aarch64 |

Images are downloaded from official CDN sources the first time they are needed.

---

## When to use

- Default for most workloads on Linux
- When KVM is not available
- For fast startup with lightweight isolation
- On macOS/Windows where Firecracker is not supported
