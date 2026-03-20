# Phase 9 & 10: Remote Runner & Cloud Runtime

## Objective

Break Zenith out of the local machine. Phase 9 lets users push workflows to remote servers they already own (SSH targets, dedicated build machines). Phase 10 connects to an official Zenith cloud service for serverless, ephemeral Firecracker execution — same `.zenith.yml` syntax, zero local setup required.

This is where Zenith becomes: **your CI system, your way, anywhere.**

---

## Phase 9: Remote Runner (`zenith remote`)

### Concept

```bash
zenith remote add build-server user@192.168.1.10
zenith run --remote build-server
```

Zenith SSHes into the remote machine, streams the local project directory to it, starts the zenith agent on the remote (downloading it if needed), and then runs the exact same workflow as if it were local — streaming logs back in real time.

### Current State

Nothing from Phase 9 exists. The runner in `src/runner.rs` is entirely local. The remote concept will require a new `src/remote/` module and a lightweight `zenith-agent` binary.

---

### Milestones & Tasks

#### Milestone 9.1 — Remote Config & CLI

**Tasks:**

1. **Add `Remote` subcommand to `src/cli.rs`**:
   ```
   zenith remote add <name> <ssh-target>     # register a remote (e.g., user@host)
   zenith remote list                         # show registered remotes
   zenith remote remove <name>               # unregister a remote
   zenith remote status <name>               # ping the remote, check agent version
   ```

2. **Create `~/.zenith/remotes.toml`** to persist remote definitions:
   ```toml
   [remotes.build-server]
   host = "user@192.168.1.10"
   port = 22
   key = "~/.ssh/id_ed25519"   # optional override
   ```
   Add a `RemoteConfig` struct in `src/remote/config.rs` and load/save via `serde` + `toml`.

3. **Add `--remote <name>` flag to the `Run` command in `src/cli.rs`**:
   ```
   zenith run --remote build-server
   zenith run --remote build-server --job test
   ```

---

#### Milestone 9.2 — SSH Transport Layer

**Why:** We need to copy files to the remote and run commands on it. Use the `openssh` or `ssh2` crate rather than shelling out to `ssh`, for proper programmatic control.

**Tasks:**

1. **Add SSH dependency to `Cargo.toml`**:
   ```toml
   openssh = { version = "0.10", features = ["native-mux"] }
   ```

2. **Create `src/remote/transport.rs`**
   - `async fn connect(remote: &RemoteConfig) -> Result<openssh::Session>`
   - `async fn upload_project(session: &Session, local: &Path, remote_path: &str) -> Result<()>`
     - Use `sftp` or `rsync`-style chunked transfer
     - Upload only files not in `.gitignore` (respect `.zenith/` and `target/` exclusions)
   - `async fn run_remote_command(session: &Session, cmd: &str) -> Result<String>`
     - Execute a shell command on the remote and return stdout
   - `async fn stream_remote_output(session: &Session, cmd: &str) -> Result<()>`
     - Like above but streams output line by line to the local terminal as it arrives

---

#### Milestone 9.3 — Zenith Agent (Remote Daemon)

**Why:** On the remote machine, something must receive commands and execute them within Zenith's sandbox system. This is the `zenith-agent` — a lightweight companion binary.

**Tasks:**

1. **Create a second binary target in `Cargo.toml`**:
   ```toml
   [[bin]]
   name = "zenith-agent"
   path = "src/agent/main.rs"
   ```

2. **Create `src/agent/main.rs`**
   - The agent listens on a vsock or TCP socket (configurable, default port `7621`)
   - Accepts JSON-RPC messages: `{ "method": "run", "params": { "config": "...", "job": "..." } }`
   - Deserializes the config, calls `runner::execute_local` exactly as the local runner does
   - Streams stdout/stderr back over the same connection

3. **Auto-bootstrap the agent on the remote**
   - When `zenith run --remote <name>` is invoked:
     1. SSH into the remote
     2. Check if `zenith-agent` is already installed (`~/.zenith/bin/zenith-agent`)
     3. If not: upload the agent binary compiled for the remote's architecture
        - Zenith ships pre-compiled agents for `x86_64-linux`, `aarch64-linux`
        - Select the right one based on `uname -m` output from the remote
     4. Start the agent as a background process if not already running

---

#### Milestone 9.4 — Remote Execution Driver

**Tasks:**

1. **Create `src/remote/runner.rs`**
   - `async fn execute_remote(remote: &RemoteConfig, config: &ZenithConfig, job: Option<String>) -> Result<()>`
   - Connects to the remote via SSH
   - Uploads the project directory
   - Sends the workflow config to the agent
   - Streams logs back, prefixing each line with `[remote:<name>]`

2. **Wire into `src/main.rs`**: when `--remote` flag is present, call `remote::runner::execute_remote` instead of `runner::execute_local`

---

## Phase 10: Cloud Runtime (`zenith cloud`)

### Concept

```bash
zenith cloud run                    # run workflow on Zenith cloud
zenith cloud status <run-id>        # check status of a cloud run
zenith cloud logs <run-id>          # fetch logs from a completed run
```

The cloud backend is Zenith-as-a-service: spin up Firecracker VMs on Zenith-operated infrastructure, identical workflow syntax, no local VMs required.

### Milestones & Tasks

#### Milestone 10.1 — Cloud Auth & Config

**Tasks:**

1. **Add `zenith cloud login` command** — authenticate with the Zenith cloud API
   - Prompt for API key or OAuth token
   - Store in `~/.zenith/config.toml` under `[cloud] api_key = "..."`

2. **Add `zenith cloud` subcommand tree to `src/cli.rs`**:
   ```
   zenith cloud login                    # authenticate
   zenith cloud run                      # submit current workflow
   zenith cloud run --watch              # submit and stream logs live
   zenith cloud status <run-id>          # poll status
   zenith cloud logs <run-id>            # fetch logs
   zenith cloud cancel <run-id>          # cancel a running job
   zenith cloud list                     # list recent cloud runs
   ```

---

#### Milestone 10.2 — Cloud API Client

**Tasks:**

1. **Create `src/cloud/client.rs`** — HTTP API client
   - Use `reqwest` (already in `Cargo.toml`) with JSON bodies
   - Base URL configurable via `[cloud] api_url = "https://api.zenith.run"` in config
   - Methods:
     - `async fn submit_run(api_key: &str, config: &str, project_tar: Vec<u8>) -> Result<String>` — returns run ID
     - `async fn get_status(api_key: &str, run_id: &str) -> Result<RunStatus>`
     - `async fn stream_logs(api_key: &str, run_id: &str) -> Result<()>` — uses server-sent events (SSE)
     - `async fn cancel_run(api_key: &str, run_id: &str) -> Result<()>`

2. **Create a `RunStatus` enum** in `src/cloud/types.rs`:
   ```rust
   pub enum RunStatus { Queued, Running, Success, Failed, Cancelled }
   ```

---

#### Milestone 10.3 — Project Packaging for Cloud

**Why:** The cloud service needs the project files to execute the workflow. Package them into a tar.gz before uploading.

**Tasks:**

1. **Create `src/cloud/packager.rs`**
   - `fn package_project(dir: &Path) -> Result<Vec<u8>>`
   - Walk the directory, skip `.git`, `target`, `.zenith/` (same exclusions as local sandbox)
   - Create an in-memory `tar.gz` using `flate2` + `tar` (already in `Cargo.toml`)
   - Return the raw bytes for the HTTP upload body

---

#### Milestone 10.4 — Live Log Streaming

**Why:** `zenith cloud run --watch` should stream logs in real time, not poll after completion. This is the UX that makes cloud CI feel local.

**Tasks:**

1. **Implement SSE (Server-Sent Events) parsing in `src/cloud/client.rs`**
   - The cloud API sends log lines as SSE: `data: [alpine] Step 1: npm test\n\n`
   - Parse SSE frames and print each `data:` line to stdout as it arrives
   - Use `reqwest`'s streaming body (`response.bytes_stream()`)

2. **Handle connection drops with auto-reconnect**
   - SSE disconnect: wait 2 seconds, reconnect with `Last-Event-ID` header
   - Max reconnect attempts: 5 (then fail gracefully with a message)

---

## Key Files Reference

| File | Role |
|---|---|
| `src/remote/mod.rs` | Remote module root |
| `src/remote/config.rs` | `RemoteConfig` struct, `remotes.toml` parse/save |
| `src/remote/transport.rs` | SSH connection, file upload, command streaming |
| `src/remote/runner.rs` | High-level remote execution orchestration |
| `src/agent/main.rs` | `zenith-agent` binary — remote workflow executor |
| `src/cloud/mod.rs` | Cloud module root |
| `src/cloud/client.rs` | HTTP API calls (submit, status, logs, cancel) |
| `src/cloud/types.rs` | `RunStatus`, `RunInfo` types |
| `src/cloud/packager.rs` | Project tar.gz builder |
| `src/cli.rs` | Add `remote`, `cloud` command trees; `--remote` flag on `run` |
| `src/main.rs` | Route remote/cloud commands |
| `Cargo.toml` | Add `openssh`, `serde_json` (if not yet added) |

---

## Verification Checklist

**Phase 9 — Remote:**
- [ ] `zenith remote add build-server user@host` persists to `~/.zenith/remotes.toml`
- [ ] `zenith run --remote build-server` uploads the project and runs the workflow on the remote
- [ ] Logs stream back live from the remote machine with `[remote:build-server]` prefix
- [ ] `zenith-agent` is auto-installed on the remote if not present
- [ ] SSH key auth works without a password prompt

**Phase 10 — Cloud:**
- [ ] `zenith cloud login` stores the API key securely
- [ ] `zenith cloud run` submits the workflow and prints a run ID
- [ ] `zenith cloud run --watch` streams logs in real time until completion
- [ ] `zenith cloud status <run-id>` shows current state
- [ ] `zenith cloud cancel <run-id>` stops a running job

## Next Steps

With the full execution surface covered — local, remote, cloud — Phase 11-15 focuses on the **Developer Platform**: GUI dashboards, IDE integrations, a custom hypervisor, and ultimately an OS-level runtime.
