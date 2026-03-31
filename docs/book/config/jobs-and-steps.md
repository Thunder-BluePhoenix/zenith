# Jobs & Steps

Jobs are the top-level units of work in Zenith. Each job is a named collection of steps that run inside an isolated sandbox. Steps are the individual commands that make up a job.

---

## Job definition

```yaml
jobs:
  my-job:
    runs-on: alpine          # OS image
    backend: container       # isolation engine
    arch: x86_64             # target CPU architecture
    env:
      MY_VAR: hello          # env vars for every step in this job
    working_directory: src   # default working directory for every step
    cache: true              # enable step caching (default: true)
    toolchain:               # per-job toolchain overrides
      node: "18.0.0"
    strategy:                # matrix expansion (see Matrix chapter)
      matrix:
        os: [alpine, ubuntu]
    steps:
      - ...
```

### Job fields

| Field | Type | Default | Description |
|---|---|---|---|
| `runs-on` | string | `alpine` | OS image to boot in the sandbox |
| `backend` | string | `container` | Isolation engine: `container`, `firecracker`, `wasm`, `wine`, or a plugin name |
| `arch` | string | host arch | Target CPU: `x86_64`, `aarch64`. Cross-arch auto-downloads QEMU |
| `env` | map | — | Environment variables injected into every step |
| `working_directory` | string | — | Default working directory. Relative to the sandbox workspace root |
| `cache` | bool | `true` | Master switch — disable to bypass step caching for the entire job |
| `toolchain` | object | — | Per-job version overrides (same keys as top-level `env:`) |
| `strategy` | object | — | Matrix expansion configuration |

---

## Step definition

```yaml
steps:
  - name: Build
    run: cargo build --release
    env:
      CARGO_TERM_COLOR: always
    working_directory: backend
    cache: true
    cache_key: my-custom-key   # override auto-generated hash key
    watch:
      - src/**/*.rs
      - Cargo.toml
    outputs:
      - target/release/myapp
    allow_failure: false
    depends_on:
      - Install deps
```

### Step fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | — | Human-readable label. Also used as the target name in `depends_on` |
| `run` | string | **required** | Shell command to execute |
| `env` | map | — | Additional env vars for this step only (merged with job-level env) |
| `working_directory` | string | — | Override the working directory for this step |
| `cache` | bool | `true` | Enable/disable caching for this specific step |
| `cache_key` | string | — | Manual cache key — replaces the auto-computed hash. Useful for cross-OS artifact sharing |
| `watch` | list | `[]` | Glob patterns. Cache is invalidated when any matched file's content changes |
| `outputs` | list | `[]` | Paths to archive on step success and restore on cache hit |
| `allow_failure` | bool | `false` | If `true`, a non-zero exit code is logged but does not abort the pipeline |
| `depends_on` | list | `[]` | Names of steps that must complete before this one starts |

---

## Environment variable priority

Variables are merged in this order (later wins):

1. Host environment (inherited by Zenith process)
2. Top-level `env:` block
3. Job-level `env:` block
4. Step-level `env:` block

---

## Working directory resolution

`working_directory` values are relative to the sandbox workspace root (`/workspace` inside the VM, or the project root in local mode). An absolute path is used as-is.

If neither the step nor the job sets `working_directory`, the sandbox workspace root is used.

---

## Multi-step example

```yaml
version: "2"

env:
  node: "20.11.0"

jobs:
  frontend:
    runs-on: alpine
    steps:
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

      # Lint and Test run in parallel.
      # Build only starts after both complete.
      - name: Build
        run: npm run build
        watch: [src/**/*.ts]
        outputs: [dist/]
        depends_on: [Lint, Test]
```
