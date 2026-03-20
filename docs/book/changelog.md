# Changelog

## v0.1.0 (current)

### Phase 0–5: Core runtime
- Local workflow execution from `.zenith.yml`
- Container sandbox backend (Linux namespaces)
- Matrix strategy (parallel multi-OS jobs)
- Step caching with SHA-256 content hashing
- Artifact archiving (`outputs:` + `watch:`)

### Phase 6: Step Caching
- `watch:` glob patterns for cache invalidation
- `outputs:` artifact archive/restore
- `cache_key:` manual override for cross-OS sharing
- `zenith cache list / clean / prune`

### Phase 7: Toolchain Declarations
- `env:` block — auto-provision Node, Python, Go, Rust
- `zenith env init / shell / list / clean`

### Phase 8: Plugin System
- `zenith plugin install / remove / list / info / search`
- Plugin versioning: `requires_zenith`

### Phase 9: Remote Build Machines
- `zenith remote add / list / remove / status`
- SSH-based remote execution

### Phase 10: Cloud Service
- `zenith cloud login / run / status / logs / cancel / list`

### Phase 11: Dashboard & TUI
- `zenith ui` — web dashboard
- `zenith tui` — terminal dashboard

### Phase 12: Low-Level Optimization
- Custom stripped Linux kernel (boot in <50ms)
- `zenith-init` PID 1 — no shell, no SSH, pure `execve`
- Firecracker warm-pool: restore from snapshot (~10ms)
- Minimal rootfs (<5MB)

### Phase 13: Reproducibility Engine
- Derivation model (`zenith build --derivation`)
- Content-addressable build store (`zenith store list / gc / info`)
- Remote binary cache (`zenith cache remote`)
- Parallel step execution with `depends_on` dependency graph

### Phase 14: Full Developer Platform
- Config schema v2 — `cache:` top-level block
- `zenith migrate` — v1 → v2 upgrade
- JSON Schema v2 for IDE validation
- Criterion benchmark suite (`zenith benchmark`)
- Documentation site (`zenith docs`)
- Plugin registry search (`zenith plugin search`)
- Plugin versioning enforcement (`requires_zenith`)
