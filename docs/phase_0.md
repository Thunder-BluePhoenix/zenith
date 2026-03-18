# Phase 0: CLI Core & Foundation

## Objective
Establish the foundational command-line interface (CLI) for Zenith. This phase does not implement the actual virtualization or containerization engines, but rather sets up the project structure, configuration parsing, routing, and basic execution layer.

## Milestones

1.  **Project Initialization**
    *   Set up a new Rust (Cargo) or Go project.
    *   Define the directory structure for core components (e.g., `cmd`, `config`, `runner`, `logger`).
2.  **CLI Scaffold**
    *   Implement the base CLI framework (e.g., `clap` for Rust, `cobra` for Go).
    *   Define the root command `zenith` and basic subcommands:
        *   `zenith run <cmd>`
        *   `zenith lab <action>`
        *   `zenith matrix <action>`
        *   `zenith shell`
3.  **Configuration Management**
    *   Implement a robust parser for `.zenith.yml` using `serde` (Rust) or `gopkg.in/yaml.v3` (Go).
    *   Define the strict schema for workflows, lab definitions, and matrix arrays.
    *   Load and validate the global config from `~/.zenith/config.toml` or similar.
4.  **Local Execution Runner**
    *   Build a simple wrapper around the standard OS `exec` commands.
    *   For Phase 0, `zenith run echo test` should just pass "echo test" through to the host system shell, capturing standard output and error.
5.  **Logging and Telemetry**
    *   Implement structured logging with different verbosity levels (debug, info, warn, error).
    *   Ensure all stdout/stderr from child processes stream reliably back to the Zenith terminal without mangling ANSI colors.

## Verification
*   Running `zenith --help` lists all commands.
*   Running `zenith run "echo hello"` outputs "hello" via the internal runner wrapper rather than a direct shell invocation.
*   Invalid `.zenith.yml` blocks fail gracefully with descriptive error messages detailing the exact schema violation.

## Next Steps
Once the foundational engine can parse configs and invoke local commands cleanly, we move to Phase 1 to begin creating isolated filesystems (Sandbox Lab Environments) for these commands to run inside.
