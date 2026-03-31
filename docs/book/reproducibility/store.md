# Content-Addressable Build Store

The build store (`~/.zenith/store/`) is Zenith's persistent artifact store. Every successful build step that declares `outputs:` has its results committed here, indexed by derivation ID.

---

## Structure on disk

```
~/.zenith/store/
  a3f8c2e1b9d4f7a0.../
    outputs/
      target/release/myapp     # restored to workspace on cache hit
    meta.json                  # derivation + timestamp
  7e2d91c4f0a53b8e.../
    outputs/
    meta.json
```

Each entry is self-contained. The `outputs/` directory mirrors the structure of the files declared in `outputs:` relative to the workspace root.

---

## When the store is populated

1. A step with `outputs:` runs successfully
2. Zenith stages the declared output paths into a temporary directory
3. The staging directory is committed to the store under the derivation ID
4. If `cache.push = true`, the entry is also uploaded to the remote cache

---

## When the store is read

Before executing any step with `outputs:`, Zenith checks the store:

1. Compute the derivation ID for this step
2. Check `~/.zenith/store/<drv_id>/` — if found, restore `outputs/` into the workspace and skip execution
3. If not found locally, check the remote cache (if configured)
4. If neither hits, execute and commit on success

---

## Store commands

```bash
# List all stored entries
zenith store list
# DRV ID           NAME              BUILT                SIZE
# a3f8c2e1...      Build             2026-03-31 10:05    2.1MB
# 7e2d91c4...      Install deps      2026-03-31 10:04    18.4MB

# Inspect a specific entry
zenith store info a3f8c2e1b9d4f7a0...
# name:    Build
# command: cargo build --release
# os:      alpine
# outputs: target/release/myapp
# built:   2026-03-31T10:05:33Z
# size:    2.1MB

# Remove entries older than N days
zenith store gc 30       # remove entries not accessed in 30 days
zenith store gc 0        # remove all entries
```

---

## Difference from the step cache

| | Step cache (`~/.zenith/cache/`) | Build store (`~/.zenith/store/`) |
|---|---|---|
| Key | SHA-256 of command + env + files | Derivation ID (full input graph) |
| Content | Output artifact tarball | Structured outputs directory |
| Provenance | Not stored | Full derivation JSON in `meta.json` |
| Remote sharing | Remote binary cache | Remote binary cache |
| Gc command | `zenith cache prune` | `zenith store gc` |

The build store is the higher-fidelity system introduced in Phase 13. For most users the distinction is invisible — Zenith checks both automatically.

---

## Garbage collection

The store grows over time as new derivations are added. Run GC periodically or configure a TTL:

```bash
zenith store gc 30    # remove entries not accessed in 30 days
```

In `.zenith.yml`:

```yaml
cache:
  ttl_days: 14    # applies to both the step cache and the build store
```
