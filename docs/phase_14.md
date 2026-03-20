# Phase 14: Full Developer Platform

## Objective

Unite all previous phases into a seamless **Universal Developer Runtime**. Phase 14 is integration and polish work — a revised config schema that covers every feature, a performance benchmarking suite, and a documentation site that makes Zenith accessible to new users.

**Status: NOT STARTED**

---

## Milestones & Tasks

### Milestone 14.1 — Unified Config Schema v2

**Why:** The `.zenith.yml` schema has grown organically across phases 0–13. v2 formalises all features into a single coherent schema with versioning, better defaults, and backward compatibility.

**Tasks:**

1. **Revise `.zenith.yml` to support all features introduced in Phases 6–13**:
   ```yaml
   version: 2

   env:
     node: "20"
     python: "3.12"

   cache:
     ttl_days: 14
     remote: "https://cache.zenith.run"

   jobs:
     test:
       runs-on: alpine
       backend: firecracker
       arch: aarch64
       strategy:
         matrix:
           os: [ubuntu, alpine]
       steps:
         - name: Build
           run: cargo build --release
           watch: [src/**/*.rs]
           outputs: [target/release/myapp]
           depends_on: []
         - name: Test
           run: cargo test
           depends_on: [Build]
   ```

2. **Write a migration guide** from v1 to v2 schema
   - `zenith migrate` command that reads a v1 `.zenith.yml` and writes an upgraded v2 version

3. **JSON Schema v2** for IDE validation
   - Update `vscode-zenith/schemas/zenith-schema.json` with all new fields
   - Add `version` field validation

---

### Milestone 14.2 — Performance Benchmarking Suite

**Why:** Performance is a core Zenith value. Automated benchmarks ensure regressions are caught before release.

**Tasks:**

1. **Create `benches/` directory** with Criterion benchmarks:
   - `cold_start` — time from `zenith run` to first step executing (container backend)
   - `cache_hit` — time saved by a cache hit vs full step re-run
   - `matrix_spawn` — time to launch N parallel matrix jobs
   - `rootfs_extract` — time to decompress and mount a rootfs image
   - `config_parse` — time to parse and validate `.zenith.yml`

2. **Run benchmarks in CI** — add a GitHub Actions workflow that runs `cargo bench` and saves results as artifacts

3. **`zenith benchmark` command** — runs the benchmark suite and prints a human-readable performance report
   - Compares against a stored baseline (`~/.zenith/bench-baseline.json`)
   - Highlights regressions > 10%

---

### Milestone 14.3 — Documentation Site

**Why:** A CLI tool is only as powerful as its documentation. Phase 14 makes Zenith approachable with a first-class docs site.

**Tasks:**

1. **Set up `mdBook`** (Rust-native, compiles to static HTML)
   - `book.toml` at repo root
   - Sections: Getting Started, Configuration Reference, Backends, Toolchains, Plugins, Remote & Cloud, Dashboard & TUI, Reproducibility

2. **Convert existing docs** from `docs/*.md` to mdBook chapters
   - Add interactive `.zenith.yml` examples that users can copy and run immediately
   - Add an FAQ section based on common user questions

3. **Auto-publish on release** — GitHub Actions workflow: `cargo test && mdbook build && deploy to GitHub Pages`

4. **`zenith docs`** command — opens the documentation site in the default browser

---

### Milestone 14.4 — Plugin Ecosystem Improvements

**Tasks:**

1. **Plugin registry index** — a hosted `registry.toml` listing community plugins:
   ```
   zenith plugin search <query>
   zenith plugin install <name>     # install from registry (not just local path)
   ```

2. **Plugin versioning** — `requires_zenith: ">=0.2.0"` in `plugin.toml`; Zenith checks compatibility on install

3. **Plugin sandboxing** — run plugin processes in a restricted environment (limited filesystem access, no network by default)

---

## Key Files

| File | Role |
|---|---|
| `src/config.rs` | v2 schema structs, `version` field, migration helpers |
| `benches/zenith_benchmarks.rs` | Criterion benchmark suite |
| `book.toml` | mdBook config |
| `src/plugin/registry.rs` | Extend with remote registry fetch |

---

## Verification Checklist

- [ ] `.zenith.yml` v2 schema passes JSON Schema validation with all feature combinations
- [ ] `zenith migrate` upgrades a v1 config to v2 without data loss
- [ ] `cargo bench` runs all benchmarks and produces stable results
- [ ] Benchmark CI job fails when a cold-start regression > 10% is detected
- [ ] Documentation site builds with `mdbook build` and is browsable
- [ ] `zenith plugin search <query>` returns results from the hosted registry
- [ ] `zenith docs` opens the documentation site in the browser
