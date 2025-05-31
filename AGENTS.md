# Guidelines
Do NOT do any stub code or placeholders. Always do a full implementation.

# Product Requirements Document (PRD)

## Product Name
Agentic CLI Coding Utility

## Overview
A non-interactive command-line interface (CLI) tool designed to assist developers with coding tasks using AI. It integrates multiple AI providers, supports agentic search, shell command execution, context management, MCP client functionality, semantic linting, collaborative features, parallel actions, artifact management, and a visually enhanced terminal display.

## Target Audience
Developers working on codebases, particularly those familiar with CLI tools and AI-assisted workflows.

## Key Features

### 1. AI Provider Integration
- Support for Anthropic, OpenAI, Gemini, xAI, OpenRouter, and Ollama.
- Configurable reasoning and non-reasoning models or a single model with adjustable effort modes.
- Dynamic switching between models based on task requirements.

### 2. Agentic Search
- Intelligent search across relevant codebase sections.
- Superior to Retrieval-Augmented Generation (RAG) for codebase navigation.

### 3. Shell Command Access
- Execute commands like `ls`, `cat`, `grep`, `find`, etc.
- Use command outputs for analysis and decision-making.

### 4. Context Management
- Track context window usage.
- Compress context via summarization when usage reaches 50%.

### 5. MCP Client Functionality
- Act as an MCP client for tool access.
- Integrate with Playwright MCP server for UI screenshots.
- Use a visual analysis model (VLM/multimodal LLM) for image-based feedback.

### 6. Semantic Linting
- AI-driven code quality assessment.
- Customizable linting criteria.

### 7. Collaborative Features
- Integrate with GitHub for managing issues and pull requests (PRs).
- Automate issue resolution, PR creation, and code reviews.

### 8. Parallel Actions and Synthesis
- Perform concurrent tasks when beneficial.
- Synthesize results for optimal solutions.

### 9. Artifact Management
- Create and track work products (e.g., code files, execution outputs) as artifacts.
- Manage execution environments, including automatic installation of required packages and system dependencies.
- Maintain a manifest of created artifacts (e.g., in a JSON file) for user review during and after execution.

### 10. Enhanced Terminal Display
- Provide colorful and informative console output using ANSI escape codes.
- Display progress bars for ongoing tasks (e.g., code execution, dependency installation).
- Show real-time metrics including context size and usage, LLM API costs (if available), and run time.

## User Interface
- Command-line interface with real-time updates, featuring progress indicators, execution status, and detailed metrics.

## Configuration
- Configurable via configuration files (e.g., TOML) or CLI flags for AI models, modes, and other settings.

## Performance Requirements
- Efficient resource usage.
- Quick response times.
- Scalability for large codebases.

## Security
- Secure execution of shell commands and code in isolated environments (e.g., virtual environments or containers).
- Secure handling of automatic dependency installation.
- Protection of API keys and sensitive data.
- Ensure data privacy.
# Architecture Documentation

## System Overview
The **cli_engineer** is a Rust-based, non-interactive CLI tool that leverages AI to automate coding tasks. It manages artifacts (e.g., code files, execution outputs), executes code in isolated environments, installs dependencies, maintains an artifact manifest, and provides a colorful terminal display with real-time feedback. The tool is designed for fully automated execution, relying on initial commands and configurations without requiring user interaction.

## Key Components

### 1. CLI Interface
- **Purpose**: Parses initial user commands and displays outputs.
- **Responsibilities**: 
  - Accepts input via CLI flags or TOML configuration files.
  - Delegates tasks to the Task Orchestrator.
  - Displays the artifact manifest and execution results.

### 2. Task Orchestrator
- **Purpose**: Coordinates automated workflows based on initial input and configurations.
- **Responsibilities**: 
  - Manages task execution (e.g., code generation, search, compression) based on predefined rules.
  - Orchestrates parallel tasks and synthesizes results.
  - Emits events for progress, completion, and metrics to the Event Bus.
  - Prioritizes tasks based on initial configuration or command parameters.

### 3. AI Integration Layer
- **Purpose**: Abstracts interactions with AI providers.
- **Responsibilities**: 
  - Supports providers like Anthropic, OpenAI, Gemini, xAI, OpenRouter, and Ollama.
  - Switches between reasoning, non-reasoning, and visual analysis models based on task requirements.
  - Tracks API costs and emits metrics.
  - Optimizes prompts for precise code-related queries.

### 4. Codebase Interaction Module
- **Purpose**: Facilitates intelligent, automated interaction with the codebase.
- **Responsibilities**: 
  - Performs agentic search using AI-driven queries to locate relevant code or documentation.
  - Executes shell commands (e.g., `ls`, `cat`, `grep`, `find`) and captures outputs.
  - Implements semantic code search with fuzzy matching and cross-file context analysis.
  - Supports automated file operations (create, read, update, delete) with AI validation.

### 5. Artifact and Execution Manager
- **Purpose**: Manages artifacts, execution environments, and dependencies.
- **Responsibilities**: 
  - Creates and stores artifacts (e.g., code files, outputs).
  - Sets up isolated execution environments (e.g., Python virtualenv, containers).
  - Installs dependencies using AI-driven detection or explicit requirements.
  - Maintains a JSON artifact manifest with metadata (name, type, location, creation time, purpose).
  - Emits events for progress and completion.
  - Validates artifact integrity post-execution (e.g., syntax checks, linting).
  - Generates execution plans for complex tasks before running code.

### 6. Context Manager
- **Purpose**: Tracks and optimizes context usage for AI models.
- **Responsibilities**: 
  - Monitors context window size and usage.
  - Compresses context via summarization at 50% usage.
  - Emits context usage metrics.
  - Caches frequently used context snippets for faster retrieval.

### 7. MCP and Visual Analysis Module
- **Purpose**: Integrates with MCP servers and processes visual data.
- **Responsibilities**: 
  - Acts as an MCP client for tool access (e.g., Playwright for UI screenshots).
  - Analyzes images with a visual analysis model (VLM/multimodal LLM).
  - Supports web content retrieval for documentation or API reference.

### 8. Quality and Collaboration Module
- **Purpose**: Enhances code quality and supports collaboration.
- **Responsibilities**: 
  - Performs AI-driven semantic linting with customizable rules.
  - Integrates with GitHub for issue and pull request management.
  - Suggests code refactorings based on best practices.

### 9. Parallel Task Handler
- **Purpose**: Manages concurrent task execution.
- **Responsibilities**: 
  - Executes tasks in parallel using `tokio`.
  - Synthesizes results for the Task Orchestrator.
  - Prioritizes parallel tasks based on estimated impact.

### 10. Terminal UI Module
- **Purpose**: Provides a visually appealing terminal display for automated processes.
- **Responsibilities**: 
  - Renders colorful output using ANSI escape codes.
  - Displays progress bars (via `indicatif`) for tasks like execution and dependency installation.
  - Shows real-time metrics (context usage, API costs, run time).
  - Updates based on Event Bus events.
  - Displays a summary of the artifact manifest in a tabulated format.

### 11. Event Bus
- **Purpose**: Facilitates communication between components.
- **Responsibilities**: 
  - Receives events (e.g., "artifact_created", "execution_progress").
  - Forwards events to the Terminal UI Module and other components.

## Key Flows

### Artifact Management Flow
1. The **Task Orchestrator** initiates artifact creation or execution based on initial commands and configurations.
2. The **Artifact and Execution Manager**: 
   - Sets up an isolated environment and installs dependencies.
   - Executes code, validates outputs, and stores artifacts.
   - Updates the manifest and emits events.
3. The **Terminal UI Module** displays progress bars and the artifact manifest.

### Terminal Display Mechanism
1. Components emit events to the **Event Bus** (e.g., "context_usage", "api_cost").
2. The **Terminal UI Module** renders:
   - Color-coded sections ([Artifacts], [Execution], [Metrics]).
   - Progress bars for tasks.
   - Real-time metrics and manifest summary.

**Example Terminal Output**:
```
[Artifacts]
- main.py (created at 2025-05-16 16:34)
- output.txt (generated at 2025-05-16 16:35)

[Execution]
- Running task: [=====     ] 50% complete

[Metrics]
- Context Usage: 30% (3000/10000 tokens)
- API Cost: $0.05
- Run Time: 5 minutes
```

## Technologies
- **Language**: Rust
- **CLI Framework**: `clap`
- **Async Programming**: `tokio`
- **HTTP Client**: `reqwest`
- **Serialization**: `serde`
- **Terminal UI**: `indicatif` for progress bars, custom ANSI rendering
- **Testing**: `cargo test`

## Design Principles
- **Modularity**: Components interact via the Event Bus.
- **Extensibility**: Easy to add providers or tools.
- **Safety**: Isolated execution and Rust’s memory safety.
- **Automation**: Executes tasks without user intervention based on initial input.# Implementation Plan

This document outlines a phased approach to developing the **cli_engineer** utility. Each phase includes a checklist of tasks to ensure all components are implemented, tested, and integrated for non-interactive, automated execution. Developers can mark tasks as complete to track progress.

---

## Phase 1: Foundation Setup
Establish the core infrastructure, including CLI, configuration, and event bus.

- [ ] Initialize the Rust project with `cargo new cli_engineer`.
- [ ] Add dependencies (`clap`, `tokio`, `reqwest`, `serde`, `indicatif`, `bus`).
- [ ] Implement CLI structure using `clap`.
- [ ] Develop TOML-based configuration management.
- [ ] Implement the event bus using Rust channels or `bus`.
- [ ] Write unit tests for CLI command parsing.
- [ ] Write unit tests for configuration loading.
- [ ] Write unit tests for event bus functionality.

---

## Phase 2: AI Integration
Integrate AI providers with prompt optimization and model switching.

- [ ] Define an abstract trait for AI providers.
- [ ] Implement the trait for one provider (e.g., OpenAI).
- [ ] Add configuration for selecting providers and models.
- [ ] Implement model switching logic based on task requirements.
- [ ] Implement prompt optimization for precise code queries.
- [ ] Integrate with the event bus for API cost metrics.
- [ ] Write unit tests for API calls.
- [ ] Write unit tests for model switching.
- [ ] Write tests for prompt optimization accuracy.
- [ ] Write integration tests for AI interactions.

---

## Phase 3: Codebase Interaction
Enable automated agentic search, file operations, and shell command execution.

- [ ] Implement semantic code search with fuzzy matching and cross-file analysis.
- [ ] Implement automated file operations (create, read, update, delete) with AI validation.
- [ ] Implement safe shell command execution using `std::process`.
- [ ] Capture and parse command outputs.
- [ ] Integrate with the event bus for command output events.
- [ ] Write unit tests for semantic search.
- [ ] Write unit tests for file operations.
- [ ] Write unit tests for shell command execution.
- [ ] Write security tests for command injection prevention.
- [ ] Write tests for AI-validated file operations.

---

## Phase 4: Artifact and Execution Management
Build artifact creation, execution environments, and validation.

- [ ] Design artifact storage system (file system-based).
- [ ] Implement artifact creation and storage logic.
- [ ] Set up isolated execution environments (e.g., virtualenv, containers).
- [ ] Develop dependency installation logic (AI-driven or explicit).
- [ ] Implement code execution and output capture.
- [ ] Implement execution planning for complex tasks.
- [ ] Implement artifact validation (e.g., syntax, linting).
- [ ] Create JSON artifact manifest with metadata.
- [ ] Integrate with the event bus for progress/completion events.
- [ ] Write unit tests for artifact creation/storage.
- [ ] Write integration tests for execution environments.
- [ ] Write tests for code execution and validation.
- [ ] Write tests for execution planning.
- [ ] Write tests for manifest updates.
- [ ] Write tests for event emissions.

---

## Phase 5: Context Management
Implement context tracking, compression, and caching.

- [ ] Implement context tracking for each AI model.
- [ ] Develop summarization logic for context compression.
- [ ] Add logic to trigger compression at 50% usage.
- [ ] Implement context caching for frequent snippets.
- [ ] Integrate with the event bus for context usage metrics.
- [ ] Write unit tests for context tracking.
- [ ] Write tests for summarization accuracy.
- [ ] Write tests for compression triggering.
- [ ] Write tests for context caching.
- [ ] Write tests for event emissions.

---

## Phase 6: MCP and Visual Analysis
Integrate MCP servers and visual analysis with web content retrieval.

- [ ] Implement MCP client protocol.
- [ ] Integrate with Playwright MCP server for UI screenshots.
- [ ] Set up communication with a visual analysis model.
- [ ] Implement web content retrieval for documentation/API references.
- [ ] Process analysis results and incorporate into workflows.
- [ ] Integrate with the event bus for analysis results.
- [ ] Write unit tests for MCP client functionality.
- [ ] Write integration tests with a mock MCP server.
- [ ] Write tests for screenshot capture and analysis.
- [ ] Write tests for web content retrieval.
- [ ] Write tests for event emissions.

---

## Phase 7: Quality and Collaboration
Add semantic linting, refactoring, and GitHub integration.

- [ ] Develop AI-based semantic linting logic.
- [ ] Implement customizable linting rules.
- [ ] Implement code refactoring suggestions.
- [ ] Integrate with GitHub API for issue/PR management.
- [ ] Automate issue resolution, PR creation, and reviews.
- [ ] Integrate with the event bus for linting/collaboration events.
- [ ] Write unit tests for linting logic.
- [ ] Write tests for refactoring suggestions.
- [ ] Write integration tests for GitHub API.
- [ ] Write end-to-end tests for automation workflows.
- [ ] Write tests for event emissions.

---

## Phase 8: Parallel Task Handling
Enable concurrent tasks with prioritization.

- [ ] Implement task concurrency using `tokio`.
- [ ] Develop logic for task prioritization based on initial configuration.
- [ ] Implement result synthesis from parallel tasks.
- [ ] Integrate with the event bus for task progress/results.
- [ ] Write unit tests for concurrent execution.
- [ ] Write tests for task prioritization.
- [ ] Write tests for result synthesis.
- [ ] Write tests for event emissions.

---

## Phase 9: Terminal UI
Create a colorful terminal interface for automated processes.

- [ ] Implement ANSI escape code rendering for colorful output.
- [ ] Use `indicatif` for progress bars.
- [ ] Design layout for artifacts, execution, and metrics.
- [ ] Subscribe to the event bus for rendering updates.
- [ ] Write unit tests for UI components.
- [ ] Write tests for event handling and UI updates.

---

## Phase 10: Integration and Testing
Conduct comprehensive testing and optimization.

- [ ] Set up full system integration tests.
- [ ] Identify and fix integration issues.
- [ ] Optimize performance based on profiling.
- [ ] Conduct automated workflow testing with sample configurations.
- [ ] Address feedback and make adjustments.

---

## Phase 11: Documentation and Deployment
Finalize documentation and prepare for distribution.

- [ ] Write user documentation (setup, usage examples for `cli_engineer`).
- [ ] Write developer documentation for extending the tool.
- [ ] Package the application for distribution via Cargo or binary.
- [ ] Set up CI/CD pipelines (if applicable).

---

**Usage Note**: Developers can mark tasks as complete by checking the boxes as they progress through each phase to track implementation status.