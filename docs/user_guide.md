# CLI Engineer User Guide

Welcome to the comprehensive user guide for CLI Engineer. This document provides detailed information on installation, configuration, and usage to help you get the most out of the tool.

## Table of Contents
- [Installation](#installation)
- [Configuration](#configuration)
  - [Configuration File](#configuration-file)
  - [API Keys](#api-keys)
- [Command-Line Usage](#command-line-usage)
  - [Command Structure](#command-structure)
  - [Global Options (Flags)](#global-options-flags)
  - [Commands](#commands)
- [Writing Effective Goals](#writing-effective-goals)
  - [General Principles](#general-principles)
  - [Examples by Command](#examples-by-command)
- [Interpreting the Output](#interpreting-the-output)
  - [UI Options](#ui-options)
  - [Generated Artifacts](#generated-artifacts)
  - [Log Files](#log-files)

## Installation

You can install CLI Engineer directly from crates.io using Cargo:

```bash
cargo install cli_engineer
```

After installation, verify it's working by running the help command:

```bash
cli_engineer --help
```

## Configuration

### Configuration File

CLI Engineer is configured via a `cli_engineer.toml` file. The tool searches for this file in the following locations, in order:

1.  A path specified with the `--config` flag.
2.  `cli_engineer.toml` in the current directory.
3.  `.cli_engineer.toml` in the current directory.
4.  `~/.config/cli_engineer/config.toml` in your home directory.

The configuration file allows you to select your preferred AI provider, set model parameters, and customize the agent's behavior.

### API Keys

API keys are not stored in the configuration file for security. They must be set as environment variables:

-   `OPENAI_API_KEY` for OpenAI models.
-   `ANTHROPIC_API_KEY` for Anthropic (Claude) models.
-   `GEMINI_API_KEY` for Google Gemini models.
-   `OPENROUTER_API_KEY` for models accessed via OpenRouter.

Ollama runs locally and does not require an API key.

## Command-Line Usage

### Command Structure

The basic structure for using CLI Engineer is:

```bash
cli_engineer [OPTIONS] <COMMAND> [PROMPT]
```

-   `[OPTIONS]`: Global flags that modify the tool's behavior (e.g., `--verbose`).
-   `<COMMAND>`: The primary task to perform (e.g., `code`, `review`).
-   `[PROMPT]`: A detailed description of your goal for the agent.

### Global Options (Flags)

-   `-v, --verbose`: Enables verbose logging. In dashboard mode, it shows more detailed logs in the UI. In both modes, it creates a timestamped log file (e.g., `cli_engineer_20240729_103000.log`) with a full record of the session.
-   `--no-dashboard`: Disables the default interactive dashboard UI and switches to a simple, clean text output. This is ideal for scripting or use in minimal terminal environments.
-   `-c, --config <PATH>`: Specifies the path to a custom `cli_engineer.toml` configuration file, overriding the default search locations.

### Commands

CLI Engineer provides several specialized commands:

-   `code`: Generates new source code files based on your prompt. Requires a prompt.
-   `review`: Analyzes the existing codebase for quality, bugs, and best practices. Outputs its findings to `code_review.md`. The prompt can be used to specify areas of focus.
-   `docs`: Generates documentation for the existing codebase. Creates or modifies files within the `docs/` directory.
-   `security`: Performs a security analysis on the codebase, checking for common vulnerabilities. Outputs a report to `security_report.md`.
-   `refactor`: Modifies existing code to improve its structure, performance, or readability without changing its external behavior.

## Writing Effective Goals

The quality of the agent's output is highly dependent on the quality of your prompt.

### General Principles

-   **Be Specific:** Instead of "make a website," describe the type of website, its features, and the technology stack.
-   **Provide Context:** Mention key requirements, constraints, or existing code structures the agent should be aware of.
-   **Define the "What," Not the "How":** Describe the desired outcome, and let the agent determine the steps to get there.

### Examples by Command

-   **`code`**
    -   **Good:** `"create a command-line tool in Rust using clap that takes a URL as input, fetches the content, and counts the number of words"`
    -   **Bad:** `"make a rust app"`

-   **`review`**
    -   **Good:** `"review the Python code, focusing on error handling in the API client and potential race conditions in the concurrency module"`
    -   **Bad:** `"is my code good?"`

-   **`docs`**
    -   **Good:** `"generate comprehensive documentation for the project, including an architectural overview, API reference for the public modules, and a setup guide for new developers"`
    -   **Bad:** `"write docs"`

-   **`refactor`**
    -   **Good:** `"refactor the user authentication logic to use a service-based architecture and improve performance by caching user sessions"`
    -   **Bad:** `"clean up the code"`

## Interpreting the Output

### UI Options

-   **Dashboard UI (Default):** An interactive, real-time interface that provides live metrics on cost and token usage, progress bars, and a window for the model's streaming "thoughts" or reasoning process.
-   **Simple Text Mode (`--no-dashboard`):** A clean, traditional command-line output suitable for scripting or minimal environments. It prints status updates sequentially.

### Generated Artifacts

The agent produces different outputs depending on the command:

-   `code`, `refactor`: Modifies or creates source files directly in your project directory.
-   `docs`: Creates or modifies documentation files, typically within a `docs/` subdirectory.
-.  `review`: Generates a detailed `code_review.md` file in your project's root directory.
-   `security`: Generates a detailed `security_report.md` file in your project's root directory.

### Log Files

When you use the `--verbose` (`-v`) flag, a detailed log file named `cli_engineer_YYYYMMDD_HHMMSS.log` is created. This file contains:
-   The full prompt and configuration.
-   The agent's plan.
-   The full, un-truncated output from each step.
-   The final review and summary.

This is invaluable for debugging or understanding the agent's decision-making process.