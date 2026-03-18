# Phase 2: Workflow Engine (Local CI)

## Objective
Implement a GitHub Actions-style workflow runner that automates sequential task execution based on a declarative YAML configuration. This establishes Zenith as a robust Local CI engine (**Idea 3**).

## Technical Approach
We introduce the concept of "Jobs" and "Steps," allowing Zenith to read a `.zenith.yml` file, spin up the required Lab from Phase 1, run an automated sequence of commands, and handle cleanup.

## Milestones

1.  **Workflow Schema & Parser Expansion**
    *   Extend the CLI parser in Phase 0 to comprehend advanced YAML keys: `steps`, `env`, `uses`, `working_directory`.
    *   Example `.zenith.yml`:
        ```yaml
        jobs:
          build:
            runs-on: ubuntu-latest
            steps:
              - name: Install dependencies
                run: make install
              - name: Run unit tests
                run: make test
        ```
2.  **Sequential Step Executor**
    *   For each step defined in a job, inject the command into the designated Lab Environment.
    *   Capture standard output and standard error, tagging logs with prefix labels (e.g., `[build:step1]`).
3.  **State and Environment Propagation**
    *   Allow environment variables (`env`) to be passed down into the Lab environment.
    *   Maintain filesystem state between sequential steps (changes made in Step 1 must be available in Step 2).
4.  **Error Handling & Exit Codes**
    *   If a step exits with a non-zero status code, immediately halt the pipeline, mark the workflow as failed, and print the error block.
    *   Always ensure the Lab environment is destroyed (or recycled if specified) post-execution, even on failures.
5.  **Interactive Run Mode**
    *   `zenith run`: Executes the default workflow in the current directory.
    *   `zenith run <job_name>`: Executes a specific job.

## Verification
*   A workflow with three steps runs them sequentially inside a single lab instance.
*   If `make test` fails with code 1, Zenith aborts, cleans up, and returns an overarching failure code to the host terminal.
*   Environment variables defined in `.zenith.yml` correctly resolve inside the sandbox.

## Next Steps
With sequential local CI established, Phase 3 will introduce concurrent matrix execution to test against multiple environments simultaneously.
