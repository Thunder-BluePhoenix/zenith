# Go Toolchain

Zenith downloads official Go binary releases from go.dev and prepends them to `PATH`. No system Go installation required.

---

## Declaration

```yaml
env:
  go: "1.22.1"
```

Or per-job:

```yaml
jobs:
  legacy-service:
    toolchain:
      go: "1.20.14"
```

---

## What gets downloaded

Zenith fetches from the official Go download CDN:

- **Linux:** `go{version}.linux-{arch}.tar.gz`
- **macOS:** `go{version}.darwin-{arch}.tar.gz`
- **Windows:** `go{version}.windows-{arch}.zip`

Extracted into `~/.zenith/toolchains/go/{version}/`. The `bin/` subdirectory is prepended to `PATH`, making `go` resolve to the declared version.

`GOPATH` and `GOMODCACHE` are set to directories inside `~/.zenith/toolchains/go/{version}/` to keep module caches isolated per version.

---

## Example workflow

```yaml
version: "2"

env:
  go: "1.22.1"

jobs:
  build:
    runs-on: alpine
    steps:
      - name: Verify Go version
        run: go version    # prints go version go1.22.1 linux/amd64

      - name: Download modules
        run: go mod download
        watch: [go.sum, go.mod]

      - name: Build
        run: go build -o bin/server ./cmd/server
        watch: [cmd/**/*.go, internal/**/*.go, go.sum]
        outputs: [bin/server]
        depends_on: [Download modules]

      - name: Test
        run: go test ./... -race
        depends_on: [Download modules]
```

---

## Cross-compilation

Combine Go's built-in cross-compilation with Zenith's `arch:` field:

```yaml
jobs:
  release:
    strategy:
      matrix:
        goarch: [amd64, arm64]
    env:
      GOOS: linux
      GOARCH: ${{ matrix.goarch }}
    toolchain:
      go: "1.22.1"
    steps:
      - name: Build
        run: go build -o bin/server-${{ matrix.goarch }} ./cmd/server
        watch: [cmd/**/*.go, go.sum]
        outputs: [bin/server-${{ matrix.goarch }}]
```

---

## Management

```bash
zenith env init      # download Go (and all other declared toolchains)
zenith env list      # show installed versions and paths
zenith env clean     # remove all downloaded toolchains
```
