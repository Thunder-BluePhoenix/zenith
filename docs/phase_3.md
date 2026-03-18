# Phase 3: Matrix Runner (Multi-OS Pipelines)

## Objective
Enable Zenith to execute a single workflow across a matrix of different operating systems or environments concurrently, simulating full CI behavior locally (**Idea 1**).

## Technical Approach
We will introduce concurrent orchestration. The Workflow Engine from Phase 2 will be upgraded to spawn multiple parallel threads, each instantiating its own isolated Lab environment and running the defined steps.

## Milestones

1.  **Matrix Configuration Parsing**
    *   Extend `.zenith.yml` parsing to support multi-dimensional matrix variables.
    *   Example:
        ```yaml
        jobs:
          test:
            strategy:
              matrix:
                os: [ubuntu, alpine, debian-slim]
            runs-on: ${{ matrix.os }}
            steps:
              - run: npm test
        ```
2.  **Parallel Execution Engine**
    *   Implement async/thread-pool management to execute jobs concurrently.
    *   Dynamically generate sub-jobs based on the matrix expansion (e.g., `test-ubuntu`, `test-alpine`).
3.  **Lab Multiplexing**
    *   Ensure the Sandbox Lab Manager (Phase 1) is thread-safe.
    *   Each matrix node must receive a unique temporary OverlayFS workspace to prevent I/O collisions when multiple OS environments write to disk simultaneously.
4.  **Log Multiplexing & UI**
    *   Implement a TUI (Terminal User Interface) or log prefixer so parallel standard outputs are human-readable.
    *   Logs must be deterministic (e.g., printing `[ubuntu] ...` alongside `[alpine] ...` without tearing output strings).
5.  **Matrix Result Aggregation**
    *   Wait for all matrix threads to conclude.
    *   Print a final success/failure summary table for the entire matrix run.

## Verification
*   `zenith matrix run` executes tests on three different Linux distros concurrently.
*   Host system CPU utilization reflects parallel workload execution.
*   Temporary lab directories are distinctly separated (`~/.zenith/labs/runner-thread-1`, `runner-thread-2`) and all safely torn down upon completion.

## Next Steps
Up to this point, isolation has relied on lightweight containers/chroots (sharing the host kernel). Phase 4 introduces MicroVMs (Firecracker) for true hardware-level OS isolation, fulfilling the core promise of a "Lightweight VM Workflow."
