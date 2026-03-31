# Zenith Cloud

The Zenith cloud service provides fully managed remote execution. Submit your `.zenith.yml` workflow and Zenith runs it on cloud infrastructure — no server setup, no SSH keys, no agent installation.

---

## Authentication

```bash
zenith cloud login <your-api-key>
```

The API key is stored in `~/.zenith/config.toml`. To remove it:

```bash
zenith cloud logout
```

---

## Submitting a run

```bash
# Submit and stream logs live
zenith cloud run --watch

# Submit and return a run ID immediately
zenith cloud run

# Submit a specific job
zenith cloud run --job test --watch
```

Zenith packages your project (excluding `.git/`, `target/`, `node_modules/`, etc.) as a multipart upload and submits it to the cloud API.

---

## Monitoring a run

```bash
# Poll status
zenith cloud status <run-id>
# run-id: r_abc123
# status: running
# job: ci
# started: 2026-03-31T10:05:22Z

# Stream logs (SSE)
zenith cloud logs <run-id>
# [ci] Running: cargo build --release
# [ci] Compiling zenith v0.1.0
# ...
```

---

## Listing past runs

```bash
zenith cloud list
# RUN ID       JOB    STATUS     STARTED              DURATION
# r_abc123     ci     success    2026-03-31 10:05     1m 23s
# r_def456     test   failed     2026-03-31 09:50     0m 42s
# r_ghi789     ci     success    2026-03-30 18:12     1m 19s
```

---

## Cancelling a run

```bash
zenith cloud cancel <run-id>
```

---

## How it compares to remote machines

| Feature | `zenith cloud` | `zenith remote` |
|---|---|---|
| Infrastructure | Managed by Zenith | Your own servers |
| Setup | API key only | SSH + zenith-agent |
| OS | Managed by cloud | Your server's OS |
| Cost | Usage-based | Your hardware |
| Firecracker support | Yes (on cloud VMs) | Yes (if server has KVM) |

---

## Notes

- The cloud service uses the same `.zenith.yml` format as local execution — no new syntax
- The cloud's build cache is separate from your local `~/.zenith/store/`, but both can share a remote binary cache if configured
- Log streaming uses SSE (Server-Sent Events) over HTTP; `zenith cloud run --watch` connects to this stream automatically
