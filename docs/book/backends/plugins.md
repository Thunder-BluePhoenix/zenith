# Plugin Backends

Any installed Zenith plugin with `type = "backend"` can be used as a first-class backend. This lets you extend Zenith with custom execution environments without modifying Zenith itself.

---

## Using a plugin as a backend

```yaml
jobs:
  deploy:
    backend: my-plugin-name    # matches the `name` field in plugin.toml
    steps:
      - name: Deploy
        run: deploy --env staging
```

Zenith routes the step execution through the plugin's `execute` JSON-RPC method.

---

## Installing a plugin backend

```bash
# From a local directory
zenith plugin install ./my-backend-plugin

# From the hosted registry
zenith plugin search kubernetes
zenith plugin install k8s-deploy
```

---

## Plugin manifest

Every plugin backend must have a `plugin.toml`:

```toml
[plugin]
name            = "k8s-deploy"
version         = "1.2.0"
type            = "backend"
entrypoint      = "k8s-deploy-bin"       # binary in the same directory
description     = "Kubernetes deployment backend"
requires_zenith = ">=0.1.0"              # version constraint — enforced on install
```

The `requires_zenith` field is checked at install time. If the running Zenith version does not satisfy the constraint, installation is refused with a clear error message.

---

## JSON-RPC protocol

Plugin backends communicate with Zenith over stdin/stdout using a line-delimited JSON-RPC protocol.

### Requests Zenith sends

| Method | Description |
|---|---|
| `name` | Returns the plugin's name (used as smoke test on install) |
| `execute` | Run a step command in the plugin's environment |
| `provision` | Set up the sandbox environment before steps run |
| `teardown` | Clean up the sandbox after all steps complete |

### `execute` parameters

```json
{
  "method": "execute",
  "params": {
    "lab_id": "abc123",
    "base_os": "alpine",
    "cmd": "cargo build --release",
    "env": { "MY_VAR": "value" },
    "working_directory": null
  }
}
```

### `execute` response

```json
{
  "result": { "exit_code": 0, "stdout": "...", "stderr": "..." }
}
```

---

## Writing a plugin backend

See [Writing a Plugin](../plugins/writing.md) for the complete authoring guide, including a reference implementation in Rust and the full protocol specification.

---

## Managing plugin backends

```bash
zenith plugin list                   # list all installed plugins
zenith plugin info <name>            # show manifest details
zenith plugin remove <name>          # uninstall
```
