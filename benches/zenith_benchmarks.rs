/// Zenith Criterion benchmark suite (Phase 14)
///
/// Run with:   cargo bench
/// Save HTML:  cargo bench -- --output-format html
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

// ─── config_parse ─────────────────────────────────────────────────────────────

fn bench_config_parse(c: &mut Criterion) {
    let yaml = r#"
version: "2"
env:
  rust: stable
  node: "20"
cache:
  ttl_days: 14
  remote: "https://cache.zenith.run"
jobs:
  build:
    runs-on: alpine
    backend: container
    steps:
      - name: Build
        run: cargo build --release
        watch: ["src/**/*.rs", "Cargo.toml"]
        outputs: ["target/release/myapp"]
      - name: Test
        run: cargo test
        depends_on: [Build]
"#;

    c.bench_function("config_parse", |b| {
        b.iter(|| {
            let cfg: zenith::config::ZenithConfig =
                serde_yaml::from_str(black_box(yaml)).unwrap();
            black_box(cfg);
        });
    });
}

// ─── cache_key_hash ───────────────────────────────────────────────────────────

fn bench_cache_key_hash(c: &mut Criterion) {
    use sha2::{Digest, Sha256};

    let command = "cargo build --release -- target/release/myapp Cargo.toml src/main.rs";
    let env_str = "RUST_BACKTRACE=1 CARGO_TERM_COLOR=always";

    c.bench_function("cache_key_hash", |b| {
        b.iter(|| {
            let mut h = Sha256::new();
            h.update(black_box(command.as_bytes()));
            h.update(black_box(env_str.as_bytes()));
            let result = h.finalize();
            black_box(result);
        });
    });
}

// ─── derivation_id ────────────────────────────────────────────────────────────

fn bench_derivation_id(c: &mut Criterion) {
    let step = zenith::config::Step {
        name: Some("Build".into()),
        run: "cargo build --release".into(),
        env: None,
        working_directory: None,
        allow_failure: false,
        cache: None,
        watch: vec!["src/**/*.rs".into(), "Cargo.toml".into()],
        outputs: vec!["target/release/myapp".into()],
        cache_key: None,
        depends_on: vec![],
    };
    let env: HashMap<String, String> = HashMap::new();

    c.bench_function("derivation_id", |b| {
        b.iter(|| {
            let drv = zenith::build::derivation::Derivation::from_step(
                black_box(&step),
                black_box(&env),
                "alpine",
                "x86_64",
            );
            black_box(drv.id());
        });
    });
}

// ─── config_migrate ───────────────────────────────────────────────────────────

fn bench_config_migrate(c: &mut Criterion) {
    let v1_yaml = r#"
jobs:
  build:
    runs-on: alpine
    steps:
      - name: Build
        run: cargo build
      - name: Test
        run: cargo test
"#;

    c.bench_function("config_migrate", |b| {
        b.iter(|| {
            let result = zenith::config::migrate_v1_to_v2(black_box(v1_yaml)).unwrap();
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_config_parse,
    bench_cache_key_hash,
    bench_derivation_id,
    bench_config_migrate,
);
criterion_main!(benches);
