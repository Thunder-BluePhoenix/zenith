# Config Schema v2 Overview

Zenith uses a YAML configuration file named `.zenith.yml`. Schema v2 (introduced in Phase 14) formalises all features into a coherent structure.

## Top-level fields

| Field | Type | Description |
|---|---|---|
| `version` | `"1"` \| `"2"` | Schema version (default: `"1"`) |
| `env` | object | Global toolchain declarations |
| `cache` | object | Cache settings (v2 only) |
| `jobs` | object | Named job definitions |
| `steps` | array | Flat step list (legacy format) |

## Migrating from v1

Run:

```
zenith migrate
```

This prints the upgraded v2 config. To apply in-place:

```
zenith migrate --write
```

## Full v2 example

```yaml
version: "2"

env:
  node: "20"
  python: "3.12"

cache:
  ttl_days: 14
  remote: "https://cache.zenith.run"
  push: true

jobs:
  test:
    runs-on: alpine
    backend: firecracker
    arch: x86_64
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
