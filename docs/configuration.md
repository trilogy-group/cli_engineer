# Configuration Guide

This document provides a comprehensive guide to configuring the `cli_engineer` application. Configuration is primarily handled through the `cli_engineer.toml` file, with its structure defined in `src/config.rs`.

## Main Configuration (`cli_engineer.toml`)

The `cli_engineer.toml` file is the primary way to customize the agent's behavior, including selecting LLM providers, setting execution parameters, and controlling the UI.

API keys are not stored in this file. They must be set as environment variables:
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `GEMINI_API_KEY`
- `OPENROUTER_API_KEY`

### Key Sections

#### `[execution]`
Controls the agent's core loop and file management.
```toml
[execution]
max_iterations = 8
disable_auto_git = true
parallel_enabled = true
```
- `max_iterations`: The maximum number of plan-execute-review cycles before stopping.
- `disable_auto_git`: Prevents the agent from automatically initializing a git repository.
- `parallel_enabled`: Allows for concurrent execution of tasks (not fully implemented).

#### `[ui]`
Customizes the user interface experience.
```toml
[ui]
colorful = true
progress_bars = true
metrics = true
output_format = "terminal"
```
- `output_format`: Determines the UI style. `"terminal"` enables the dashboard.

#### `[context]`
Manages the context window for the LLM.
```toml
[context]
max_tokens = 100000
compression_threshold = 0.6
cache_enabled = true
```
- `max_tokens`: The maximum number of tokens to hold in context.
- `compression_threshold`: The usage percentage (e.g., 0.6 for 60%) at which the context will be compressed to save space.

#### `[ai_providers]`
This is where you select and configure your desired Large Language Model. To use a provider, you must set `enabled = true` for it. Only one provider can be enabled at a time.

**Example: Enabling Gemini**
```toml
[ai_providers.gemini]
enabled = true
temperature = 0.2
model = "gemini-2.5-pro-preview-06-05"
cost_per_1m_input_tokens = 1.25
cost_per_1m_output_tokens = 10.00
max_tokens = 1047576
```

## Internal Configuration Structure (`src/config.rs`)

The settings in `cli_engineer.toml` are deserialized into Rust structs defined in `src/config.rs`. This provides type safety and a clear structure within the application code.

The main struct is `Config`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai_providers: AIProvidersConfig,
    pub execution: ExecutionConfig,
    pub ui: UIConfig,
    pub context: ContextConfig,
}
```

Each section of the TOML file maps to a corresponding struct, for example:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub model: String,
    pub temperature: Option<f32>,
    pub cost_per_1m_input_tokens: Option<f32>,
    pub cost_per_1m_output_tokens: Option<f32>,
    pub max_tokens: Option<usize>,
}
```

## Test Configuration (`test_config.toml`)

A separate `test_config.toml` file is used for running automated tests. It provides a minimal, predictable configuration to ensure tests run consistently. For example, it sets a low `max_iterations` to prevent long-running tests.

```toml
[execution]
max_iterations = 3

[ai_providers.ollama]
enabled = true
model = "qwen3:4b"
```

## Project Dependencies (`Cargo.toml`)

All external Rust libraries (crates) used by `cli_engineer` are managed in `Cargo.toml`. Key dependencies include:

| Crate         | Purpose                                       |
|---------------|-----------------------------------------------|
| `tokio`       | Asynchronous runtime for concurrent operations. |
| `clap`        | Command-line argument parsing.                |
| `serde`       | Serialization and deserialization (for TOML). |
| `anyhow`      | Flexible error handling.                      |
| `reqwest`     | HTTP client for making API calls to LLMs.     |
| `log`         | A logging facade for application events.      |
| `simplelog`   | A simple implementation for the `log` facade. |
| `ollama-rs`   | Client for the Ollama local LLM provider.     |
| `indicatif`   | Progress bars for the UI.                     |
| `colored`     | Terminal colorization for the UI.             |
| `crossterm`   | Terminal manipulation for the dashboard UI.   |
| `uuid`        | Generating unique identifiers.                |
| `chrono`      | Date and time handling.                       |

This setup allows for a highly configurable and extensible agent, capable of adapting to different tasks and environments.