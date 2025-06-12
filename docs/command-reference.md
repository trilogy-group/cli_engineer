# Command Reference

This document provides a detailed reference for all `cli_engineer` commands and options.

## General Usage

The basic structure of a `cli_engineer` command is:

```bash
cli_engineer [OPTIONS] <COMMAND> -- [PROMPT]
```

- `[OPTIONS]`: Global flags that modify the tool's behavior (e.g., `--verbose`).
- `<COMMAND>`: The specific engineering task to perform (e.g., `code`, `review`).
- `[PROMPT]`: A natural language description of the task. This is required for the `code` command and optional for others.

## Global Options

These options can be used with any command.

| Option                | Short | Description                                         |
|-----------------------|-------|-----------------------------------------------------|
| `--verbose`           | `-v`  | Enables detailed, verbose logging to the console and a log file. |
| `--no-dashboard`      |       | Disables the interactive dashboard UI, using simple text output instead. |
| `--config <PATH>`     | `-c`  | Specifies the path to a custom configuration file.  |
| `--help`              | `-h`  | Displays the help message.                          |

## Commands

### `code`

Generates new code based on a prompt. This is the primary command for creating new applications or features from scratch.

**Usage:**
```bash
cli_engineer code "<your detailed prompt here>"
```

**Example:**
```bash
cli_engineer code "create a simple command-line calculator in Python that can add, subtract, multiply, and divide"
```

### `refactor`

Analyzes and refactors existing code in the current directory. If a prompt is provided, it will focus the refactoring efforts on specific goals.

**Usage:**
```bash
# General refactoring
cli_engineer refactor

# Focused refactoring
cli_engineer refactor "improve performance and reduce code duplication in the data processing module"
```

### `review`

Performs a comprehensive review of the existing codebase and generates a `code_review.md` report. It does not modify any code.

**Usage:**
```bash
# General code review
cli_engineer review

# Focused code review
cli_engineer review "focus on error handling and adherence to DRY principles"
```

### `docs`

Generates documentation for the existing codebase. It creates markdown files inside a `docs/` directory.

**Usage:**
```bash
# Generate comprehensive documentation
cli_engineer docs

# Generate documentation with specific instructions
cli_engineer docs "create a user guide and an API reference for the public-facing functions"
```

### `security`

Performs a security analysis of the existing codebase and generates a `security_report.md` with findings and recommendations. It does not modify any code.

**Usage:**
```bash
# General security analysis
cli_engineer security

# Focused security analysis
cli_engineer security "check for potential SQL injection vulnerabilities and insecure API endpoints"
```