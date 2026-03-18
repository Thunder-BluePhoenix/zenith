# Phase 1: Lab Environments (Sandbox)

## Objective
Transform Zenith into a local sandboxing tool capable of fetching, mounting, and executing code inside isolated filesystems (Lab environments). This is the realization of **Idea 2**.

## Technical Approach
Instead of booting full virtual machines immediately, Phase 1 utilizes process-level isolation (`chroot`, Linux Namespaces, or container delegates like `podman`/`docker` in rootless mode) alongside `overlayfs` to create ephemeral, throwaway workspaces.

## Milestones

1.  **Rootfs Image Management**
    *   Implement an image downloader that fetches minimal root filesystems (e.g., Alpine Linux `minirootfs.tar.gz`, Ubuntu Cloud Images).
    *   Extract and locally cache these base images in `~/.zenith/cache/rootfs/<os>-<version>`.
2.  **Filesystem Isolation (OverlayFS/Bind Mounts)**
    *   When an environment is created, mount the base static `rootfs` as the lower directory in an OverlayFS.
    *   Create a temporary writeable upper layer in `~/.zenith/labs/<lab-id>/`.
    *   Bind-mount the user's current project directory context into the lab (e.g., mounted at `/workspace`).
3.  **Lab Lifecycle Commands**
    *   `zenith lab create <os>`: Generates the upper layer and readies the sandbox.
    *   `zenith lab shell <os>`: Spawns an interactive `/bin/sh` or `/bin/bash` process *inside* the isolated rootfs environment.
    *   `zenith lab run <os> <cmd>`: Executes a specific command synchronously inside the lab context.
    *   `zenith lab destroy <os>`: Unmounts the overlay and deletes the upper directory, resetting the state instantly.
4.  **Process Jail (Linux Only v1)**
    *   Wrap execution inside `chroot` or `unshare` to pivot into the new root before running the target command.

## Verification
*   Running `zenith lab run alpine "cat /etc/os-release"` returns Alpine Linux details, regardless of the host OS.
*   Writing a file inside `zenith lab shell ubuntu` does not modify the host machine, except inside the explicitly mounted project directory.
*   Destroying the lab removes all temporary system changes made inside the sandbox.

## Next Steps
With functional, ephemeral Lab environments ready, Phase 2 will introduce the automated Workflow Engine to orchestrate multi-step pipelines within these labs.
