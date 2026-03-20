# Your First Workflow

A complete `.zenith.yml` for a Rust project with matrix testing:

```yaml
version: "2"

env:
  rust: stable

cache:
  ttl_days: 14
  remote: "https://cache.myteam.example.com"

jobs:
  test:
    strategy:
      matrix:
        os: [alpine, ubuntu]
    runs-on: ${{ matrix.os }}
    backend: container
    steps:
      - name: Build
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml, Cargo.lock]
        outputs: [target/release/]

      - name: Unit tests
        run: cargo test --lib
        depends_on: [Build]

      - name: Integration tests
        run: cargo test --test '*'
        depends_on: [Build]
```

Run:

```
zenith run
```

Zenith will:
1. Spawn two parallel matrix jobs (`alpine` and `ubuntu`)
2. In each job, run `Build` first, then `Unit tests` and `Integration tests` in parallel (since both depend only on `Build`)
3. Cache build outputs keyed by source hash — subsequent runs skip unchanged steps
