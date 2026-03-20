# Phase 8: Plugin System

## Objective

Transform Zenith from a monolithic binary into an **extensible platform**. Third parties should be able to write custom backends (e.g., a `bhyve` backend for FreeBSD VMs), custom syntax parsers, custom log formatters, or new toolchain providers — and distribute them as installable plugins without forking Zenith itself.

The plugin system is what transforms a tool into an ecosystem.

## Current State in the Codebase

The backend system (`src/sandbox/backend.rs`) already uses a Rust trait (`Backend`) that is essentially a plugin interface — but it is hardcoded at compile time. `get_backend()` in `src/sandbox/mod.rs` is a `match` block; there is no runtime loading.

The plugin system will make this dynamic: plugins are external binaries or WASM modules that Zenith discovers, loads, and calls at runtime.

---

## Milestones & Tasks

### Milestone 1 — Plugin Architecture Decision

**Why:** There are two major approaches to plugins in Rust, and the choice affects everything that follows. Pick one before writing code.

**Option A — External Process (subprocess / gRPC)**
- A plugin is a standalone binary (e.g., `zenith-backend-bhyve`)
- Zenith spawns it as a subprocess and communicates over stdin/stdout (JSON-RPC or gRPC)
- Pro: Language-agnostic, no ABI issues
- Con: Higher overhead per call, process management complexity

**Option B — WASM Plugins (wasmtime)**
- A plugin is a `.wasm` module compiled to WASI
- Zenith embeds `wasmtime` and loads `.wasm` files at runtime
- Pro: Sandboxed by design, portable across platforms
- Con: Plugin authors must compile to `wasm32-wasi`, some syscall limitations

**Recommended choice for Phase 8:** Start with **Option A (subprocess + JSON-RPC)** because:
- Plugin authors can write in any language
- No wasmtime version pinning headaches
- Aligns with Phase 9-10 (remote runner also uses process communication)

Document this decision in the codebase at the top of `src/plugin/mod.rs`.

---

### Milestone 2 — Plugin Discovery & Registry

**Why:** Before Zenith can call a plugin, it needs to find it. Plugins must be installable, discoverable, and listed.

**Tasks:**

1. **Define the plugin install directory:** `~/.zenith/plugins/<plugin-name>/`
   - Each plugin is a directory containing at minimum:
     - `plugin.toml` — manifest (name, version, type, entry point)
     - An executable binary or `.wasm` file

2. **Define `plugin.toml` schema** — create `src/plugin/manifest.rs`:
   ```toml
   [plugin]
   name = "bhyve-backend"
   version = "0.1.0"
   type = "backend"        # backend | toolchain | syntax | logger
   entrypoint = "zenith-backend-bhyve"   # binary name inside the plugin dir
   description = "FreeBSD bhyve VM backend for Zenith"
   ```
   Deserialize with `serde` into a `PluginManifest` struct.

3. **Create `src/plugin/registry.rs`**
   - `fn discover_plugins() -> Vec<PluginManifest>`: walks `~/.zenith/plugins/`, reads each `plugin.toml`
   - `fn find_plugin(name: &str) -> Option<PluginManifest>`: finds a specific plugin by name

4. **Add `zenith plugin` CLI subcommand in `src/cli.rs`**:
   ```
   zenith plugin list                        # list installed plugins
   zenith plugin install <name>              # download and install a plugin
   zenith plugin remove <name>               # uninstall a plugin
   zenith plugin info <name>                 # show plugin manifest details
   ```

---

### Milestone 3 — Plugin Communication Protocol (JSON-RPC over stdio)

**Why:** Zenith and plugins must speak a common language. JSON-RPC over stdio is simple, debuggable, and language-agnostic.

**Tasks:**

1. **Define the JSON-RPC message format** in `src/plugin/protocol.rs`:
   ```rust
   #[derive(Serialize, Deserialize)]
   pub struct RpcRequest {
       pub method: String,   // "provision" | "execute" | "teardown" | "name"
       pub params: serde_json::Value,
       pub id: u64,
   }

   #[derive(Serialize, Deserialize)]
   pub struct RpcResponse {
       pub result: Option<serde_json::Value>,
       pub error: Option<String>,
       pub id: u64,
   }
   ```

2. **Create `src/plugin/client.rs` — the `PluginBackend` struct**
   - Implements the `Backend` trait from `src/sandbox/backend.rs`
   - On each method call (`provision`, `execute`, `teardown`), it:
     1. Spawns the plugin binary as a child process (if not already running)
     2. Serializes the call as a `RpcRequest` JSON line to the child's stdin
     3. Reads one `RpcResponse` JSON line from child's stdout
     4. Returns `Ok(())` or `Err(...)` based on the response
   - Use `tokio::process::Command` with `Stdio::piped()` for async I/O

3. **Add `PluginBackend` to the backend factory in `src/sandbox/mod.rs`**:
   ```rust
   pub fn get_backend(name: &str) -> Box<dyn Backend> {
       match name {
           "firecracker" => Box::new(FirecrackerBackend),
           "wasm" => Box::new(WasmBackend),
           _ => {
               // Try to load as a plugin
               if let Some(manifest) = plugin::registry::find_plugin(name) {
                   return Box::new(plugin::client::PluginBackend::new(manifest));
               }
               Box::new(ContainerBackend)
           }
       }
   }
   ```

---

### Milestone 4 — Plugin Installation

**Why:** `zenith plugin install bhyve-backend` should work end to end. Start with installing from a local path; remote registry can come later.

**Tasks:**

1. **Implement `zenith plugin install <path-or-url>`**
   - Phase 8a: Install from local path — copy the directory into `~/.zenith/plugins/<name>/`
   - Phase 8b: Install from GitHub releases — `zenith plugin install github:owner/repo@v1.0.0`
     - Fetch the release asset tarball from GitHub API
     - Extract into `~/.zenith/plugins/<name>/`
     - Reuse the download/extract utilities from `src/utils.rs`

2. **Validate the plugin after install**
   - Parse the `plugin.toml` manifest — fail if missing or malformed
   - Check the entrypoint binary exists and is executable
   - Run a smoke test: spawn the binary, send `{ "method": "name", "params": {}, "id": 1 }` and expect a valid response

---

### Milestone 5 — Plugin SDK Reference (for Plugin Authors)

**Why:** The plugin system is only useful if people can write plugins. Provide a minimal reference implementation.

**Tasks:**

1. **Create `examples/plugin-example/` in the repo**
   - A simple Rust binary that:
     - Reads JSON-RPC requests from stdin line by line
     - Responds to `name`, `provision`, `execute`, `teardown`
     - For `execute`, just runs the command via `std::process::Command`

2. **Create `docs/plugin_authoring.md`**
   - Protocol spec (JSON-RPC over stdio)
   - Required methods and their parameter shapes
   - How to write a `plugin.toml`
   - How to test: `echo '{"method":"name","params":{},"id":1}' | ./my-plugin`

---

## Key Files Reference

| File | Role |
|---|---|
| `src/plugin/mod.rs` | Module root, architecture decision comment, public API |
| `src/plugin/manifest.rs` | `PluginManifest` struct, `plugin.toml` parsing |
| `src/plugin/registry.rs` | Plugin discovery, `find_plugin`, `discover_plugins` |
| `src/plugin/protocol.rs` | `RpcRequest`, `RpcResponse` types |
| `src/plugin/client.rs` | `PluginBackend` — implements `Backend` via subprocess RPC |
| `src/sandbox/mod.rs` | Extend `get_backend()` to fall through to plugin registry |
| `src/cli.rs` | Add `zenith plugin` command tree |
| `src/main.rs` | Route plugin commands, declare `mod plugin` |
| `examples/plugin-example/` | Reference plugin implementation |
| `docs/plugin_authoring.md` | Plugin author guide |

---

## Verification Checklist

- [ ] `zenith plugin list` shows installed plugins
- [ ] `zenith plugin install ./my-plugin` installs the plugin and validates the manifest
- [ ] A `.zenith.yml` with `backend: my-plugin` routes execution through the plugin binary
- [ ] The plugin's `execute` method receives the correct `cmd` and `env` parameters
- [ ] If the plugin binary crashes, Zenith surfaces a readable error (not a panic)
- [ ] `zenith plugin remove my-plugin` deletes the plugin directory
- [ ] The example plugin in `examples/plugin-example/` works end-to-end

## Next Steps

With the plugin system in place, Phase 9-10 builds the **Remote & Cloud Runner** — letting workflows break out of the local machine entirely.
