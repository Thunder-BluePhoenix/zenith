/// Phase 8: Plugin System
///
/// Architecture decision: External process + JSON-RPC over stdio (Option A).
///
/// Why not shared libraries (.so / .dll)?
///   Rust has no stable ABI. Loading a Rust .so compiled by a different
///   rustc version causes UB. Dynamic linking is off the table.
///
/// Why not WASM plugins?
///   Plugin authors would need to compile to wasm32-wasi, and some syscalls
///   (process spawning, network) are limited. Phase 8 uses WASM as an
///   execution backend (src/sandbox/wasm.rs) — not as a plugin transport.
///
/// Why external process + JSON-RPC?
///   - Language-agnostic: plugins can be written in Go, Python, Node, etc.
///   - No ABI: communication is newline-delimited JSON over stdin/stdout.
///   - Debuggable: `echo '{"method":"name","params":null,"id":0}' | ./plugin`
///   - Aligns with Phase 9 (remote runner also uses process communication).
///
/// Plugin directory: ~/.zenith/plugins/<name>/
///   Required files:
///     plugin.toml   — manifest (name, version, type, entrypoint, description)
///     <entrypoint>  — executable binary (or .exe on Windows)
///
/// Plugin protocol (JSON-RPC, one object per line):
///   Request:  { "method": "provision", "params": {...}, "id": 1 }
///   Response: { "result": null, "error": null, "id": 1 }
///
/// Required methods: name | provision | execute | teardown

pub mod manifest;
pub mod registry;
pub mod protocol;
pub mod client;
