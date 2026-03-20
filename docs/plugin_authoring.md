# Writing a Zenith Plugin

Zenith plugins are standalone executables that communicate with Zenith over **JSON-RPC on stdio**.
Plugins can be written in any language — Rust, Go, Python, Node.js, shell scripts, etc.

---

## Directory Layout

A plugin lives in `~/.zenith/plugins/<name>/` and must contain:

```
~/.zenith/plugins/my-backend/
├── plugin.toml               ← required manifest
└── zenith-my-backend         ← executable (zenith-my-backend.exe on Windows)
```

---

## plugin.toml Manifest

```toml
[plugin]
name        = "my-backend"
version     = "1.0.0"
type        = "backend"          # backend | toolchain | syntax | logger
entrypoint  = "zenith-my-backend"
description = "My custom VM backend"
```

| Field       | Required | Description |
|---|---|---|
| name        | yes | Unique name. Used in `.zenith.yml` as `backend: my-backend` |
| version     | yes | SemVer string |
| type        | yes | Plugin category (only `backend` is active in Phase 8) |
| entrypoint  | yes | Binary filename inside the plugin directory |
| description | no  | Short human-readable description |

---

## Communication Protocol (JSON-RPC over stdio)

Zenith spawns the plugin binary for **each method call** and communicates via:
- **stdin** — one JSON request line (terminated by `\n`)
- **stdout** — one JSON response line (terminated by `\n`)
- **stderr** — forwarded directly to the terminal (safe to print logs here)

### Request format

```json
{"method": "provision", "params": {...}, "id": 1}
```

### Response format

Success:
```json
{"result": null, "error": null, "id": 1}
```

Error:
```json
{"result": null, "error": "something went wrong", "id": 1}
```

The `id` in the response must match the `id` in the request.

---

## Required Methods

### `name`
Returns the plugin's self-reported name. Used for smoke testing on install.

**Params:** `null`
**Response:** `{"result": "my-backend", ...}`

### `provision`
Called once before any steps run. Set up resources (create VM, download image, etc.).

**Params:**
```json
{
  "lab_id":      "alpine-abc123",
  "base_os":     "alpine",
  "target_arch": "x86_64"
}
```

### `execute`
Called once per workflow step. Run the command.

**Params:**
```json
{
  "lab_id":           "alpine-abc123",
  "base_os":          "alpine",
  "target_arch":      "x86_64",
  "cmd":              "echo hello",
  "env":              {"FOO": "bar"},
  "working_directory": "/workspace"
}
```

> **Important:** Do not write anything to stdout before the JSON response.
> Any command output must go to **stderr** (visible in terminal) or be captured and discarded.
> Stdout is exclusively for the JSON-RPC response.

### `teardown`
Called after all steps complete (even if a step failed). Clean up resources.

**Params:**
```json
{"lab_id": "alpine-abc123"}
```

---

## Installing a Plugin

```bash
zenith plugin install ./path/to/my-plugin-dir
```

Zenith will:
1. Validate `plugin.toml`
2. Copy the directory to `~/.zenith/plugins/<name>/`
3. Check the entrypoint binary exists
4. Run a smoke test (calls `name` method)

Other commands:
```bash
zenith plugin list              # list installed plugins
zenith plugin info my-backend   # show manifest details
zenith plugin remove my-backend # uninstall
```

---

## Using a Plugin in a Workflow

```yaml
# .zenith.yml
jobs:
  my-job:
    runs_on: my-vm
    backend: my-backend        # ← plugin name from plugin.toml
    steps:
      - name: Run something
        run: echo "hello from plugin!"
```

---

## Testing a Plugin Manually

```bash
# name method
echo '{"method":"name","params":null,"id":0}' | ./zenith-my-backend

# execute method
echo '{"method":"execute","params":{"cmd":"echo hi","env":{},"lab_id":"x","base_os":"local","target_arch":"x86_64","working_directory":null},"id":2}' | ./zenith-my-backend
```

---

## Reference Implementation

See [`examples/plugin-example/`](../examples/plugin-example/) for a complete working plugin in Rust.
