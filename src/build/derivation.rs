/// Derivation model — Phase 13.
///
/// A derivation is a pure function: given the same inputs (files + env + command),
/// it always produces the same outputs. The derivation's SHA-256 hash is its
/// permanent, globally unique identity — two builds with identical derivation
/// hashes are guaranteed to produce identical outputs and can share cached results.
///
/// This mirrors Nix derivations but is embedded natively in Zenith.
///
/// Usage:
///   let d  = Derivation::from_step(&step, &env, "alpine", "x86_64");
///   let id = d.id();          // hex SHA-256 — the build's identity
///   let json = d.to_json();   // deterministic JSON for human inspection

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// ─── Types ────────────────────────────────────────────────────────────────────

/// A single input to a derivation — a file path and its content hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileInput {
    pub path:        String,
    pub sha256:      String,
}

/// A derivation: the complete, deterministic description of a build step.
///
/// The derivation is always serialised with sorted keys so the JSON is
/// byte-for-byte identical regardless of insertion order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Derivation {
    /// Human-readable name (step name).
    pub name:    String,
    /// Shell command to execute.
    pub command: String,
    /// OS the step runs on (e.g. "alpine", "ubuntu").
    pub os:      String,
    /// CPU architecture (e.g. "x86_64", "aarch64").
    pub arch:    String,
    /// Environment variables — sorted for determinism.
    pub env:     BTreeMap<String, String>,
    /// Watched input files and their content hashes — sorted by path.
    pub inputs:  Vec<FileInput>,
    /// Expected output paths (declared, not verified until after execution).
    pub outputs: Vec<String>,
    /// Derivation hashes of steps this step depends on.
    pub deps:    Vec<String>,
}

impl Derivation {
    /// Build a derivation from a workflow step.
    pub fn from_step(
        step:    &crate::config::Step,
        env:     &std::collections::HashMap<String, String>,
        os:      &str,
        arch:    &str,
    ) -> Self {
        // Sort env for determinism
        let sorted_env: BTreeMap<String, String> = env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Hash watched input files
        let mut inputs: Vec<FileInput> = step.watch.iter()
            .flat_map(|pattern| {
                glob::glob(pattern)
                    .unwrap_or_else(|_| glob::glob("").unwrap())
                    .flatten()
                    .filter(|p| p.is_file())
                    .map(|p| {
                        let content = std::fs::read(&p).unwrap_or_default();
                        let hash    = hex::encode(Sha256::digest(&content));
                        FileInput { path: p.to_string_lossy().into_owned(), sha256: hash }
                    })
            })
            .collect();
        inputs.sort();

        let outputs = step.outputs.clone();
        let mut outputs_sorted = outputs;
        outputs_sorted.sort();

        Self {
            name:    step.name.clone().unwrap_or_else(|| step.run.clone()),
            command: step.run.clone(),
            os:      os.to_string(),
            arch:    arch.to_string(),
            env:     sorted_env,
            inputs,
            outputs: outputs_sorted,
            deps:    vec![],
        }
    }

    /// Set upstream derivation hashes that this derivation depends on.
    pub fn with_deps(mut self, deps: Vec<String>) -> Self {
        let mut deps = deps;
        deps.sort();
        self.deps = deps;
        self
    }

    /// The derivation's identity — SHA-256 of its deterministic JSON.
    ///
    /// Two derivations with the same `id()` are guaranteed to have identical
    /// inputs and will therefore produce identical outputs.
    pub fn id(&self) -> String {
        let json = self.to_json();
        hex::encode(Sha256::digest(json.as_bytes()))
    }

    /// Deterministic JSON representation.
    ///
    /// Uses `serde_json` with sorted keys (BTreeMap for env ensures field order).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Pretty-printed JSON for human inspection.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Step;
    use std::collections::HashMap;

    fn make_step(run: &str) -> Step {
        Step {
            name: Some("test".into()),
            run: run.into(),
            env: None,
            working_directory: None,
            allow_failure: false,
            cache: None,
            cache_key: None,
            watch: vec![],
            outputs: vec![],
            depends_on: vec![],
        }
    }

    #[test]
    fn id_is_deterministic() {
        let step = make_step("cargo build");
        let env: HashMap<String, String> = [("RUST_LOG".into(), "info".into())].into();
        let d1 = Derivation::from_step(&step, &env, "alpine", "x86_64");
        let d2 = Derivation::from_step(&step, &env, "alpine", "x86_64");
        assert_eq!(d1.id(), d2.id());
    }

    #[test]
    fn different_command_different_id() {
        let env = HashMap::new();
        let d1 = Derivation::from_step(&make_step("cargo build"), &env, "ubuntu", "x86_64");
        let d2 = Derivation::from_step(&make_step("cargo test"),  &env, "ubuntu", "x86_64");
        assert_ne!(d1.id(), d2.id());
    }

    #[test]
    fn different_os_different_id() {
        let env = HashMap::new();
        let step = make_step("make");
        let d1 = Derivation::from_step(&step, &env, "alpine", "x86_64");
        let d2 = Derivation::from_step(&step, &env, "ubuntu", "x86_64");
        assert_ne!(d1.id(), d2.id());
    }

    #[test]
    fn different_arch_different_id() {
        let env = HashMap::new();
        let step = make_step("make");
        let d1 = Derivation::from_step(&step, &env, "alpine", "x86_64");
        let d2 = Derivation::from_step(&step, &env, "alpine", "aarch64");
        assert_ne!(d1.id(), d2.id());
    }

    #[test]
    fn different_env_different_id() {
        let env1: HashMap<String, String> = [("FOO".into(), "a".into())].into();
        let env2: HashMap<String, String> = [("FOO".into(), "b".into())].into();
        let step = make_step("make");
        let d1 = Derivation::from_step(&step, &env1, "alpine", "x86_64");
        let d2 = Derivation::from_step(&step, &env2, "alpine", "x86_64");
        assert_ne!(d1.id(), d2.id());
    }

    #[test]
    fn deps_change_id() {
        let env = HashMap::new();
        let d1 = Derivation::from_step(&make_step("make"), &env, "alpine", "x86_64");
        let d2 = d1.clone().with_deps(vec!["abc123".into()]);
        assert_ne!(d1.id(), d2.id());
    }

    #[test]
    fn to_json_is_valid_json() {
        let env = HashMap::new();
        let d = Derivation::from_step(&make_step("echo hi"), &env, "alpine", "x86_64");
        let json = d.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert_eq!(parsed["command"], "echo hi");
    }

    #[test]
    fn id_is_64_hex_chars() {
        let env = HashMap::new();
        let d = Derivation::from_step(&make_step("ls"), &env, "ubuntu", "x86_64");
        assert_eq!(d.id().len(), 64);
        assert!(d.id().chars().all(|c| c.is_ascii_hexdigit()));
    }
}
