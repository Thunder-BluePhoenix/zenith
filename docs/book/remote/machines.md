# Remote Build Machines

Zenith can run workflows on any machine reachable over SSH. This lets you offload heavy builds to a powerful server while developing on a laptop, or run Linux-only backends (Firecracker, namespace isolation) from a macOS or Windows host.

---

## Adding a remote

```bash
zenith remote add <name> <user@host> [--port N] [--key /path/to/key]
```

Examples:

```bash
zenith remote add buildbox deploy@192.168.1.50
zenith remote add ci-server ci@build.mycompany.com --port 2222 --key ~/.ssh/ci_rsa
```

Remotes are stored in `~/.zenith/remotes.toml`.

---

## Checking a remote

```bash
zenith remote status buildbox
# buildbox: reachable — arch=x86_64, zenith-agent v0.1.0 installed
```

This pings the remote via SSH and reports its architecture. If `zenith-agent` is not installed, Zenith installs it automatically on the next `zenith run --remote`.

---

## Running on a remote

```bash
zenith run --remote buildbox
zenith run --remote buildbox --job compile
zenith run --remote buildbox --no-cache
```

Zenith will:
1. Package your project as a `.tar.gz` (excluding `.git/`, `target/`, `node_modules/`, etc.)
2. Upload the archive via SSH
3. Install `zenith-agent` on the remote if needed
4. Execute the workflow on the remote machine
5. Stream logs back in real time with a `[remote:buildbox]` prefix
6. Clean up the temporary workspace on the remote

---

## Managing remotes

```bash
zenith remote list
# NAME         HOST                       PORT   KEY
# buildbox     deploy@192.168.1.50        22     default
# ci-server    ci@build.mycompany.com     2222   ~/.ssh/ci_rsa

zenith remote remove buildbox
```

---

## zenith-agent

`zenith-agent` is a lightweight binary that runs on the remote machine and handles workflow execution. It is built into the Zenith release and auto-installed on the remote during the first `zenith run --remote`.

You can also install it manually on the remote:

```bash
# On the remote machine
cargo install --path . --bin zenith-agent
```

---

## Use cases

- **Offload to a build server:** Run `cargo build -j64` on a 64-core machine while developing on a laptop
- **Linux backends from macOS/Windows:** Use Firecracker or namespace isolation via an SSH jump to a Linux machine
- **Cross-architecture builds:** Remote to an arm64 server for native arm64 builds

---

## Notes

- The remote machine must have Rust installed to build `zenith-agent` on first use, or have `zenith-agent` pre-installed
- The remote's `~/.zenith/` cache is separate from your local cache — remote builds populate the remote cache
- If a remote binary cache is configured, both local and remote machines share it automatically
