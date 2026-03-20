# Phase 7: Environment & Package System (Reproducible Environments)

## Objective

Let developers declare the exact versions of tools their project needs — `node: 20`, `python: 3.12`, `rust: 1.78` — and have Zenith download, cache, and inject those binaries into the shell `$PATH` automatically. No Docker. No system installs. No `nvm`, `pyenv`, or `rbenv` per-tool wrappers.

This is what Nix flakes and Devbox do. The difference is Zenith's env system is integrated with the same `.zenith.yml` that drives sandboxes and workflows.

## Current State in the Codebase

Nothing from Phase 7 exists yet. The `Job` struct in `src/config.rs` has an `env` field for environment *variables*, but there is no concept of declarative toolchain *versions*. The runner injects env vars into subprocesses but does not manage tool installation.

---

## Milestones & Tasks

### Milestone 1 — YAML Schema: Declare Toolchain Versions

**Why:** The user experience starts with the config file. Define the schema first so everything else has a clear contract to implement against.

**Tasks:**

1. **Add a top-level `env` block to `ZenithConfig` in `src/config.rs`**
   ```yaml
   env:
     node: "20"
     python: "3.12.3"
     rust: "1.78.0"
     go: "1.22"
   ```
   Add an `EnvConfig` struct and a `env: Option<EnvConfig>` field to `ZenithConfig`:
   ```rust
   #[derive(Debug, Serialize, Deserialize, Clone)]
   pub struct EnvConfig {
       pub node: Option<String>,
       pub python: Option<String>,
       pub rust: Option<String>,
       pub go: Option<String>,
       // Extend for more runtimes as needed
   }
   ```

2. **Allow per-job env overrides**
   - Jobs can declare a local `env` toolchain block that overrides the top-level one
   - This allows the matrix to run `node: "18"` in one job and `node: "20"` in another

---

### Milestone 2 — Toolchain Manager Module

**Why:** We need a dedicated module to handle downloading, caching, and PATH-injecting toolchain binaries. This is the core of Phase 7.

**Tasks:**

1. **Create `src/toolchain/mod.rs`** as a new module
   - Declare it in `src/main.rs` with `mod toolchain;`
   - This module owns all toolchain download/cache logic

2. **Define the `Toolchain` trait in `src/toolchain/mod.rs`**
   ```rust
   pub trait Toolchain {
       fn name(&self) -> &str;
       fn version(&self) -> &str;
       // Returns the bin directory to prepend to PATH
       fn ensure_installed(&self) -> Result<PathBuf>;
   }
   ```
   Implement concrete structs: `NodeToolchain`, `PythonToolchain`, `RustToolchain`, `GoToolchain`

3. **Define download sources per toolchain**
   - Node.js: `https://nodejs.org/dist/v{version}/node-v{version}-linux-x64.tar.gz`
   - Python: Use `python-build-standalone` project releases from GitHub
   - Go: `https://go.dev/dl/go{version}.linux-amd64.tar.gz`
   - Rust: Use `rustup` offline installer or pre-built `rust-{version}-x86_64-unknown-linux-gnu.tar.gz`

4. **Implement `ensure_installed` for each toolchain**
   - Check if `~/.zenith/toolchains/<name>/<version>/bin/` already exists (cache hit)
   - If not: download the archive, extract into `~/.zenith/toolchains/<name>/<version>/`
   - Return the `bin/` directory path
   - Reuse the existing `download_file` and `extract_tarball` helpers from `src/sandbox/mod.rs` (move them to a shared `src/utils.rs`)

---

### Milestone 3 — PATH Injection into Workflow Steps

**Why:** Once binaries are on disk, they need to be prepended to the `PATH` inside every step that runs under this job — so `node --version` inside the workflow resolves to the Zenith-managed version, not the system one.

**Tasks:**

1. **Create `resolve_toolchain_env` function in `src/toolchain/mod.rs`**
   - Input: `&EnvConfig`
   - Output: `HashMap<String, String>` containing the augmented `PATH`
   - For each declared toolchain, call `ensure_installed()`, collect bin dirs, prepend to `PATH`

2. **Call `resolve_toolchain_env` in `src/runner.rs` before step execution**
   - In `execute_single_job`, after loading the config but before the step loop:
     ```rust
     let mut tool_env = HashMap::new();
     if let Some(ref env_cfg) = config_env {
         tool_env = toolchain::resolve_toolchain_env(env_cfg)?;
     }
     ```
   - Merge `tool_env` into `merged_env` so steps automatically get the right `PATH`

3. **Thread `config_env` through the runner**
   - `execute_local` receives the `ZenithConfig`; extract `config.env` and pass it into `execute_single_job`

---

### Milestone 4 — `zenith env` CLI Commands

**Why:** Users need first-class CLI commands to set up their env without running a full workflow.

**Tasks:**

1. **Add `Env` subcommand to `src/cli.rs`**
   ```
   zenith env init          # parse .zenith.yml env block, download all toolchains
   zenith env shell         # drop into a shell with the toolchain PATH injected
   zenith env list          # show all installed toolchains and their versions
   zenith env clean         # remove all cached toolchains
   ```

2. **Implement `zenith env shell` in `src/main.rs`**
   - Load config, call `resolve_toolchain_env`
   - Spawn `$SHELL` (or `sh`) as a subprocess with the augmented `PATH` env
   - The user is now inside a shell where `node`, `python`, etc. resolve to Zenith versions

3. **Implement `zenith env list`**
   - Walk `~/.zenith/toolchains/` directory
   - Print each installed toolchain, its version, and its install path

---

### Milestone 5 — Integration with Sandbox Labs

**Why:** When a workflow runs inside a sandbox lab (container or firecracker), the toolchain binaries are on the host. They need to be visible inside the sandbox too.

**Tasks:**

1. **Bind-mount or copy toolchain bins into the sandbox workspace**
   - In `ContainerBackend::provision`, after setting up the workspace, also copy or bind-mount the resolved toolchain dirs into the rootfs at `/usr/local/zenith-tools/`
   - On Linux: use bind-mount via `nix::mount::mount` (the `nix` crate is already a dependency)
   - On Windows/macOS: copy the directories (same approach as `push_project`)

2. **Set `PATH` inside the sandbox to include `/usr/local/zenith-tools/<name>/bin`**
   - Inject this as part of the `env` passed to `backend.execute()`

---

## Key Files Reference

| File | Role |
|---|---|
| `src/config.rs` | Add `EnvConfig` struct, `env` field to `ZenithConfig` and `Job` |
| `src/toolchain/mod.rs` | New module: `Toolchain` trait + all runtime implementations |
| `src/toolchain/node.rs` | Node.js download/install logic |
| `src/toolchain/python.rs` | Python download/install logic |
| `src/toolchain/go.rs` | Go download/install logic |
| `src/toolchain/rust.rs` | Rust toolchain download/install logic |
| `src/runner.rs` | Call `resolve_toolchain_env`, merge into step env |
| `src/cli.rs` | Add `zenith env` command tree |
| `src/main.rs` | Route `zenith env` to handler, declare `mod toolchain` |
| `src/utils.rs` | Move shared `download_file`, `extract_tarball` here |

---

## Verification Checklist

- [ ] `.zenith.yml` with `env: { node: "20" }` causes `node --version` inside a step to print `v20.x.x`
- [ ] The binary is cached in `~/.zenith/toolchains/node/20/` and not re-downloaded on second run
- [ ] `zenith env shell` opens an interactive shell where `which node` points to `~/.zenith/toolchains/`
- [ ] Two matrix jobs declaring different Node versions each get the correct version in their `PATH`
- [ ] `zenith env clean` removes all toolchain binaries from the cache directory
- [ ] Inside a sandbox lab, `node --version` still resolves to the Zenith-managed version

## Next Steps

With reproducible, isolated toolchains established, Phase 8 opens Zenith to the community through the **Plugin System** — letting anyone write and distribute custom backends, syntax extensions, and execution hooks.
