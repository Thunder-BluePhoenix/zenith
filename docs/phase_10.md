# Phase 10: Cloud Runtime

## Objective

Connect Zenith to an official cloud service for serverless, ephemeral workflow execution. Same `.zenith.yml` syntax, zero local VM setup required. The cloud backend runs Firecracker VMs on Zenith-operated infrastructure.

```bash
zenith cloud run                    # run workflow on Zenith cloud
zenith cloud run --watch            # run and stream logs live
zenith cloud status <run-id>        # check status
zenith cloud logs <run-id>          # fetch logs from a completed run
```

**Status: COMPLETE**

---

## Milestones & Tasks

### Milestone 10.1 — Cloud Auth & Config

**Tasks:**

1. **Add `zenith cloud login` command** — store API key in `~/.zenith/config.toml` under `[cloud] api_key = "..."`

2. **Add `zenith cloud` subcommand tree to `src/cli.rs`**:
   ```
   zenith cloud login                    # authenticate
   zenith cloud logout                   # remove stored credentials
   zenith cloud run                      # submit current workflow
   zenith cloud run --watch              # submit and stream logs live
   zenith cloud status <run-id>          # poll status
   zenith cloud logs <run-id>            # fetch logs
   zenith cloud cancel <run-id>          # cancel a running job
   zenith cloud list                     # list recent cloud runs
   ```

3. **`load_cloud_config()` / `save_api_key()` / `clear_api_key()`** in `src/cloud/client.rs`
   - Config stored at `~/.zenith/config.toml`; `[cloud]` table section

---

### Milestone 10.2 — Cloud API Client

**Tasks:**

1. **Create `src/cloud/client.rs`** — `reqwest`-based HTTP client
   - `submit_run()` — multipart upload: config YAML + project tar.gz; returns run ID
   - `get_status()` — poll run state
   - `list_runs()` — recent cloud runs
   - `cancel_run()` — stop a running job
   - `stream_logs()` — SSE parsing + live print (see Milestone 10.4)

2. **Create `src/cloud/types.rs`**:
   ```rust
   pub struct RunInfo { pub run_id: String, pub status: String, pub created_at: String, pub updated_at: String }
   pub struct CloudConfig { pub api_key: Option<String>, pub api_url: String }
   ```

---

### Milestone 10.3 — Project Packaging

**Why:** The cloud service needs the project files to execute the workflow. Package them into a tar.gz before uploading — same exclusion rules as the local sandbox.

**Tasks:**

1. **Create `src/cloud/packager.rs`**
   - `package_project(dir: &Path) -> Result<Vec<u8>>`
   - Walk the directory, skip `.git`, `target`, `.zenith/`, `node_modules`
   - Build an in-memory `tar.gz` using `flate2` + `tar`
   - Return raw bytes for the multipart upload body

---

### Milestone 10.4 — Live Log Streaming (SSE)

**Why:** `zenith cloud run --watch` should stream logs in real time, not poll after completion.

**Tasks:**

1. **Implement SSE parsing in `stream_logs()`**
   - Cloud API sends log lines as SSE: `data: <log line>\n\n`
   - Parse SSE frames from `reqwest`'s byte stream (`response.bytes_stream()`)
   - Print each `data:` line to stdout as it arrives

2. **Handle `done` / `error` sentinel events**
   - `event: done` → stream complete, exit normally
   - `event: error` → print error message and exit with non-zero code

---

## Key Files

| File | Role |
|---|---|
| `src/cloud/mod.rs` | Cloud module root |
| `src/cloud/client.rs` | HTTP API calls (submit, status, logs, cancel, list) |
| `src/cloud/types.rs` | `RunInfo`, `CloudConfig` types |
| `src/cloud/packager.rs` | Project tar.gz builder |
| `src/cli.rs` | `cloud` subcommand tree |
| `src/main.rs` | Route cloud commands to `handle_cloud()` |
| `Cargo.toml` | `reqwest` (multipart, stream, json), `futures-util` |

---

## Verification Checklist

- [x] `zenith cloud login <key>` stores the API key in `~/.zenith/config.toml`
- [x] `zenith cloud logout` removes stored credentials
- [x] `zenith cloud run` packages the project and submits the workflow
- [x] `zenith cloud run --watch` streams logs in real time via SSE
- [x] `zenith cloud status <run-id>` prints current run state
- [x] `zenith cloud logs <run-id>` replays stored log stream
- [x] `zenith cloud cancel <run-id>` stops a running job
- [x] `zenith cloud list` shows recent cloud runs

---

## Next Steps

With local, remote, and cloud execution all implemented, Phase 11 adds the **visual layer**: a web dashboard, TUI, and VSCode extension.
