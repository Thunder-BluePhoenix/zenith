# Phase 9: Remote Runner

## Objective

Break Zenith out of the local machine. Phase 9 lets users push workflows to remote servers they already own — SSH targets, dedicated build machines — using the exact same `.zenith.yml` syntax, with logs streaming back in real time.

```bash
zenith remote add build-server user@192.168.1.10
zenith run --remote build-server
```

**Status: COMPLETE**

---

## Concept

Zenith SSHes into the remote machine, streams the local project directory to it, starts the zenith agent on the remote (downloading it automatically if needed), then runs the exact same workflow as if it were local — streaming logs back with a `[remote:<name>]` prefix.

---

## Milestones & Tasks

### Milestone 9.1 — Remote Config & CLI

**Tasks:**

1. **Add `Remote` subcommand to `src/cli.rs`**:
   ```
   zenith remote add <name> <ssh-target>     # register a remote
   zenith remote list                         # show registered remotes
   zenith remote remove <name>               # unregister a remote
   zenith remote status <name>               # ping the remote, check connectivity
   ```

2. **Create `~/.zenith/remotes.toml`** to persist remote definitions:
   ```toml
   [remotes.build-server]
   host = "user@192.168.1.10"
   port = 22
   key = "~/.ssh/id_ed25519"   # optional override
   ```
   `RemoteConfig` struct in `src/remote/config.rs`, loaded/saved via `serde` + `toml`.

3. **Add `--remote <name>` flag to the `Run` command**:
   ```
   zenith run --remote build-server
   zenith run --remote build-server --job test
   ```

---

### Milestone 9.2 — SSH Transport Layer

**Why:** Use the system `ssh` binary via `tokio::process::Command` — zero extra dependencies, works wherever OpenSSH is installed (Linux / macOS / Windows 10+).

**Tasks:**

1. **Create `src/remote/transport.rs`**
   - `ping()` → `ssh host 'uname -m'` — check reachability and detect arch
   - `package_project()` → in-memory tar.gz, excluding `.git`, `target`, `.zenith`, `node_modules`
   - `upload_project()` → pipe tarball to `ssh host 'tar xz -C <dir>'`
   - `bootstrap_agent()` → check `~/.zenith/bin/zenith-agent`, upload current binary via SSH pipe if absent
   - `run_agent()` → pipe JSON task to `zenith-agent --agent-mode`, stream stdout/stderr back with `[remote:<name>]` prefix

---

### Milestone 9.3 — Zenith Agent (Remote Executor)

**Why:** On the remote machine, something must receive the workflow and execute it inside Zenith's runner. This is `zenith-agent` — a lightweight companion binary sharing the same crate.

**Tasks:**

1. **Create a second binary target in `Cargo.toml`**:
   ```toml
   [[bin]]
   name = "zenith-agent"
   path = "src/agent/main.rs"
   ```

2. **Create `src/agent/main.rs`**
   - Only activates with `--agent-mode` flag
   - Reads a JSON task from stdin: `{ "config_yaml": "...", "job": null, "workspace": "/tmp/..." }`
   - Writes config to a temp file, calls `zenith::runner::execute_local`
   - Streams stdout/stderr back to the SSH connection

3. **Create `src/lib.rs`** — shared library target
   - All modules declared `pub` so both `zenith` and `zenith-agent` binaries can `use zenith::...`
   - Add `default-run = "zenith"` to `[package]` in `Cargo.toml`

---

### Milestone 9.4 — Remote Execution Driver

**Tasks:**

1. **Create `src/remote/runner.rs`**
   - `execute_remote()` orchestrates: ping → package → upload → bootstrap → run_agent

2. **Wire into `src/main.rs`**: when `--remote` flag is present, call `remote::runner::execute_remote` instead of `runner::execute_local`

---

## Key Files

| File | Role |
|---|---|
| `src/remote/mod.rs` | Remote module root |
| `src/remote/config.rs` | `RemoteConfig` struct, `remotes.toml` parse/save |
| `src/remote/transport.rs` | SSH connection, file upload, command streaming |
| `src/remote/runner.rs` | High-level remote execution orchestration |
| `src/agent/main.rs` | `zenith-agent` binary — remote workflow executor |
| `src/lib.rs` | Shared library crate root (dual-binary pattern) |
| `src/cli.rs` | `remote` subcommand tree; `--remote` flag on `run` |
| `src/main.rs` | Route remote commands to handlers |

---

## Verification Checklist

- [x] `zenith remote add build-server user@host` persists to `~/.zenith/remotes.toml`
- [x] `zenith remote status <name>` pings and reports remote arch
- [x] `zenith run --remote build-server` uploads the project and runs the workflow on the remote
- [x] Logs stream back live with `[remote:build-server]` prefix
- [x] `zenith-agent` is auto-installed on the remote if not present
- [x] Both `zenith` and `zenith-agent` compile from the same crate via `src/lib.rs`
