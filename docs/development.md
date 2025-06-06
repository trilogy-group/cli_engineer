# Development Documentation

This document provides comprehensive information for developers working on the CLI Engineer project, including contribution guidelines, code structure, and extension guides.

## Table of Contents

- [Contributing Guidelines](#contributing-guidelines)
- [Code Structure](#code-structure)
- [Extending the System](#extending-the-system)
- [Development Setup](#development-setup)
- [Testing](#testing)
- [Code Style](#code-style)

## Contributing Guidelines

### Getting Started

1. **Fork the Repository**
   - Fork the repository on GitHub
   - Clone your fork locally
   - Add the upstream repository as a remote

2. **Development Environment**
   - Ensure you have Rust 1.70+ installed
   - Install required tools: `cargo`, `rustfmt`, `clippy`
   - Set up your IDE with Rust support

3. **Making Changes**
   - Create a new branch for your feature/fix
   - Follow the coding standards outlined below
   - Write tests for new functionality
   - Update documentation as needed

4. **Pull Request Process**
   - Ensure all tests pass
   - Run `cargo fmt` and `cargo clippy`
   - Write clear commit messages
   - Submit a pull request with a detailed description

### Code Review Process

- All changes require review from at least one maintainer
- Automated checks must pass (tests, formatting, linting)
- Documentation updates are required for user-facing changes
- Performance implications should be considered for core components

## Code Structure

The CLI Engineer codebase is organized into several key modules:

### Core Architecture

```
src/
├── main.rs              # Application entry point and CLI parsing
├── config.rs            # Configuration management
├── event_bus.rs         # Event system for component communication
├── agentic_loop.rs      # Main orchestration logic
├── interpreter.rs       # Task interpretation
├── planner.rs           # Plan creation and management
├── executor.rs          # Step execution
├── reviewer.rs          # Code review and quality assessment
├── context.rs           # Context and conversation management
├── artifact.rs          # File and artifact management
├── llm_manager.rs       # LLM provider abstraction
├── ui/                  # User interface components
│   ├── ui.rs
│   ├── ui_dashboard.rs
│   └── ui_enhanced.rs
├── providers/           # LLM provider implementations
│   ├── mod.rs
│   ├── openai.rs
│   ├── anthropic.rs
│   ├── openrouter.rs
│   └── ollama.rs
└── utils/               # Utility modules
    ├── logger.rs
    ├── logger_dashboard.rs
    ├── concurrency.rs
    └── iteration_context.rs
```

### Module Responsibilities

#### Core Modules

**main.rs**
- CLI argument parsing using `clap`
- Application initialization and teardown
- Command routing and execution

**agentic_loop.rs**
- Orchestrates the interpret → plan → execute → review cycle
- Manages iteration context between cycles
- Handles error recovery and retry logic

**config.rs**
- Configuration file parsing (TOML)
- Environment variable integration
- Provider-specific settings management

**event_bus.rs**
- Pub/sub event system for loose coupling
- Metrics collection and aggregation
- Cross-component communication

#### Task Processing

**interpreter.rs**
- Converts natural language input into structured tasks
- Extracts goals and requirements from user input
- Currently uses simple heuristics (can be enhanced with NLP)

**planner.rs**
- Creates structured execution plans from tasks
- Categorizes steps by type (FileOperation, CodeGeneration, etc.)
- Manages step dependencies and complexity estimation

**executor.rs**
- Executes individual plan steps
- Handles different step categories with appropriate prompting
- Manages artifact creation and file operations

**reviewer.rs**
- Evaluates execution results for quality and correctness
- Identifies issues and suggests improvements
- Determines if tasks are ready for deployment

#### Data Management

**context.rs**
- Manages conversation history and context
- Implements context compression to stay within token limits
- Handles context caching and persistence

**artifact.rs**
- Manages created files and artifacts
- Provides artifact metadata and versioning
- Handles cleanup and organization

**llm_manager.rs**
- Abstracts LLM provider differences
- Manages API calls and token counting
- Handles cost tracking and provider switching

#### Providers

**providers/openai.rs**
- OpenAI API integration
- Handles GPT-4, GPT-3.5-turbo, and other OpenAI models
- Implements proper error handling and rate limiting

**providers/anthropic.rs**
- Anthropic Claude API integration
- Supports Claude-3 family models
- Handles Anthropic-specific message formatting

**providers/ollama.rs**
- Ollama local LLM integration
- Uses OpenAI-compatible API endpoints
- Supports various open-source models (Llama, Mistral, etc.)

**providers/openrouter.rs**
- OpenRouter API integration for multiple model access
- Supports various open-source models
- Cost-effective alternative to direct API access

### Data Flow

1. **Input Processing**: User input → Interpreter → Task structure
2. **Planning**: Task → Planner (+ LLM) → Structured Plan
3. **Execution**: Plan → Executor (+ LLM) → Step Results + Artifacts
4. **Review**: Results → Reviewer (+ LLM) → Quality Assessment
5. **Iteration**: Review → Agentic Loop → Next iteration or completion

### Event System

The event bus enables loose coupling between components:

```rust
// Components emit events
event_bus.emit(Event::TaskStarted { 
    task_id: "123".to_string(),
    description: "Generate code".to_string() 
}).await?;

// UI components subscribe to events
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    // Handle event
}
```

## Extending the System

### Adding New LLM Providers

1. **Create Provider Module**
   Create a new file in `src/providers/` implementing the `LLMProvider` trait:

```rust
use async_trait::async_trait;
use crate::llm_manager::LLMProvider;

pub struct NewProvider {
    api_key: String,
    model: String,
    // other fields
}

#[async_trait]
impl LLMProvider for NewProvider {
    fn name(&self) -> &str { "new_provider" }
    fn context_size(&self) -> usize { 8192 }
    fn model_name(&self) -> &str { &self.model }
    
    async fn send_prompt(&self, prompt: &str) -> anyhow::Result<String> {
        // Implementation
    }
}
```

2. **Update Configuration**
   Add provider config to `config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProvidersConfig {
    pub openai: Option<ProviderConfig>,
    pub anthropic: Option<ProviderConfig>,
    pub new_provider: Option<ProviderConfig>, // Add this
    // ...
}
```

3. **Integration**
   Update `main.rs` to initialize the new provider based on configuration.

### Adding New Step Categories

1. **Extend StepCategory Enum**
   In `planner.rs`, add new categories:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepCategory {
    Analysis,
    FileOperation,
    CodeGeneration,
    NewCategory, // Add this
    // ...
}
```

2. **Update Executor**
   In `executor.rs`, handle the new category in `build_step_prompt()` and `execute_step()`.

3. **Update Planner**
   In `planner.rs`, add keyword detection for the new category in `create_step_from_lines()`.

### Adding New Event Types

1. **Extend Event Enum**
   In `event_bus.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Existing events...
    NewEventType {
        field1: String,
        field2: i32,
    },
}
```

2. **Update Metrics**
   If the event affects metrics, update `update_metrics()` in `event_bus.rs`.

3. **Handle in UI**
   Update UI components to handle the new event type if needed.

### Adding New Artifact Types

1. **Extend ArtifactType**
   In `artifact.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    SourceCode,
    Configuration,
    NewType, // Add this
    // ...
}
```

2. **Update File Extension Mapping**
   Update the artifact creation logic to handle new file types.

### Creating Custom UI Components

1. **Implement EventEmitter**
   Use the provided macro:

```rust
use crate::impl_event_emitter;

pub struct CustomUI {
    event_bus: Option<Arc<EventBus>>,
    // other fields
}

impl_event_emitter!(CustomUI);
```

2. **Handle Events**
   Subscribe to relevant events and update your UI accordingly.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Git
- An editor with Rust support (VS Code with rust-analyzer recommended)

### Environment Variables

Set up API keys for testing:

```bash
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key" 
export OPENROUTER_API_KEY="your-key"
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Development Commands

```bash
# Check code formatting
cargo fmt --check

# Apply formatting
cargo fmt

# Run linter
cargo clippy

# Check for security vulnerabilities
cargo audit

# Run with verbose logging
cargo run -- -v "test task"

# Run with simple text UI (opt-out of dashboard)
cargo run -- --no-dashboard "test task"
```

## Testing

### Test Structure

- Unit tests are located alongside the code they test
- Integration tests are in the `tests/` directory
- Test utilities are in `tests/common/`

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_async_function() {
        // Async test implementation
    }
}
```

### Test Categories

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test component interactions
3. **End-to-End Tests**: Test complete workflows
4. **Performance Tests**: Benchmark critical paths

### Mocking

Use the LocalProvider for testing without real API calls:

```rust
let providers: Vec<Box<dyn LLMProvider>> = vec![
    Box::new(LocalProvider)
];
let llm_manager = LLMManager::new(providers, event_bus, config);
```

## Code Style

### Formatting

- Use `cargo fmt` for consistent formatting
- Line length: 100 characters
- Use 4 spaces for indentation

### Naming Conventions

- Types: `PascalCase`
- Functions and variables: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

### Documentation

- Use `///` for public API documentation
- Include examples in doc comments
- Document all public functions and types

```rust
/// Creates a new task from user input.
/// 
/// # Arguments
/// 
/// * `input` - The user's natural language input
/// 
/// # Examples
/// 
/// ```
/// let task = interpreter.interpret("create a hello world script")?;
/// assert_eq!(task.description, "create a hello world script");
/// ```
pub fn interpret(&self, input: &str) -> Result<Task> {
    // Implementation
}
```

### Error Handling

- Use `anyhow::Result` for error propagation
- Provide context with `.context()`
- Use `thiserror` for custom error types

```rust
use anyhow::{Context, Result};

fn read_config() -> Result<Config> {
    std::fs::read_to_string("config.toml")
        .context("Failed to read configuration file")?;
    // ...
}
```

### Async Code

- Use `async/await` for IO operations
- Prefer `tokio` primitives for concurrency
- Use `Arc<RwLock<T>>` for shared mutable state

### Performance Considerations

- Use `&str` instead of `String` when possible
- Prefer borrowing over cloning
- Use `Vec::with_capacity()` when size is known
- Profile performance-critical paths

This development documentation should help both new and experienced contributors understand the codebase structure and how to effectively extend the CLI Engineer system.