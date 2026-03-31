# Firecracker Backend

The Firecracker backend provides hardware-level microVM isolation using AWS Firecracker. Each job boots a real Linux kernel inside a KVM virtual machine — the strongest isolation level available in Zenith.

---

## Requirements

- Linux host with `/dev/kvm` enabled
- No root access required (user-space KVM is sufficient)

---

## Enabling

```yaml
jobs:
  build:
    runs-on: alpine
    backend: firecracker
```

---

## Auto-provisioning

Zenith downloads all required components automatically on first use:

| Component | Location |
|---|---|
| Firecracker binary | `~/.zenith/bin/firecracker` |
| Linux kernel (stock) | `~/.zenith/kernel/vmlinux` |
| Zenith custom kernel | `~/.zenith/kernel/vmlinux-zenith` |
| rootfs image | `~/.zenith/rootfs-fc/<os>.ext4` |

```bash
zenith tools download-kernel    # pre-download the Zenith custom kernel
zenith tools download-rootfs    # pre-download the Zenith minimal rootfs
zenith tools status             # show all downloaded components and sizes
```

---

## Zenith custom kernel (`runs-on: zenith`)

For the fastest possible boot time, use the Zenith-specific OS image:

```yaml
jobs:
  fast:
    runs-on: zenith       # custom kernel + zenith-init PID 1
    backend: firecracker
```

The Zenith custom kernel is compiled with only the options needed for CI:
- Disabled: sound, USB, Bluetooth, wireless, complex ACPI, PCI bus enumeration
- Enabled: virtio, 9p, overlayfs, KVM guest, vsock, minimal network

**Boot time:** < 50ms from kernel start to first step executing.

---

## zenith-init PID 1

When `runs-on: zenith`, the VM boots directly into `zenith-init` as PID 1 — not bash, not systemd. `zenith-init`:

1. Mounts `/proc`, `/sys`, `/dev`
2. Opens the serial console (ttyS0) to receive the step command from the host
3. Executes the command with `execve`, forwarding stdout/stderr with `O:`/`E:` prefixes
4. Writes `EXIT:<code>` when the command finishes
5. Powers off the VM via `/proc/sysrq-trigger`

No SSH, no shell overhead, no extra processes.

---

## Warm VM pool

When the Zenith daemon is running, it maintains a pool of pre-booted Firecracker VMs in snapshot state. `zenith run` restores a snapshot instead of cold-booting:

```bash
zenith daemon start --pool 4
zenith run    # < 1ms startup — snapshot restore
```

Without the daemon, each `zenith run` cold-boots a new VM (~50ms with the Zenith kernel).

---

## Per-run rootfs snapshots

Each Firecracker job gets a copy-on-write snapshot of the rootfs ext4 image. Changes made during the job (installed packages, created files) are written to the snapshot and discarded when the job finishes. The master image is never modified.

---

## Security properties

- **Separate kernel:** each job has its own kernel instance in a KVM guest
- **No shared memory:** host memory is not accessible from inside the VM
- **No host filesystem access:** the VM only sees the rootfs snapshot and the workspace virtio-9p mount
- **Network isolation:** the VM has its own network namespace

---

## When to use

- Security-sensitive workloads (fuzz testing, auditing untrusted code)
- Maximum reproducibility (hardware-isolated, no host process interference)
- Compliance requirements
- When the container backend's namespace isolation is insufficient
