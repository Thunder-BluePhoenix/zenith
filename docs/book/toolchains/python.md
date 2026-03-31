# Python Toolchain

Zenith downloads fully standalone Python builds from the python-build-standalone project (maintained by Astral). These are self-contained binaries with no dependency on system Python, libpython, or any shared libraries.

---

## Declaration

```yaml
env:
  python: "3.12.3"
```

Or per-job:

```yaml
jobs:
  legacy:
    toolchain:
      python: "3.10.14"
```

---

## What gets downloaded

Zenith fetches a pre-built standalone Python archive from the python-build-standalone release CDN. The archive is extracted into `~/.zenith/toolchains/python/{version}/` and the `bin/` subdirectory is prepended to `PATH`.

After installation, `python3`, `python`, and `pip` all resolve to the declared version.

---

## Example workflow

```yaml
version: "2"

env:
  python: "3.12.3"

jobs:
  ml-pipeline:
    runs-on: ubuntu
    steps:
      - name: Verify Python version
        run: python3 --version    # prints Python 3.12.3

      - name: Install deps
        run: pip install -e ".[test]"
        watch: [pyproject.toml, requirements*.txt]
        outputs: [.venv/]

      - name: Type check
        run: mypy src/
        depends_on: [Install deps]

      - name: Test
        run: pytest tests/ -v --tb=short
        depends_on: [Install deps]
```

---

## Multi-version matrix

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
        watch: [pyproject.toml]
      - name: Test
        run: pytest tests/
        depends_on: [Install]
```

---

## Virtual environments

The standalone Python includes `pip` and `venv`. Standard virtual environment usage works without modification:

```yaml
steps:
  - name: Create venv
    run: python3 -m venv .venv
  - name: Install
    run: .venv/bin/pip install -r requirements.txt
    depends_on: [Create venv]
    outputs: [.venv/]
```

---

## Management

```bash
zenith env init      # download Python (and all other declared toolchains)
zenith env list      # show installed versions and paths
zenith env clean     # remove all downloaded toolchains
```
