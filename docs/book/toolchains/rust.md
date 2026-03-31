# Rust Toolchain

Zenith installs Rust toolchains via `rustup-init` with fully isolated `CARGO_HOME` and `RUSTUP_HOME` directories. No system Rust or system `rustup` is touched.

---

## Declaration

```yaml
env:
  rust: "stable"     # channel: stable, beta, or nightly
```

Or a specific version:

```yaml
env:
  rust: "1.78.0"
```

Per-job override:

```yaml
jobs:
  nightly-features:
    toolchain:
      rust: nightly
```

---

## What gets installed

Zenith runs `rustup-init` (downloaded automatically) with the specified toolchain channel, setting:

- `CARGO_HOME` → `~/.zenith/toolchains/rust/{channel}/cargo/`
- `RUSTUP_HOME` → `~/.zenith/toolchains/rust/{channel}/rustup/`

The `bin/` subdirectory of `CARGO_HOME` is prepended to `PATH`, making `cargo`, `rustc`, and `rustfmt` all resolve to the declared toolchain.

---

## Example workflow

```yaml
version: "2"

env:
  rust: stable

jobs:
  ci:
    runs-on: alpine
    steps:
      - name: Verify Rust version
        run: cargo --version && rustc --version

      - name: Check
        run: cargo check
        watch: [src/**/*.rs, Cargo.toml, Cargo.lock]

      - name: Test
        run: cargo test --locked
        depends_on: [Check]

      - name: Clippy
        run: cargo clippy -- -D warnings
        depends_on: [Check]

      - name: Build release
        run: cargo build --release --locked
        watch: [src/**/*.rs, Cargo.toml, Cargo.lock]
        outputs: [target/release/myapp]
        depends_on: [Test, Clippy]
```

---

## MSRV (minimum supported Rust version) testing

```yaml
jobs:
  msrv:
    strategy:
      matrix:
        rust: ["1.70.0", "1.75.0", stable]
    toolchain:
      rust: ${{ matrix.rust }}
    steps:
      - name: Test on Rust ${{ matrix.rust }}
        run: cargo test --locked
```

---

## Cross-compilation

To cross-compile, install the target inside the step:

```yaml
steps:
  - name: Add target
    run: rustup target add x86_64-unknown-linux-musl

  - name: Build musl
    run: cargo build --release --target x86_64-unknown-linux-musl
    outputs: [target/x86_64-unknown-linux-musl/release/myapp]
    depends_on: [Add target]
```

---

## Management

```bash
zenith env init      # download Rust (and all other declared toolchains)
zenith env list      # show installed channels and paths
zenith env clean     # remove all downloaded toolchains
```
