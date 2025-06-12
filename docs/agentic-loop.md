# The Agentic Loop

The Agentic Loop is the core operational engine of `cli_engineer`. It facilitates an autonomous, iterative cycle of planning, execution, and review to accomplish complex software engineering tasks. This mechanism allows the agent to break down a high-level goal, act on it, assess its own work, and self-correct until the objective is met.

The entire process is managed by the `AgenticLoop` struct defined in `src/agentic_loop.rs`.

## The Core Cycle

The loop follows a distinct, repeating sequence of phases for each iteration. The state and knowledge from one iteration are passed to the next via the `IterationContext`.

The flow can be summarized as:

1.  **Interpret**: The initial user request is parsed into a structured `Task`.
2.  **Plan**: A detailed, step-by-step `Plan` is created to achieve the `Task`'s goal.
3.  **Execute**: The agent carries out each `Step` in the `Plan`.
4.  **Review**: The results of the execution are critically evaluated for correctness and quality.
5.  **Repeat or Finish**: If the review determines the task is complete, the loop terminates. Otherwise, it begins a new iteration, using the review's feedback to create a revised plan.

This cycle continues until the task is successfully completed or the configured `max_iterations` limit is reached.

### 1. Interpretation

-   **Source**: `src/interpreter.rs`
-   The process begins when the `Interpreter` receives the raw user input. It analyzes the prompt to define a clear `Task` with a high-level `description` and a specific `goal`.

### 2. Planning

-   **Source**: `src/planner.rs`
-   The `Planner` is responsible for creating a structured `Plan`. It uses the LLM to break down the `Task`'s goal into a sequence of actionable `Step`s.
-   Crucially, the `Planner` is given the current `IterationContext`. This allows it to make informed decisions based on what has already been accomplished, what files exist, and what issues were found in the previous iteration. For example, if a file already exists, the planner will generate a `CodeModification` step instead of a `CodeGeneration` step.

### 3. Execution

-   **Source**: `src/executor.rs`
-   The `Executor` takes the `Plan` and processes each `Step` sequentially.
-   For each step, it constructs a specific prompt for the LLM based on the step's category (e.g., `CodeGeneration`, `FileOperation`, `Documentation`).
-   The `Executor` is responsible for interacting with the `ArtifactManager` to create or modify files based on the LLM's output. The outcome of this phase is a collection of `StepResult` objects.

### 4. Review

-   **Source**: `src/reviewer.rs`
-   The `Reviewer` acts as the quality assurance gate. It examines the `StepResult`s from the execution phase.
-   Using the LLM, it assesses the work against the original plan, identifying `Issue`s and evaluating the `overall_quality`.
-   The output is a `ReviewResult`, which contains a list of issues, suggestions, and a `ready_to_deploy` flag. This flag is the primary signal for terminating the loop successfully.

## State Management: The `IterationContext`

-   **Source**: `src/iteration_context.rs`

The `IterationContext` is the "memory" of the agent, carrying state between iterations. It is the key to the agent's ability to learn and self-correct.

```rust
pub struct IterationContext {
    pub iteration: usize,
    pub existing_files: HashMap<String, FileInfo>,
    pub last_review: Option<ReviewResult>,
    pub pending_issues: Vec<Issue>,
    pub progress_summary: String,
}
```

Its primary responsibilities are:
-   **`existing_files`**: Tracks all files that have been created or modified. This prevents the agent from re-creating files and helps the `Planner` decide between generation and modification.
-   **`pending_issues` & `last_review`**: This is the feedback mechanism. The issues identified by the `Reviewer` in one iteration are fed directly into the `Planner` in the next. This prompts the agent to generate steps that specifically address and fix the problems it found in its own work.

By passing this context object through each loop, the agent builds a progressively more accurate understanding of the project's state and what needs to be done next.