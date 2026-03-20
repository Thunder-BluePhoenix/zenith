# Phase 13: Build System & Reproducibility Engine

## Objective

Go deeper than Phase 6's step caching. Build a full **content-addressable build system** where every output is uniquely identified by its inputs — like Nix derivations but embedded natively in Zenith. Given the same inputs, Zenith always produces bit-for-bit identical outputs, forever.

**Status: NOT STARTED**

---

## Milestones & Tasks

### Milestone 13.1 — Derivation Model

**Why:** The current cache uses a hash of command + env + file contents. A derivation makes this contract explicit and composable — the hash of a derivation's inputs _is_ the derivation's identity. Downstream derivations can depend on upstream ones by hash.

**Tasks:**

1. **Introduce the `Derivation` concept** in `src/build/derivation.rs`:
   ```rust
   pub struct Derivation {
       pub name:    String,
       pub inputs:  Vec<DerivationInput>,  // files, env vars, other derivation hashes
       pub command: String,
       pub outputs: Vec<String>,           // output paths
   }
   ```
   - Serializes to a deterministic JSON form (sorted keys, stable format)
   - Its SHA-256 is the build's unique identity

2. **`zenith build --derivation`** — evaluate and print a derivation without executing it
   - Lets users inspect exactly what Zenith considers as inputs before running

3. **Extend `Step` in `src/config.rs`** with `depends_on: Vec<String>` to express inter-step dependencies
   - Steps with no dependencies can run in parallel within a job

---

### Milestone 13.2 — Content-Addressable Store

**Tasks:**

1. **Local build store at `~/.zenith/store/<hash>/`**
   - Each successfully built derivation stores its outputs under this path
   - Multiple projects that produce the same artifact (same hash) share the store entry — no duplication
   - GC policy: entries unreferenced for more than `ttl_days` are pruned

2. **`zenith store gc`** — garbage collect unreferenced store entries

3. **`zenith store verify`** — re-hash all store entries and report any corruption

---

### Milestone 13.3 — Remote Binary Cache

**Tasks:**

1. **Configure a remote cache URL** in `~/.zenith/config.toml`:
   ```toml
   [cache]
   remote = "https://cache.zenith.run"
   ```

2. **Before building, query the remote cache**
   - `HEAD /store/<hash>` — check if the derivation is already built
   - On hit: `GET /store/<hash>` — download and restore to the local store; skip local build entirely

3. **After building, push to the remote cache** (if `push = true` in config)
   - `PUT /store/<hash>` — upload the output archive
   - Used for team-shared caches: one developer's build saves all others

4. **Signed cache entries**
   - Each uploaded entry is signed with the uploader's key
   - Downloaders verify the signature before trusting the cached output

---

### Milestone 13.4 — Parallel Step Execution

**Why:** Independent steps within a job currently run sequentially. With explicit `depends_on:` relationships, independent steps can run concurrently.

**Tasks:**

1. **Build a dependency graph** from `step.depends_on` fields at job start
   - Topological sort to determine execution order
   - Steps with no unfulfilled dependencies start immediately

2. **Execute independent steps in parallel using `JoinSet`** (same pattern as matrix jobs)

3. **`zenith build --dry-run`** — print the execution graph without running anything

---

## Key Files

| File | Role |
|---|---|
| `src/build/derivation.rs` | `Derivation` struct, hash computation, JSON serialization |
| `src/build/store.rs` | Local content-addressable store, GC, verify |
| `src/build/remote_cache.rs` | Remote cache client (query, download, upload, sign) |
| `src/config.rs` | Add `depends_on` to `Step` |
| `src/runner.rs` | Parallel step execution based on dependency graph |

---

## Verification Checklist

- [ ] Same `.zenith.yml` + same source files = same derivation hash on any machine
- [ ] `zenith build --derivation` prints a stable JSON derivation (same output on repeated runs)
- [ ] Derivation output stored in `~/.zenith/store/<hash>/`
- [ ] Two projects with identical build steps share a single store entry
- [ ] `zenith build` fetches a pre-built artifact from the remote binary cache (no local compilation)
- [ ] `zenith store gc` removes entries older than configured TTL
- [ ] Independent steps in a job run in parallel when `depends_on` allows it
