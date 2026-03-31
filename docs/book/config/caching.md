# Caching

Zenith caches at the step level. A cache hit means the step is skipped entirely and its outputs are restored from the archive — no re-execution.

---

## How the cache key is computed

The cache key is a SHA-256 hash of:

- OS and architecture
- All environment variables for the step (job-level + step-level merged)
- The exact `run:` command text
- The content of every file matched by `watch:` globs

If any of these change, the cache is invalidated and the step re-runs.

---

## Two-level cache

Before executing a step, Zenith checks:

1. **Local store** (`~/.zenith/store/`) — fastest; derivation ID match means instant restore
2. **Remote binary cache** (optional) — shared across machines; a miss falls through to execution
3. **Execute** — on success, commit to local store and optionally push to remote

---

## Watching files

```yaml
steps:
  - name: Install deps
    run: npm ci
    watch:
      - package.json
      - package-lock.json    # re-run when lockfile changes; skip otherwise
```

Glob patterns are supported:

```yaml
watch:
  - src/**/*.rs
  - Cargo.toml
  - Cargo.lock
```

---

## Saving outputs

```yaml
steps:
  - name: Compile
    run: cargo build --release
    watch: [src/**/*.rs, Cargo.toml]
    outputs:
      - target/release/myapp    # archived on success; restored on cache hit
      - target/release/myapp.d
```

On a cache hit, Zenith extracts the archive into the workspace before the step runs — downstream steps see the restored files immediately.

---

## Disabling cache

Per-step:

```yaml
steps:
  - name: Always runs
    run: date
    cache: false
```

Per-job:

```yaml
jobs:
  fresh:
    cache: false    # all steps in this job skip caching
```

At the CLI (one-time bypass):

```bash
zenith run --no-cache
```

---

## Manual cache key

Override the auto-computed hash for cross-OS artifact sharing:

```yaml
steps:
  - name: Download assets
    run: ./scripts/download-assets.sh
    cache_key: "assets-v3"    # same key regardless of OS/arch
    outputs: [assets/]
```

Use this when two matrix instances (e.g. `alpine` and `ubuntu`) need to share the same cached output.

---

## TTL and pruning

```yaml
cache:
  ttl_days: 14    # entries older than 14 days are pruned (default: 7)
```

```bash
zenith cache list            # show all entries with age
zenith cache prune           # remove entries older than TTL
zenith cache clean           # delete everything
```

---

## Remote binary cache

```yaml
cache:
  remote: "https://cache.myteam.example.com"
  push: true    # upload outputs after every successful build
```

Or configure once at the CLI:

```bash
zenith cache remote https://cache.myteam.example.com --push
zenith cache remote --status    # show current configuration
```

One developer's build warms the cache for the whole team and for CI. The cache key (derivation ID) is identical across machines because it is a hash of inputs — not of timestamps or host names.

---

## Cache commands

```bash
zenith cache list             # list all step cache entries
zenith cache prune            # remove expired entries
zenith cache clean            # remove all entries
zenith cache remote <url>     # configure remote cache URL
zenith cache remote <url> --push    # configure URL and enable auto-push
zenith cache remote --status        # show current remote cache config
```
