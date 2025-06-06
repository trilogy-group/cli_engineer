# CLI Engineer Documentation Structure

This document outlines the documentation organization for the CLI Engineer project - an autonomous CLI coding agent with pluggable LLM providers, task interpretation, planning, execution, review, and an agentic loop.

## Documentation Organization

The documentation is organized into the following logical sections:

### 1. API Documentation (`docs/api/`)
- **Core Components**: LLM Manager, Event Bus, Context Manager
- **Provider Interfaces**: OpenAI, Anthropic, OpenRouter providers
- **Execution Engine**: Planner, Executor, Reviewer components
- **Artifact Management**: File creation and management APIs
- **Configuration**: Configuration structures and options

### 2. Architecture Documentation (`docs/architecture/`)
- **System Overview**: High-level architecture and component interactions
- **Agentic Loop**: The core planning-execution-review cycle
- **Event-Driven Design**: Event bus and component communication
- **Context Management**: Token management and compression strategies
- **Provider Architecture**: Pluggable LLM provider system

### 3. User Guide (`docs/user-guide/`)
- **Installation**: Setup and installation instructions
- **Command Reference**: CLI commands and options
- **Configuration**: How to configure providers and settings
- **Examples**: Common use cases and workflows
- **Troubleshooting**: Common issues and solutions

## Key Components Overview

### Core Architecture
The CLI Engineer follows an event-driven architecture with these main components:

```rust
// Core components
pub struct AgenticLoop;    // Main orchestrator
pub struct LLMManager;     // Manages AI providers
pub struct EventBus;       // Event communication
pub struct ContextManager; // Token and context management
```

### Command Structure
The CLI supports multiple command types:

```bash
# Code generation
cli_engineer code "create a Python hello world script"

# Code review
cli_engineer review "focus on security issues"

# Documentation generation
cli_engineer docs "create API documentation"

# Refactoring
cli_engineer refactor "improve error handling"

# Security analysis
cli_engineer security "scan for vulnerabilities"
```

### Configuration System
Configuration is managed through TOML files with support for:

- **AI Provider Settings**: API keys, models, cost tracking
- **Execution Parameters**: Iteration limits, parallel processing
- **UI Options**: Dashboard mode, progress bars, output formats
- **Context Management**: Token limits, compression settings

### Provider System
Pluggable LLM providers support:

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    fn context_size(&self) -> usize;
    async fn send_prompt(&self, prompt: &str) -> Result<String>;
    fn model_name(&self) -> &str;
}
```

### Event System
The event bus enables loose coupling between components:

```rust
pub enum Event {
    TaskStarted { task_id: String, description: String },
    TaskProgress { task_id: String, progress: f32, message: String },
    TaskCompleted { task_id: String, result: String },
    ArtifactCreated { name: String, path: String, artifact_type: String },
    APICallStarted { provider: String, model: String },
    // ... more events
}
```

## File Structure

The project follows this organization:

```
cli_engineer/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── agentic_loop.rs        # Core orchestration
│   ├── llm_manager.rs         # LLM provider management
│   ├── event_bus.rs           # Event communication
│   ├── context.rs             # Context and token management
│   ├── artifact.rs            # File and artifact management
│   ├── config.rs              # Configuration management
│   ├── planner.rs             # Task planning
│   ├── executor.rs            # Task execution
│   ├── reviewer.rs            # Code review and quality
│   ├── interpreter.rs         # Task interpretation
│   ├── providers/             # LLM provider implementations
│   │   ├── openai.rs
│   │   ├── anthropic.rs
│   │   └── openrouter.rs
│   └── ui/                    # User interface components
│       ├── ui.rs
│       ├── ui_dashboard.rs
│       └── ui_enhanced.rs
├── docs/                      # Documentation
├── artifacts/                 # Generated artifacts
└── cli_engineer.toml         # Configuration file
```

## Documentation Standards

### Code Examples
All code examples should:
- Use proper syntax highlighting
- Include relevant imports and context
- Demonstrate real usage patterns
- Be tested and verified

### API Documentation
API documentation should include:
- Function signatures and parameters
- Return types and error conditions
- Usage examples
- Integration patterns

### Architecture Documentation
Architecture docs should cover:
- Component responsibilities
- Data flow and interactions
- Design decisions and rationales
- Extension points and customization

This documentation structure provides comprehensive coverage of the CLI Engineer system while maintaining clear organization and accessibility for different types of users.