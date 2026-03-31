# Matrix Strategies

Matrix builds run the same job across multiple configurations in parallel. Each combination gets its own isolated workspace, cache entry, and log prefix.

---

## Defining a matrix

```yaml
jobs:
  test:
    strategy:
      matrix:
        os:      [alpine, ubuntu]
        version: ["18", "20", "22"]
```

This expands to **6 parallel instances** — every combination of `os` and `version`.

---

## Referencing matrix values

Use `${{ matrix.<key> }}` anywhere in the job or step definition:

```yaml
jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [alpine, ubuntu]
        node: ["18", "20"]
    env:
      NODE_VERSION: ${{ matrix.node }}
    steps:
      - name: Test on ${{ matrix.os }} / Node ${{ matrix.node }}
        run: |
          echo "OS: ${{ matrix.os }}"
          echo "Node: $NODE_VERSION"
          node --version
          npm test
```

---

## How cache keys work with matrix

Each matrix combination has its own cache entry. The OS and arch are part of the cache key, so:

- `alpine` + `node 18` → unique cache key
- `ubuntu` + `node 18` → different cache key
- `alpine` + `node 20` → different cache key

Changing the `run:` command or watched files invalidates only the affected combination.

---

## Commands

```bash
zenith matrix list    # preview all expanded combinations without running
zenith matrix run     # run all combinations in parallel
```

To run a single matrix job by name, use `zenith run --job <name>` (the job runs all matrix instances).

---

## Single-dimension example

```yaml
jobs:
  compat:
    strategy:
      matrix:
        python: ["3.10", "3.11", "3.12", "3.13"]
    toolchain:
      python: ${{ matrix.python }}
    steps:
      - name: Install
        run: pip install -e ".[test]"
        watch: [pyproject.toml, requirements*.txt]
      - name: Test
        run: pytest tests/ -x -q
        depends_on: [Install]
```

---

## Multi-dimension example

```yaml
jobs:
  cross:
    strategy:
      matrix:
        os:   [alpine, ubuntu]
        arch: [x86_64, aarch64]
    runs-on: ${{ matrix.os }}
    arch: ${{ matrix.arch }}
    steps:
      - name: Build ${{ matrix.os }} / ${{ matrix.arch }}
        run: cargo build --release
        watch: [src/**/*.rs, Cargo.toml]
        outputs: [target/release/myapp-${{ matrix.os }}-${{ matrix.arch }}]
```

This produces 4 parallel builds: alpine/x86_64, alpine/aarch64, ubuntu/x86_64, ubuntu/aarch64.
