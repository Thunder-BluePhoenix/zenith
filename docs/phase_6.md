# Phase 6: Build & Cache System

## Objective

Make Zenith fast through intelligent caching. Instead of re-running every step on every `zenith run`, hash the inputs of each step and skip execution when nothing has changed. This is how Nix, Bazel, and Docker layer caching work — and it is the difference between a 30-second CI loop and a 3-second one.

## Current State in the Codebase

A basic `CacheManager` already exists at `src/sandbox/cache.rs`. It:
- Hashes step command + env vars + OS + arch using SHA-256
- Writes a marker file to `~/.zenith/cache/<hash>` on success
- Checks for that file to skip steps on re-run

**What is missing:**
- Cache is keyed only on the step command string — file content changes are not detected
- No cache invalidation strategy (stale entries live forever)
- No `zenith cache` CLI subcommand to inspect or clean the cache
- No layer/artifact caching (only step-level "did it succeed" markers)
- No content-addressable output storage (Nix/Bazel style)
- `zenith build` command does not exist yet

---

## Milestones & Tasks

### Milestone 1 — File-Content-Aware Hashing

**Why:** Right now, changing a source file does not invalidate the cache because the step command (`make build`) never changes. The hash must include the content of files the step depends on.

**Tasks:**

1. **Add a `watch` field to `Step` in `src/config.rs`**
   ```yaml
   steps:
     - name: Build
       run: make build
       watch:
         - src/**/*.rs
         - Cargo.toml
   ```
   Add `watch: Option<Vec<String>>` to the `Step` struct.

2. **Implement glob-based file hashing in `src/sandbox/cache.rs`**
   - Add a method `hash_watched_files(patterns: &[String]) -> String`
   - Walk matching files (use the `glob` crate), sort paths for determinism
   - SHA-256 hash their contents in order, return a combined hex digest

3. **Feed the file hash into `compute_step_hash`**
   - If `step.watch` is `Some(patterns)`, call `hash_watched_files` and mix that digest into the step hash
   - The final hash now changes whenever watched file contents change

**Files to modify:** `src/config.rs`, `src/sandbox/cache.rs`
**New dependency:** `glob = "0.3"` in `Cargo.toml`

---

### Milestone 2 — Cache TTL and Invalidation

**Why:** Without expiry, the cache grows unbounded and stale entries can silently persist across environment changes (e.g., upgrading a toolchain).

**Tasks:**

1. **Store metadata alongside the cache marker**
   - Instead of writing `"SUCCESS"` to the file, write a JSON blob:
     ```json
     { "created_at": 1710000000, "os": "alpine", "arch": "x86_64", "run": "make build" }
     ```
   - Use `serde_json` for serialization (add `serde_json = "1.0"` to `Cargo.toml`)

2. **Implement cache TTL check in `is_cached`**
   - Read the metadata file, parse `created_at`
   - If the entry is older than a configurable TTL (default: 7 days), treat it as a cache miss
   - TTL can be set in the global config (`~/.zenith/config.toml`) under `[cache] ttl_days = 7`

3. **Add `zenith cache clean` subcommand**
   - In `src/cli.rs`, add `Cache` as a top-level command with a `clean` sub-action
   - Implementation: delete all files in `~/.zenith/cache/`
   - Also add `zenith cache list` to print all cached hashes with their metadata

**Files to modify:** `src/cli.rs`, `src/main.rs`, `src/sandbox/cache.rs`
**New dependency:** `serde_json = "1.0"` in `Cargo.toml`

---

### Milestone 3 — Artifact / Layer Caching (Output Caching)

**Why:** Step-level boolean caching ("did it pass?") is phase 1. The real power is caching the *output artifacts* of a step — build directories, compiled binaries, downloaded packages — so they can be restored on cache hit without re-building.

**Tasks:**

1. **Add an `outputs` field to `Step` in `src/config.rs`**
   ```yaml
   steps:
     - name: Build
       run: cargo build --release
       watch: [src/**/*.rs, Cargo.toml]
       outputs: [target/release/zenith]
   ```
   Add `outputs: Option<Vec<String>>` to the `Step` struct.

2. **Implement artifact archiving in `CacheManager`**
   - Add `fn save_artifacts(hash: &str, paths: &[String]) -> Result<()>`
   - For each path in `outputs`, tar.gz the content into `~/.zenith/cache/<hash>/artifacts.tar.gz`
   - Use the existing `flate2` + `tar` crates already in `Cargo.toml`

3. **Implement artifact restoration in `CacheManager`**
   - Add `fn restore_artifacts(hash: &str, workspace: &Path) -> Result<()>`
   - On cache hit, extract the artifact archive back into the workspace before skipping the step
   - This means the next step can use the compiled binary even though the build step was skipped

4. **Wire into the runner loop in `src/runner.rs`**
   - On cache hit: call `restore_artifacts` before marking step as skipped
   - On cache miss after success: call `save_artifacts` with the step's `outputs` paths

**Files to modify:** `src/config.rs`, `src/sandbox/cache.rs`, `src/runner.rs`

---

### Milestone 4 — `zenith build` Command

**Why:** Users need a dedicated build command separate from `zenith run`, allowing them to explicitly build and cache without running tests.

**Tasks:**

1. **Add `Build` variant to `Commands` enum in `src/cli.rs`**
   ```
   zenith build              # builds all jobs with build steps
   zenith build --job mybuild
   zenith build --no-cache   # force rebuild ignoring cache
   ```

2. **Add `--no-cache` flag support across runner**
   - Thread a `force: bool` parameter through `execute_local` and `execute_single_job`
   - When `force = true`, skip all `is_cached` checks

3. **Tag steps as "build" steps in the YAML schema**
   ```yaml
   steps:
     - name: Compile
       run: cargo build --release
       type: build   # or just use 'outputs' as the marker
   ```

**Files to modify:** `src/cli.rs`, `src/main.rs`, `src/runner.rs`

---

### Milestone 5 — Cross-Job Cache Sharing

**Why:** If `matrix.os = [ubuntu, alpine]` both compile the same Rust code, they produce identical artifacts (for the same arch). Sharing cache entries between matrix nodes halves build time.

**Tasks:**

1. **Normalize the hash key so it is OS-independent for pure build steps**
   - Add a `cache_key` override field to `Step` in `src/config.rs`
   - When `cache_key` is set, use it as the hash prefix instead of the OS/arch string
   - This lets the user opt-in to cross-OS cache sharing for steps that produce portable output

2. **Implement a shared cache pool in `CacheManager`**
   - Add `~/.zenith/cache/shared/` directory for cross-job entries
   - Regular per-job cache lives in `~/.zenith/cache/jobs/<hash>/`

---

## Key Files Reference

| File | Role |
|---|---|
| `src/sandbox/cache.rs` | Core cache logic — extend all methods here |
| `src/config.rs` | Add `watch`, `outputs`, `cache_key` fields to `Step` |
| `src/runner.rs` | Wire artifact save/restore around step execution |
| `src/cli.rs` | Add `zenith cache` and `zenith build` commands |
| `src/main.rs` | Route new commands to handlers |

---

## Verification Checklist

- [ ] Running `zenith run` twice with unchanged files: second run shows `[CACHED]` for all steps
- [ ] Changing a file matched by `watch` glob causes the step to re-run
- [ ] `zenith cache list` prints all cached step hashes with timestamps
- [ ] `zenith cache clean` deletes the cache and forces full rebuild on next run
- [ ] A step with `outputs: [target/release/zenith]` restores the binary on cache hit
- [ ] `zenith build --no-cache` re-runs all steps regardless of cache state
- [ ] Cache entries older than TTL are treated as misses

## Next Steps

With fast incremental builds established, Phase 7 builds the **Environment & Package System** — allowing users to declare exact toolchain versions (`node: 20`, `python: 3.12`) and have Zenith provision them without Docker or system-level installs.
