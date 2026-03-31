# Parallel Step Execution

Zenith runs steps concurrently when their dependencies are satisfied. You express the real dependency graph using `depends_on:` — Zenith does the rest.

---

## How it works

On each iteration of the executor loop, Zenith:

1. Inspects all pending steps
2. Starts every step whose `depends_on` entries are all in the completed set
3. Waits for any running step to finish
4. Adds the finished step to the completed set
5. Repeats until all steps are done or a failure aborts the run

Steps with no `depends_on` (or an empty list) start immediately and run in parallel.

---

## Basic example

```yaml
steps:
  - name: Install
    run: npm install
    outputs: [node_modules/]

  - name: Lint
    run: npm run lint
    depends_on: [Install]     # waits for Install

  - name: Test
    run: npm test
    depends_on: [Install]     # also waits for Install

  # Lint and Test run concurrently after Install completes.

  - name: Build
    run: npm run build
    outputs: [dist/]
    depends_on: [Lint, Test]  # waits for both
```

**Execution timeline:**

```
t=0   Install ──────────────────┐
t=8s                            ├── Lint ────────┐
                                └── Test ────────┤
t=14s                                            └── Build ──────
```

Wall-clock time: 8s (Install) + 6s (Lint/Test, parallel) + 4s (Build) = 18s  
Sequential time would be: 8 + 4 + 6 + 4 = 22s

---

## Diamond dependency pattern

```yaml
steps:
  - name: Fetch deps
    run: cargo fetch
    watch: [Cargo.lock]

  - name: Build debug
    run: cargo build
    depends_on: [Fetch deps]

  - name: Build release
    run: cargo build --release
    outputs: [target/release/myapp]
    depends_on: [Fetch deps]

  - name: Test
    run: cargo test
    depends_on: [Build debug]

  - name: Package
    run: tar czf myapp.tar.gz target/release/myapp
    outputs: [myapp.tar.gz]
    depends_on: [Build release, Test]
```

`Build debug` and `Build release` run in parallel after `Fetch deps`. `Package` waits for both.

---

## Cycle detection

If `depends_on` entries form a cycle, Zenith detects it and aborts with a warning:

```
warn: Dependency cycle detected — steps still pending: [A, B, C]
      Check that depends_on entries do not form a loop.
```

No deadlock — Zenith notices that no step can make progress and exits cleanly.

---

## Logging with concurrent steps

Each step's log lines are prefixed with the step name so interleaved output is readable:

```
[Lint] Running eslint src/
[Test] Running jest
[Lint] ✓ No lint errors
[Test] PASS  tests/api.test.ts
[Test] PASS  tests/ui.test.ts
[Lint] ✓ Lint   0.4s
[Test] ✓ Test   2.1s
[Build] Running npm run build
```

---

## Interaction with caching

Each step's cache check happens independently when the step starts. Two parallel steps can both hit the cache simultaneously — Zenith handles this safely using concurrent log access via an `Arc<Mutex<RunLogger>>`.

If one parallel step fails with `allow_failure: false`, Zenith cancels all remaining running steps and aborts the pipeline.
