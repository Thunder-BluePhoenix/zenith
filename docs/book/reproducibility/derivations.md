# Derivation Model

Zenith's reproducibility engine is built on **derivations** — a concept borrowed from Nix. A derivation is a complete, deterministic description of a build step. Two derivations with identical content always produce the same outputs.

---

## What a derivation captures

```json
{
  "name": "Build",
  "command": "cargo build --release",
  "os": "alpine",
  "arch": "x86_64",
  "inputs": [
    "src/main.rs",
    "src/lib.rs",
    "Cargo.toml",
    "Cargo.lock"
  ],
  "outputs": [
    "target/release/myapp"
  ],
  "env": {
    "CARGO_TERM_COLOR": "always"
  }
}
```

The derivation ID is a SHA-256 hash of this JSON (with keys sorted for determinism). The same inputs on any machine produce the same 64-character hex ID.

---

## Printing derivations (dry run)

```bash
zenith build --derivation
```

This computes and prints the derivation JSON for each step without executing anything. Useful for auditing exactly what Zenith considers part of a build's identity.

---

## How derivation IDs are used

1. **Local store lookup:** Zenith checks `~/.zenith/store/<drv_id>/` — if outputs exist, they are restored and the step is skipped
2. **Remote cache lookup:** Zenith checks the remote cache server at `/store/<drv_id>` — if present, it downloads the outputs tarball
3. **Build store commit:** On a successful build, Zenith commits the outputs to the local store under the derivation ID
4. **Remote push:** If `cache.push = true`, the tarball is uploaded to the remote cache server

---

## Reproducibility guarantee

If two machines have:
- The same `command`, `os`, `arch`, `env`
- The same content in all `inputs` files

They produce the same derivation ID. If one machine has already built it and pushed to the remote cache, the second machine gets the output instantly — guaranteed to be identical.

---

## Derivation chaining

When step B `depends_on` step A, step B's derivation includes step A's derivation ID as an input. This means:

- Any change to step A's inputs propagates through to step B's derivation ID
- The entire dependency graph is content-addressed
- You can never get a stale downstream cache hit from a changed upstream

---

## Inspecting stored derivations

```bash
zenith store list
# DRV ID                                                          BUILT                SIZE
# a3f8c2e1...b9d4f7a0  (Build)                                   2026-03-31 10:05    2.1MB
# 7e2d91c4...f0a53b8e  (Test)                                    2026-03-31 10:06    0KB

zenith store info a3f8c2e1...b9d4f7a0
# Derivation:
#   name:    Build
#   command: cargo build --release
#   os:      alpine
#   arch:    x86_64
#   inputs:  [src/main.rs, Cargo.toml, Cargo.lock]
#   outputs: [target/release/myapp]
```
