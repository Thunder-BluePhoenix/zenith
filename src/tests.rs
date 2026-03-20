/// Zenith unit tests — core module coverage.
///
/// Run with: cargo test

// ─── Config tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod config_tests {
    use crate::config::load_config;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_yml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".yml").unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_minimal_steps() {
        let f = write_yml("steps:\n  - name: hello\n    run: echo hi\n");
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let steps = cfg.steps.unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name.as_deref(), Some("hello"));
        assert_eq!(steps[0].run, "echo hi");
    }

    #[test]
    fn parses_jobs_block() {
        let f = write_yml(
            "jobs:\n  build:\n    steps:\n      - name: compile\n        run: cargo build\n      - name: test\n        run: cargo test\n"
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let jobs = cfg.jobs.unwrap();
        assert!(jobs.contains_key("build"));
        assert_eq!(jobs["build"].steps.len(), 2);
    }

    #[test]
    fn parses_matrix_strategy() {
        let f = write_yml(
            "jobs:\n  test:\n    strategy:\n      matrix:\n        os: [ubuntu, alpine]\n        version: [\"1.0\", \"2.0\"]\n    steps:\n      - name: run\n        run: echo ok\n"
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let jobs = cfg.jobs.unwrap();
        let strategy = jobs["test"].strategy.as_ref().unwrap();
        assert_eq!(strategy.matrix["os"].len(), 2);
        assert_eq!(strategy.matrix["version"].len(), 2);
    }

    #[test]
    fn parses_env_block() {
        let f = write_yml(
            "env:\n  node: \"20\"\n  python: \"3.12.3\"\njobs:\n  build:\n    steps:\n      - name: check\n        run: node --version\n"
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let env = cfg.env.unwrap();
        assert_eq!(env.node.as_deref(), Some("20"));
        assert_eq!(env.python.as_deref(), Some("3.12.3"));
    }

    #[test]
    fn step_allow_failure_defaults_false() {
        let f = write_yml("steps:\n  - name: risky\n    run: might_fail\n");
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let step = &cfg.steps.unwrap()[0];
        assert!(!step.allow_failure);
    }

    #[test]
    fn returns_error_on_nonexistent_file() {
        let result = load_config("/nonexistent/path/does/not/exist.yml");
        assert!(result.is_err(), "Loading a nonexistent file should return Err");
    }

    #[test]
    fn parses_step_cache_flag() {
        let f = write_yml(
            "steps:\n  - name: cached\n    run: echo hi\n    cache: false\n"
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        let step = &cfg.steps.unwrap()[0];
        assert_eq!(step.cache, Some(false));
    }
}

// ─── Cache tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod cache_tests {
    use crate::sandbox::cache::CacheManager;
    use crate::config::Step;
    use std::collections::HashMap;

    fn make_step(run: &str) -> Step {
        Step {
            name: Some("test-step".into()),
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
    fn cache_manager_new_succeeds() {
        let cm = CacheManager::new();
        assert!(cm.is_ok(), "CacheManager::new() should succeed");
    }

    #[test]
    fn step_hash_is_deterministic() {
        let cm1 = CacheManager::new().unwrap();
        let cm2 = CacheManager::new().unwrap();
        let step = make_step("echo hello");
        let env: HashMap<String, String> = [("FOO".into(), "bar".into())].into();
        let h1 = cm1.compute_step_hash("ubuntu", "x86_64", &step, &env);
        let h2 = cm2.compute_step_hash("ubuntu", "x86_64", &step, &env);
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_commands_produce_different_hashes() {
        let cm = CacheManager::new().unwrap();
        let env: HashMap<String, String> = HashMap::new();
        let h1 = cm.compute_step_hash("ubuntu", "x86_64", &make_step("echo a"), &env);
        let h2 = cm.compute_step_hash("ubuntu", "x86_64", &make_step("echo b"), &env);
        assert_ne!(h1, h2);
    }

    #[test]
    fn different_os_produces_different_hashes() {
        let cm = CacheManager::new().unwrap();
        let env: HashMap<String, String> = HashMap::new();
        let step = make_step("echo hello");
        let h1 = cm.compute_step_hash("ubuntu", "x86_64", &step, &env);
        let h2 = cm.compute_step_hash("alpine", "x86_64", &step, &env);
        assert_ne!(h1, h2);
    }

    #[test]
    fn different_arch_produces_different_hashes() {
        let cm = CacheManager::new().unwrap();
        let env: HashMap<String, String> = HashMap::new();
        let step = make_step("echo hello");
        let h1 = cm.compute_step_hash("ubuntu", "x86_64", &step, &env);
        let h2 = cm.compute_step_hash("ubuntu", "aarch64", &step, &env);
        assert_ne!(h1, h2);
    }

    #[test]
    fn list_entries_returns_vec() {
        let cm = CacheManager::new().unwrap();
        let _entries = cm.list_entries(); // must not panic
    }
}

// ─── History tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod history_tests {
    use crate::ui::history::{RunLogger, RunOutcome, StepStatus, list_runs, get_steps, get_run};

    #[test]
    fn logger_creates_summary() {
        let logger = RunLogger::new("test-job");
        let run_id = logger.run_id.clone();
        let run = get_run(&run_id);
        assert!(run.is_some(), "summary.json should exist after RunLogger::new");
        let run = run.unwrap();
        assert_eq!(run.job, "test-job");
        assert_eq!(run.status, RunOutcome::Running);
        assert_eq!(run.step_count, 0);
    }

    #[test]
    fn logger_records_steps() {
        let mut logger = RunLogger::new("step-job");
        let run_id = logger.run_id.clone();

        logger.log_step_start(0, "compile");
        logger.log_step_done(0, "compile", true, vec!["built".into()]);
        logger.log_step_cached(1, "lint");
        logger.finalize(true);

        let steps = get_steps(&run_id);
        assert_eq!(steps.len(), 3, "start + done + cached = 3 events");

        assert_eq!(steps[0].name, "compile");
        assert_eq!(steps[0].status, StepStatus::Started);

        assert_eq!(steps[1].name, "compile");
        assert_eq!(steps[1].status, StepStatus::Done);
        assert_eq!(steps[1].log_lines, vec!["built"]);

        assert_eq!(steps[2].name, "lint");
        assert_eq!(steps[2].status, StepStatus::Cached);
    }

    #[test]
    fn finalize_failed_outcome() {
        let logger = RunLogger::new("outcome-job");
        let run_id = logger.run_id.clone();
        logger.finalize(false);
        let run = get_run(&run_id).unwrap();
        assert_eq!(run.status, RunOutcome::Failed);
    }

    #[test]
    fn finalize_success_outcome() {
        let logger = RunLogger::new("success-job");
        let run_id = logger.run_id.clone();
        logger.finalize(true);
        let run = get_run(&run_id).unwrap();
        assert_eq!(run.status, RunOutcome::Success);
    }

    #[test]
    fn list_runs_includes_new_run() {
        let logger = RunLogger::new("list-test-job");
        let run_id = logger.run_id.clone();
        logger.finalize(true);
        let runs = list_runs(200);
        let found = runs.iter().any(|r| r.run_id == run_id);
        assert!(found, "New run should appear in list_runs()");
    }

    #[test]
    fn step_count_tracks_correctly() {
        let mut logger = RunLogger::new("count-job");
        let run_id = logger.run_id.clone();
        logger.log_step_done(0, "a", true,  vec![]);
        logger.log_step_done(1, "b", false, vec![]);
        logger.log_step_cached(2, "c");
        logger.finalize(false);
        let run = get_run(&run_id).unwrap();
        assert_eq!(run.step_count, 3);
        assert_eq!(run.steps_ok, 2); // a(ok) + c(cached=ok) = 2; b failed
    }

    #[test]
    fn get_run_missing_returns_none() {
        let result = get_run("000000000000000000000000000000000000nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn get_steps_missing_returns_empty() {
        let steps = get_steps("000000000000000000000000000000000000nonexistent");
        assert!(steps.is_empty());
    }
}

// ─── Runner unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod runner_tests {
    use crate::config::{ZenithConfig, Job, Step};
    use crate::runner::execute_local;
    use std::collections::HashMap;

    fn make_step(run: &str, allow_failure: bool) -> Step {
        Step {
            name: Some("test".into()),
            run: run.into(),
            env: None,
            working_directory: None,
            allow_failure,
            cache: Some(false),
            cache_key: None,
            watch: vec![],
            outputs: vec![],
            depends_on: vec![],
        }
    }

    fn single_job_config(steps: Vec<Step>) -> ZenithConfig {
        let job = Job {
            runs_on: Some("local".into()),
            steps,
            env: None,
            working_directory: None,
            strategy: None,
            backend: None,
            arch: None,
            cache: Some(false),
            toolchain: None,
        };
        let mut jobs = HashMap::new();
        jobs.insert("ci".into(), job);
        ZenithConfig { version: "1".into(), jobs: Some(jobs), steps: None, env: None, cache: None }
    }

    #[tokio::test]
    async fn successful_command_passes() {
        let cfg = single_job_config(vec![make_step("echo hello", false)]);
        let result = execute_local(cfg, None, true).await;
        assert!(result.is_ok(), "echo hello should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn failing_command_returns_error() {
        let cfg = single_job_config(vec![make_step("exit 1", false)]);
        let result = execute_local(cfg, None, true).await;
        assert!(result.is_err(), "exit 1 should cause failure");
    }

    #[tokio::test]
    async fn allow_failure_does_not_halt_pipeline() {
        let steps = vec![
            make_step("exit 1", true),       // fails but allowed
            make_step("echo still ok", false), // should still run
        ];
        let cfg = single_job_config(steps);
        let result = execute_local(cfg, None, true).await;
        assert!(result.is_ok(), "allow_failure should not halt pipeline: {:?}", result);
    }

    #[tokio::test]
    async fn specific_job_can_be_selected() {
        let step_ok   = make_step("echo job-a", false);
        let step_fail = make_step("exit 1", false);

        let job_a = Job {
            runs_on: Some("local".into()), steps: vec![step_ok],
            env: None, working_directory: None, strategy: None,
            backend: None, arch: None, cache: Some(false), toolchain: None,
        };
        let job_b = Job {
            runs_on: Some("local".into()), steps: vec![step_fail],
            env: None, working_directory: None, strategy: None,
            backend: None, arch: None, cache: Some(false), toolchain: None,
        };

        let mut jobs = HashMap::new();
        jobs.insert("a".into(), job_a);
        jobs.insert("b".into(), job_b);
        let cfg = ZenithConfig { version: "1".into(), jobs: Some(jobs), steps: None, env: None, cache: None };

        // Select only job "a" — job "b" (exit 1) must NOT run
        let result = execute_local(cfg, Some("a".into()), true).await;
        assert!(result.is_ok(), "Selecting job 'a' should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn unknown_job_name_returns_error() {
        let cfg = single_job_config(vec![make_step("echo ok", false)]);
        let result = execute_local(cfg, Some("nonexistent".into()), true).await;
        assert!(result.is_err(), "Unknown job name should return error");
    }

    #[tokio::test]
    async fn env_var_injected_into_step() {
        // Step reads an env var that is set at job level
        let step = Step {
            name: Some("env-check".into()),
            run: if cfg!(target_os = "windows") {
                "if not defined ZENITH_TEST_VAR exit 1".into()
            } else {
                "test -n \"$ZENITH_TEST_VAR\"".into()
            },
            env: Some([("ZENITH_TEST_VAR".into(), "hello".into())].into()),
            working_directory: None,
            allow_failure: false,
            cache: Some(false),
            cache_key: None,
            watch: vec![],
            outputs: vec![],
            depends_on: vec![],
        };
        let cfg = single_job_config(vec![step]);
        let result = execute_local(cfg, None, true).await;
        assert!(result.is_ok(), "Env var should be available in step: {:?}", result);
    }
}
