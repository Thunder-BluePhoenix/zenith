# Writing a Plugin

Plugins are external processes that communicate with Zenith over stdin/stdout using a line-delimited JSON-RPC protocol. They can be written in any language that can read from stdin and write to stdout.

---

## Plugin manifest

Every plugin must have a `plugin.toml` in its root directory:

```toml
[plugin]
name            = "my-backend"
version         = "1.0.0"
type            = "backend"
entrypoint      = "my-backend-bin"       # binary in the same directory
description     = "My custom backend"
requires_zenith = ">=0.1.0"
```

| Field | Required | Description |
|---|---|---|
| `name` | yes | Unique plugin name — used as `backend:` value in `.zenith.yml` |
| `version` | yes | Semver string |
| `type` | yes | Currently `"backend"` |
| `entrypoint` | yes | Filename of the executable in the plugin directory |
| `description` | no | Human-readable description (shown in `zenith plugin info`) |
| `requires_zenith` | no | Version constraint — enforced at install time |

---

## JSON-RPC protocol

Zenith communicates with plugins over stdin/stdout using newline-delimited JSON. Each message is a single JSON object on one line.

### Request format

```json
{"method": "execute", "id": 1, "params": { ... }}
```

### Response format (success)

```json
{"id": 1, "result": { ... }}
```

### Response format (error)

```json
{"id": 1, "error": {"code": -1, "message": "Something went wrong"}}
```

---

## Required methods

### `name`

Returns the plugin's name. Called on install as a smoke test.

**Request:** `{"method": "name", "id": 1, "params": {}}`

**Response:** `{"id": 1, "result": {"name": "my-backend"}}`

### `execute`

Runs a step command in the plugin's environment.

**Request:**
```json
{
  "method": "execute",
  "id": 2,
  "params": {
    "lab_id": "abc123",
    "base_os": "alpine",
    "cmd": "cargo build --release",
    "env": {"CARGO_TERM_COLOR": "always"},
    "working_directory": null
  }
}
```

**Response:**
```json
{"id": 2, "result": {"exit_code": 0}}
```

### `provision`

Set up the sandbox environment. Called once before any steps run.

**Request:** `{"method": "provision", "id": 3, "params": {"lab_id": "abc123", "base_os": "alpine"}}`

**Response:** `{"id": 3, "result": {}}`

### `teardown`

Clean up after all steps complete (or on failure). Called even if steps fail.

**Request:** `{"method": "teardown", "id": 4, "params": {"lab_id": "abc123"}}`

**Response:** `{"id": 4, "result": {}}`

---

## Reference implementation (Rust)

See `examples/plugin-example/` in the Zenith repository for a complete, working backend plugin in Rust.

```rust
use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let req: Value = serde_json::from_str(&line).unwrap();
        let id = req["id"].clone();
        let method = req["method"].as_str().unwrap_or("");

        let result = match method {
            "name"      => json!({"name": "my-backend"}),
            "provision" => json!({}),
            "execute"   => {
                let cmd = req["params"]["cmd"].as_str().unwrap_or("");
                // Run cmd however your backend handles it
                eprintln!("[my-backend] executing: {}", cmd);
                json!({"exit_code": 0})
            }
            "teardown"  => json!({}),
            _           => {
                let resp = json!({"id": id, "error": {"code": -32601, "message": "Method not found"}});
                writeln!(out, "{}", resp).unwrap();
                continue;
            }
        };

        let resp = json!({"id": id, "result": result});
        writeln!(out, "{}", resp).unwrap();
        out.flush().unwrap();
    }
}
```

---

## Testing your plugin locally

```bash
# Install from your development directory
zenith plugin install ./my-backend

# Verify it installed
zenith plugin list
zenith plugin info my-backend

# Use it in a workflow
cat > .zenith.yml << 'EOF'
version: "2"
jobs:
  test:
    backend: my-backend
    steps:
      - name: Hello
        run: echo "hello from my-backend"
EOF

zenith run
```
