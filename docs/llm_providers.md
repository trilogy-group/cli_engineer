# LLM Provider Guide

`cli_engineer` features a modular architecture that allows for seamless integration with various Large Language Model (LLM) providers. This design enables users to choose the best model for their needs, balancing cost, performance, privacy, and specific features like real-time reasoning.

## Core Concepts

The LLM provider system is built around two key components: `LLMManager` and the `LLMProvider` trait.

### `LLMManager` (`src/llm_manager.rs`)

The `LLMManager` is the central hub for all LLM interactions. Its primary responsibilities are:

-   **Provider Selection**: It reads the `cli_engineer.toml` configuration to determine which LLM provider is enabled.
-   **Prompt Dispatching**: It takes a prompt from other parts of the application (like the `Planner` or `Executor`) and sends it to the active provider.
-   **Event Emission**: It emits events for API calls (`APICallStarted`, `APICallCompleted`), which are used by the UI to display metrics like cost and token usage.
-   **Cost Calculation**: For providers that don't report token usage directly, it provides a fallback mechanism to estimate costs.

### `LLMProvider` Trait (`src/llm_manager.rs`)

Every supported LLM provider implements the `LLMProvider` trait. This ensures a consistent interface for the `LLMManager` to work with. The trait defines essential methods:

-   `name()`: Returns the provider's name (e.g., "OpenAI").
-   `context_size()`: Returns the maximum context window size in tokens for the selected model.
-   `send_prompt()`: The core method that sends a prompt to the provider's API and returns the response.
-   `handles_own_metrics()`: A boolean indicating if the provider handles its own cost and token tracking (e.g., via streaming events).

## Supported Providers

You can configure which provider to use in your `cli_engineer.toml` file. Only one provider should be enabled at a time.

---

### OpenAI

-   **Source**: `src/providers/openai.rs`
-   **API Key**: `OPENAI_API_KEY` environment variable.
-   **Features**: Supports reasoning models (`o1`, `o3`, `o4-mini`) that provide a summary of their thought process after generating a response.

**Configuration (`cli_engineer.toml`):**

```toml
[ai_providers.openai]
enabled = true
temperature = 1
model = "gpt-4.1"
cost_per_1m_input_tokens = 2.00
cost_per_1m_output_tokens = 8.00
```

---

### Anthropic (Claude)

-   **Source**: `src/providers/anthropic.rs`
-   **API Key**: `ANTHROPIC_API_KEY` environment variable.
-   **Features**: Supports real-time, streaming "thinking" traces for Sonnet and Opus models, allowing you to see the model's reasoning as it works.

**Configuration (`cli_engineer.toml`):**

```toml
[ai_providers.anthropic]
enabled = true
temperature = 1
model = "claude-sonnet-4-0"
cost_per_1m_input_tokens = 3.00
cost_per_1m_output_tokens = 15.00
```

---

### Google Gemini

-   **Source**: `src/providers/gemini.rs`
-   **API Key**: `GEMINI_API_KEY` environment variable.
-   **Features**: Supports real-time, streaming "thinking" traces, similar to Anthropic's Claude models.

**Configuration (`cli_engineer.toml`):**

```toml
[ai_providers.gemini]
enabled = true
temperature = 0.2
model = "gemini-2.5-pro-preview-06-05"
cost_per_1m_input_tokens = 1.25
cost_per_1m_output_tokens = 10.00
```

---

### Ollama (Local Models)

-   **Source**: `src/providers/ollama.rs`
-   **API Key**: Not required.
-   **Features**: Allows you to run LLMs locally on your own hardware for maximum privacy and offline use. It requires the [Ollama](https://ollama.ai/) server to be running. It supports streaming reasoning traces for models that use `<think>` tags.

**Configuration (`cli_engineer.toml`):**

```toml
[ai_providers.ollama]
enabled = true
temperature = 0.7
base_url = "http://localhost:11434"
model = "qwen3:8b"
max_tokens = 128000
```

---

### OpenRouter

-   **Source**: `src/providers/openrouter.rs`
-   **API Key**: `OPENROUTER_API_KEY` environment variable.
-   **Features**: Acts as a gateway to a wide variety of models from different providers, often at a lower cost. This is a great way to experiment with different models without managing multiple API keys.

**Configuration (`cli_engineer.toml`):**

```toml
[ai_providers.openrouter]
enabled = true
temperature = 1
model = "google/gemini-2.5-pro-preview"
cost_per_1m_input_tokens = 1.25
cost_per_1m_output_tokens = 10.00
```

## Adding a New Provider

The modular design makes it straightforward to add support for a new LLM provider. The general steps are:

1.  **Create Provider Module**: Add a new file in `src/providers/`, for example, `src/providers/new_provider.rs`.
2.  **Implement `LLMProvider`**: In the new module, create a struct for your provider and implement the `LLMProvider` trait for it. This will involve handling API requests and parsing responses specific to that provider.
3.  **Update Configuration**: Add a new configuration struct in `src/config.rs` and corresponding entries in the default `cli_engineer.toml`.
4.  **Register Provider**: In `src/main.rs`, update the `setup_managers` function to initialize and register your new provider based on its configuration.