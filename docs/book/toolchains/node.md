# Node.js Toolchain

Zenith downloads official Node.js binary releases and prepends them to `PATH` before every step. No `nvm`, no system Node, no version conflicts.

---

## Declaration

```yaml
env:
  node: "20.11.0"    # exact version string
```

Or per-job:

```yaml
jobs:
  legacy:
    toolchain:
      node: "16.20.2"
```

---

## What gets downloaded

Zenith fetches from the official Node.js distribution:

- **Linux/macOS:** `node-v{version}-{platform}-{arch}.tar.gz`
- **Windows:** `node-v{version}-win-{arch}.zip`

The archive is extracted into `~/.zenith/toolchains/node/{version}/` and the `bin/` subdirectory is prepended to `PATH`.

After installation, `node`, `npm`, and `npx` all resolve to the declared version.

---

## Example workflow

```yaml
version: "2"

env:
  node: "20.11.0"

jobs:
  frontend:
    runs-on: alpine
    steps:
      - name: Verify Node version
        run: node --version    # prints v20.11.0

      - name: Install
        run: npm ci
        watch: [package-lock.json]
        outputs: [node_modules/]

      - name: Lint
        run: npm run lint
        depends_on: [Install]

      - name: Test
        run: npm test
        depends_on: [Install]

      - name: Build
        run: npm run build
        watch: [src/**/*.ts]
        outputs: [dist/]
        depends_on: [Lint, Test]
```

---

## Multi-version matrix

```yaml
jobs:
  compat:
    strategy:
      matrix:
        node: ["18.20.0", "20.11.0", "22.0.0"]
    toolchain:
      node: ${{ matrix.node }}
    steps:
      - name: Test
        run: npm test
```

---

## Management

```bash
zenith env init      # download Node.js (and all other declared toolchains)
zenith env list      # show installed versions and paths
zenith env clean     # remove all downloaded toolchains
```
