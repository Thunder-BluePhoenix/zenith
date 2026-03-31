# Toolchain Declarations

Zenith provisions language runtimes automatically. Declare the versions you need in `.zenith.yml` — Zenith downloads them into `~/.zenith/toolchains/` and prepends their `bin/` directories to `PATH` before every step.

No `nvm`, `pyenv`, `rbenv`, or system package managers needed.

---

## Global declarations

```yaml
env:
  node:   "20.11.0"
  python: "3.12.3"
  go:     "1.22.1"
  rust:   "stable"
```

These versions are used by every job unless overridden.

---

## Per-job overrides

```yaml
jobs:
  legacy-api:
    toolchain:
      node: "16.0.0"    # overrides the global env: node for this job only
```

The resolution order (later wins): global `env:` → job-level `toolchain:`.

---

## Supported toolchains

| Key | Source | Notes |
|---|---|---|
| `node` | nodejs.org official releases | Extracts binary tarball; `.zip` on Windows |
| `python` | python-build-standalone (Astral) | Fully standalone — no system Python dependency |
| `go` | go.dev official releases | Extracts into `~/.zenith/toolchains/go/<version>/` |
| `rust` | rustup-init | Isolated `CARGO_HOME` and `RUSTUP_HOME` per version |

---

## Download and installation

Toolchains are downloaded lazily — on first use by a step that needs them. To download all declared toolchains up front (useful at the start of a CI run):

```bash
zenith env init
```

All downloads are cached in `~/.zenith/toolchains/`. Once downloaded, subsequent runs start instantly.

---

## Management commands

```bash
zenith env init      # download all toolchains declared in .zenith.yml
zenith env list      # show installed toolchains and their bin paths
zenith env shell     # open $SHELL with all toolchains on PATH
zenith env clean     # remove all downloaded toolchains
```

---

## How PATH injection works

Before executing each step, Zenith builds a modified `PATH`:

```
~/.zenith/toolchains/node/20.11.0/bin:
~/.zenith/toolchains/python/3.12.3/bin:
~/.zenith/toolchains/go/1.22.1/bin:
~/.zenith/toolchains/rust/stable/bin:
<original PATH>
```

The Zenith-managed versions shadow any system-installed versions. `node`, `python3`, `go`, `cargo`, `rustc` all resolve to the declared versions.

---

## Pinning for reproducibility

Always use exact version strings (not ranges) to guarantee identical builds across machines and time:

```yaml
# Good — exact
env:
  node: "20.11.0"
  python: "3.12.3"

# Less predictable — "latest" in a range
env:
  node: "20"       # Zenith will pin this to the latest 20.x at download time
```
