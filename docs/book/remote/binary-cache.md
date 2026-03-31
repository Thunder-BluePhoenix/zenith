# Remote Binary Cache

The remote binary cache is a shared HTTP store for build outputs. Any machine that produces a build can push it; any other machine can pull it — skipping re-execution entirely.

---

## How it works

The cache key is a **derivation ID** — a SHA-256 hash of all inputs to a build step (command, env, OS, arch, watched file contents). Because the hash is deterministic, two machines building the same code produce identical keys.

Before executing a step, Zenith checks:

1. **Local store** (`~/.zenith/store/`) — instant restore if hit
2. **Remote cache** — download + populate local store if hit
3. **Execute** — on success, commit to local store and optionally push to remote

---

## Configuration

### In `.zenith.yml` (v2)

```yaml
cache:
  remote: "https://cache.myteam.example.com"
  push: true       # automatically push after every successful build
  ttl_days: 30
```

### Via CLI (one-time setup)

```bash
zenith cache remote https://cache.myteam.example.com --push
```

This saves the configuration to `~/.zenith/config.toml` under `[cache]`.

### View current configuration

```bash
zenith cache remote --status
# Remote cache: https://cache.myteam.example.com
# Push enabled: yes
```

---

## Wire protocol

The remote cache server must implement three HTTP endpoints:

| Method | Path | Description |
|---|---|---|
| `HEAD` | `/store/{drv_id}` | Returns `200` if the entry exists, `404` if not |
| `GET` | `/store/{drv_id}` | Returns a `tar.gz` archive of the outputs directory |
| `PUT` | `/store/{drv_id}` | Accepts a `tar.gz` archive body |

Authentication uses a bearer token in the `Authorization` header:

```
Authorization: Bearer <api_key>
```

The API key is configured in `~/.zenith/config.toml`:

```toml
[cache]
remote  = "https://cache.myteam.example.com"
push    = true
api_key = "your-secret-key"
```

---

## Typical team setup

**CI machine** (pushes):
```yaml
cache:
  remote: "https://cache.myteam.example.com"
  push: true
```

**Developer laptop** (pulls only):
```yaml
cache:
  remote: "https://cache.myteam.example.com"
  push: false    # or omit — default is false
```

CI builds warm the cache. Developers get instant cache hits for steps whose inputs haven't changed since the last CI run.

---

## Running your own cache server

Any HTTP server that implements the three endpoints above works. A minimal example using Caddy as a file server with `PUT` support:

```
# Caddyfile — simple static file cache server
cache.example.com {
    root * /data/zenith-cache
    file_server
    @put method PUT
    handle @put {
        # delegate to a backend that writes files
    }
}
```

For production use, consider an object store (S3, GCS, Cloudflare R2) with a thin HTTP adapter in front.

---

## Cache commands

```bash
zenith cache remote <url>               # set remote URL (pull only)
zenith cache remote <url> --push        # set URL and enable push
zenith cache remote --status            # show current configuration
zenith cache list                       # list local step cache entries
zenith cache prune                      # remove entries older than TTL
zenith cache clean                      # remove all local cache entries
```
