/// Phase 9: Remote Runner
///
/// Run local `.zenith.yml` workflows on remote machines over SSH.
///
/// Transport: system `ssh` binary (OpenSSH) — zero extra dependencies,
/// works on Linux, macOS, and Windows 10+ (which ships with OpenSSH).
///
/// Protocol:
///   1. Project is packaged as tar.gz (excluding target/, .git/, etc.)
///   2. Uploaded via `ssh host 'tar xz -C ~/.zenith/workspace/<id>/'`
///   3. zenith-agent is bootstrapped on the remote if not installed
///   4. Workflow task JSON is piped to `zenith-agent --agent-mode` over SSH
///   5. Agent output is streamed back, prefixed with `[remote:<name>]`
///
/// Remotes are persisted in ~/.zenith/remotes.toml.

pub mod config;
pub mod runner;
pub mod transport;
