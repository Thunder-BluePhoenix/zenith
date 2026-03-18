# Phase 4: MicroVM Backend Engine

## Objective
Upgrade Zenith’s isolation mechanism by integrating with MicroVM technology (specifically AWS Firecracker and Linux KVM). This transitions Zenith from a container-based sandbox to a true lightweight virtual machine manager with native kernel isolation capabilities.

## Technical Approach
Containers share the host kernel; VMs do not. Firecracker allows us to boot a custom Linux kernel + rootfs in ~150ms. Zenith will abstract the environment interface so users can swap `backend: container` for `backend: firecracker` seamlessly.

## Milestones

1.  **Backend Abstraction Layer**
    *   Refactor the Sandbox component to support different underlying hypervisor/runner interfaces via traits or interfaces.
    *   Available backends: `chroot`, `container` (legacy Phase 1), and `firecracker` (new).
2.  **Firecracker API Integration**
    *   Build a wrapper to interact with the Firecracker RESTful hypervisor API or via a rust-vmm crate interface.
    *   Manage the machine lifecycle: configure vCPUs, RAM, and network tap interfaces automatically.
3.  **Kernel Image Management**
    *   Supply a heavily stripped-down default Linux kernel (`vmlinux`) optimized for instant boot times.
    *   Download and cache this kernel alongside the user-defined `rootfs` from Phase 1.
4.  **Filesystem & Virtio Integration**
    *   Expose the host project directory into the Firecracker MicroVM. Since overlayfs doesn't cross the VM boundary easily, implement `virtio-fs`, `9p`, or a lightweight FUSE bridge to share the local codebase into the VM transparently.
5.  **Execution and Transport bridge**
    *   Pass the workflow steps into the MicroVM. This may require a tiny statically compiled `zenith-agent` binary running strictly as the init process (`PID 1`) inside the VM to receive commands via a virtio-serial port or vsock, execute them, and stream output back out to the host.

## Verification
*   `zenith lab shell ubuntu --backend firecracker` drops the user into a shell running a completely separate Linux kernel from the host.
*   The VM boot overhead adds no more than 200 milliseconds to the workflow execution time.
*   Changes made within the Firecracker block device reset instantly upon destruction, exhibiting highly immutable behavior.

## Next Steps
With true Kernel isolation established, Phase 5 will implement Emulator bridges (QEMU) handling cross-architecture and non-Linux testing environments.
