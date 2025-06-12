# Quick Start Guide

This guide provides the most direct path for a new user to get `cli_engineer` up and running.

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust and Cargo**: `cli_engineer` is built with Rust. You can install the toolchain from the official website: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

## Installation

You can install `cli_engineer` directly from crates.io using Cargo:

```bash
cargo install cli_engineer
```

After installation, verify it's working by checking the help message:

```bash
cli_engineer --help
```

## Configuration

`cli_engineer` requires configuration for an AI provider to function.

1.  **Create a Configuration File**: Create a file named `cli_engineer.toml` in your project directory. You can start with this minimal example for OpenAI:

    ```toml
    # cli_engineer.toml

    [execution]
    max_iterations = 8

    [ui]
    colorful = true

    # Enable the provider you want to use.
    [ai_providers.openai]
    enabled = true
    model = "gpt-4o-mini" # A fast and affordable model
    temperature = 0.7
    cost_per_1m_input_tokens = 0.15
    cost_per_1m_output_tokens = 0.60
    max_tokens = 128000

    # Disable other providers
    [ai_providers.anthropic]
    enabled = false

    [ai_providers.openrouter]
    enabled = false

    [ai_providers.gemini]
    enabled = false

    [ai_providers.ollama]
    enabled = false
    ```

2.  **Set Your API Key**: `cli_engineer` reads API keys from environment variables. For the configuration above, set your OpenAI key:

    ```bash
    export OPENAI_API_KEY="your-openai-api-key-here"
    ```

    For other providers, use the corresponding environment variable (`ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, etc.).

## First Run

Now you're ready to run your first task. Let's ask the agent to generate a simple Python script.

```bash
cli_engineer code "create a python script that implements the fizzbuzz algorithm and writes the output for numbers 1 to 100 to a file named fizzbuzz_output.txt"
```

The agent will now start, display its plan, and execute the steps to create the requested script. You will see the output in your terminal and find the generated files in the `artifacts/` directory.