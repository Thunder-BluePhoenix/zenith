# Installation

## Prerequisites

- Rust 1.75 or later (for building from source)
- Linux (for Firecracker backend) or any OS (for container/wasm backends)

## From source

```bash
git clone https://github.com/zenith-run/zenith
cd zenith
cargo install --path .
```

## Verify

```
zenith --version
zenith tools status
```

## First-time setup

Download the Zenith custom kernel and rootfs (Linux only, for the Firecracker backend):

```
zenith tools download-kernel
zenith tools download-rootfs
```

Both files are cached in `~/.zenith/` and verified with SHA-256 before use.
