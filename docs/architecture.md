# CLI Engineer Architecture

This document provides a high-level overview of the technical architecture of `cli_engineer`, an experimental autonomous CLI coding agent. The system is designed around an agentic loop that facilitates planning, execution, and review cycles to complete software engineering tasks.

## Core Philosophy

The architecture is modular and event-driven, centered around a core `AgenticLoop`. Components are designed to be loosely coupled, communicating primarily through an `EventBus`. This allows for flexible UI implementations and clear separation of concerns.

## Key Components

The application is composed of several key modules, each with a distinct responsibility:

-   **`main.rs`**: The application's entry point. It handles command-line argument parsing (using `clap`), sets up the configuration, initializes the appropriate UI (`DashboardUI` or `EnhancedUI`), and kicks off the main task.

-   **`AgenticLoop`**: The heart of the agent. It orchestrates the entire workflow by iteratively calling the Planner, Executor, and Reviewer until the task is complete or the maximum number of iterations is reached.

-   **`Interpreter`**: Takes the initial raw user input and translates it into a structured `Task` with a clear goal. This is the first step in understanding the user's intent.

-   **`Planner`**: Receives a `Task` and uses an LLM to create a detailed, step-by-step `Plan`. It considers the `IterationContext` to adapt the plan based on previous results and feedback.

-   **`Executor`**: Executes each `Step` in the `Plan`. For coding tasks, it constructs a specific prompt for the LLM to generate code, which is then saved as an artifact.

-   **`Reviewer`**: Analyzes the results from the `Executor`. It uses an LLM to assess the quality of the generated artifacts, identify issues, and determine if the task meets its goal. Its feedback is crucial for the iterative refinement process.

-   **`LLMManager`**: An abstraction layer that manages interactions with various Large Language Model (LLM) providers (OpenAI, Anthropic, Gemini, Ollama, etc.). It selects the active provider based on configuration and handles sending prompts and receiving responses.

-   **`ArtifactManager`**: Manages the lifecycle of generated files (artifacts). It handles creating, updating, and storing files in the designated artifact directory.

-   **`ContextManager`**: Maintains the conversational and codebase context for the LLM. It gathers relevant source files and conversation history, ensuring the LLM has the necessary information to perform its tasks. It also handles context window limits through summarization and compression.

-   **`EventBus`**: A central, asynchronous, publish-subscribe system for communication between components. This decoupling allows the UI and loggers to react to events from the core logic without being directly coupled.

-   **UI (`ui_dashboard.rs`, `ui_enhanced.rs`)**: Provides user-facing interfaces. The `DashboardUI` offers a real-time, in-place updating terminal dashboard, while the `EnhancedUI` provides a more traditional scrolling output with progress bars. Both listen to the `EventBus` for updates.

## The Agentic Workflow

The primary workflow follows a "Plan-Execute-Review" cycle managed by the `AgenticLoop`.

1.  **Interpretation**: The user's prompt is passed to the `Interpreter` to define the `Task`.
2.  **Context Gathering**: The `ContextManager` scans the current directory for relevant source code files to provide context to the LLM.
3.  **Planning**: The `Planner` receives the `Task` and the current context, queries the LLM, and produces a `Plan` containing a sequence of `Step`s.
4.  **Execution**: The `Executor` takes the `Plan` and executes each `Step` one by one. This usually involves prompting the LLM to generate code or other content.
    -   Generated files are saved via the `ArtifactManager`.
5.  **Review**: The `Reviewer` examines the results of the execution. It prompts the LLM to check for correctness, quality, and completeness.
    -   The review produces a `ReviewResult` containing a list of issues and a quality assessment.
6.  **Iteration**:
    -   If the `Reviewer` determines the task is complete (`ready_to_deploy: true`), the loop terminates successfully.
    -   If issues are found, the `IterationContext` is updated with the feedback, and the loop repeats from the **Planning** phase. The `Planner` will use the new context to create a revised plan aimed at fixing the identified issues.

This cycle continues until the goal is achieved or the configured `max_iterations` limit is hit.

## Architectural Diagram

The following diagram illustrates the flow of control and data between the major components.

```plaintext
[User Input] -> [main] -> [Interpreter] -> [Task]
                                             |
                                             v
+--------------------------------------> [AgenticLoop] <--------------------------------------+
|                                            |                                                  |
|                                            v                                                  |
|  +-----------------> [Planner] --(Plan)--> [Executor] --(Results)--> [Reviewer] --------------+
|  |                       ^                      ^                          ^                  |
|  | (IterationContext)    |                      |                          | (ReviewResult)   |
|  |                       |                      |                          |                  |
|  +-----------------------+----------------------|--------------------------+                  |
|                                                 |                                             |
|                                                 v                                             |
|                                          [LLMManager] -> (OpenAI, Anthropic, Gemini, Ollama)   |
|                                                 ^                                             |
|                                                 |                                             |
+--- [ContextManager] <--- [Files] <--- [ArtifactManager] <-------------------------------------+
       |         ^
       |         | (Events)
       +-----> [EventBus] <---- [UI/Logger]
```