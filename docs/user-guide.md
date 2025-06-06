# CLI Engineer User Guide

Welcome to CLI Engineer, an autonomous software engineering agent that can interpret tasks, create plans, execute code generation, and review results through an iterative agentic loop.

## Table of Contents

1. [Installation](#installation)
2. [Configuration](#configuration)
3. [Usage](#usage)
4. [Command Types](#command-types)
5. [User Interface Options](#user-interface-options)
6. [Examples and Workflows](#examples-and-workflows)
7. [Advanced Configuration](#advanced-configuration)
8. [Troubleshooting](#troubleshooting)

## Installation

### Prerequisites

- Rust 1.70 or later
- An active internet connection
- API keys for at least one supported LLM provider

### Install from Crates.io

The easiest way to install CLI Engineer is from crates.io:

```bash
cargo install cli_engineer
```

### Install from Source

For the latest development version:

```bash
git clone https://github.com/trilogy-group/cli_engineer
cd cli_engineer
cargo build --release
cargo install --path .
```

### Verify Installation

```bash
cli_engineer --help
```

## Configuration

CLI Engineer uses a TOML configuration file to manage settings. The tool looks for configuration files in the following order:

1. `cli_engineer.toml` (current directory)
2. `.cli_engineer.toml` (current directory)
3. `~/.config/cli_engineer/config.toml` (user config directory)

### API Key Setup

Set up environment variables for your chosen LLM provider:

```bash
# For OpenAI
export OPENAI_API_KEY="your-api-key-here"

# For Anthropic
export ANTHROPIC_API_KEY="your-api-key-here"

# For OpenRouter
export OPENROUTER_API_KEY="your-api-key-here"
```

Add these to your shell profile (`.bashrc`, `.zshrc`, etc.) to persist them.

### Basic Configuration File

Create a `cli_engineer.toml` file:

```toml
[ai_providers.anthropic]
enabled = true
model = "claude-3-sonnet-20240229"
temperature = 0.7
cost_per_1m_input_tokens = 3.0
cost_per_1m_output_tokens = 15.0
max_tokens = 200000

[ai_providers.openai]
enabled = false
model = "gpt-4"
temperature = 0.7
cost_per_1m_input_tokens = 2.0
cost_per_1m_output_tokens = 8.0
max_tokens = 128000

[execution]
max_iterations = 10
parallel_enabled = false
artifact_dir = "./artifacts"
cleanup_on_exit = false
disable_auto_git = true

[ui]
colorful = true
progress_bars = true
metrics = true
output_format = "terminal"

[context]
max_tokens = 100000
compression_threshold = 0.8
cache_enabled = true
```

## Usage

### Basic Syntax

```bash
cli_engineer [OPTIONS] <COMMAND> [PROMPT...]
```

### Options

- `-v, --verbose`: Enable verbose logging
- `-d, --dashboard`: Use dashboard UI (compact, non-scrolling display)
- `-c, --config <FILE>`: Specify custom configuration file path

## Command Types

### 1. Code Generation

Generate new code from scratch:

```bash
cli_engineer code "create a Python script that implements FizzBuzz"
cli_engineer code "build a REST API server in Rust using axum"
```

### 2. Refactoring

Analyze and refactor existing code:

```bash
cli_engineer refactor "improve error handling in the current codebase"
cli_engineer refactor  # Analyzes current directory automatically
```

### 3. Code Review

Perform comprehensive code reviews:

```bash
cli_engineer review "focus on security vulnerabilities and performance"
cli_engineer review  # General code review of current directory
```

### 4. Documentation

Generate project documentation:

```bash
cli_engineer docs "create API documentation for the web server"
cli_engineer docs  # Generate comprehensive project documentation
```

### 5. Security Analysis

Perform security audits:

```bash
cli_engineer security "analyze for SQL injection vulnerabilities"
cli_engineer security  # General security analysis
```

## User Interface Options

### Standard Terminal UI

Default interface with colored output and progress indicators:

```bash
cli_engineer -v code "create a calculator app"
```

### Dashboard UI

Compact, non-scrolling interface ideal for monitoring:

```bash
cli_engineer -v -d code "create a web scraper"
```

Features:
- Real-time metrics (API calls, costs, artifacts)
- Live log display
- Progress tracking
- Clean, box-bordered layout

## Examples and Workflows

### Example 1: Create a New Project

```bash
# Create a Python web application
cli_engineer code "create a Flask web application with user authentication, database models, and REST API endpoints for a todo list application"
```

### Example 2: Analyze Existing Code

```bash
# Navigate to your project directory
cd my-project

# Perform code review
cli_engineer review "focus on code quality, performance, and maintainability"
```

### Example 3: Generate Documentation

```bash
# Generate comprehensive docs
cli_engineer docs "create user guides, API documentation, and developer setup instructions"
```

### Example 4: Security Audit

```bash
# Security analysis
cli_engineer security "check for common vulnerabilities: SQL injection, XSS, authentication issues, and insecure dependencies"
```

### Example 5: Refactoring Session

```bash
# Refactor for better patterns
cli_engineer refactor "apply SOLID principles, improve error handling, and optimize database queries"
```

## Advanced Configuration

### AI Provider Settings

#### Temperature Control
- Lower values (0.1-0.4): More deterministic, conservative code
- Higher values (0.7-1.0): More creative, varied solutions

#### Model Selection
- **GPT-4**: Excellent for complex reasoning and code quality
- **Claude 3**: Strong at following instructions and safe code generation
- **DeepSeek**: Cost-effective option via OpenRouter

#### Cost Management

Track costs by configuring token pricing:

```toml
[ai_providers.openai]
cost_per_1m_input_tokens = 2.0
cost_per_1m_output_tokens = 8.0
```

### Execution Settings

#### Iteration Limits
```toml
[execution]
max_iterations = 15  # Allow more iterations for complex tasks
```

#### Artifact Management
```toml
[execution]
artifact_dir = "./output"  # Custom output directory
cleanup_on_exit = true     # Auto-cleanup generated files
```

#### Git Integration
```toml
[execution]
disable_auto_git = false  # Enable automatic git repo initialization
```

### Context Management

#### Memory Optimization
```toml
[context]
max_tokens = 200000           # Increase for larger codebases
compression_threshold = 0.6   # Compress earlier to save costs
cache_enabled = true          # Enable context caching
```

### UI Customization

```toml
[ui]
colorful = false         # Disable colors for CI/CD
progress_bars = false    # Minimal output
output_format = "json"   # Machine-readable output
```

## Troubleshooting

### Common Issues

#### 1. API Key Not Found
```
Error: OPENAI_API_KEY environment variable not set
```

**Solution**: Set the appropriate environment variable:
```bash
export OPENAI_API_KEY="your-key-here"
```

#### 2. No Providers Configured
```
Error: No AI providers configured, using LocalProvider
```

**Solution**: Enable at least one provider in your config file and ensure the API key is set.

#### 3. Permission Denied (Artifact Directory)
```
Error: Failed to create artifact directory
```

**Solution**: Check directory permissions or specify a different artifact directory:
```toml
[execution]
artifact_dir = "~/cli_engineer_output"
```

#### 4. Context Too Large
```
Warning: Context usage at 95%, compression will be triggered
```

**Solution**: Reduce compression threshold or increase max tokens:
```toml
[context]
compression_threshold = 0.7
max_tokens = 150000
```

### Performance Tips

1. **Use Dashboard UI** for long-running tasks to monitor progress
2. **Enable context caching** to speed up repeated operations
3. **Set appropriate iteration limits** to prevent runaway costs
4. **Use temperature < 0.5** for deterministic, production-ready code
5. **Monitor token usage** in verbose mode to optimize prompts

### Getting Help

- Check logs with `-v` flag for detailed information
- Review configuration file syntax
- Ensure API keys have sufficient quota
- Verify network connectivity for API calls

### Best Practices

1. **Start Small**: Begin with simple tasks to understand the tool's capabilities
2. **Iterate**: Use the review and refactor commands to improve generated code
3. **Monitor Costs**: Keep track of API usage, especially with premium models
4. **Version Control**: Always use version control when working with existing projects
5. **Review Output**: Always review generated code before deploying to production

## Support and Community

- **GitHub Repository**: https://github.com/trilogy-group/cli_engineer
- **Issue Tracker**: Report bugs and request features on GitHub
- **Documentation**: Additional docs available in the repository

CLI Engineer is designed to augment your development workflow, not replace human judgment. Always review and test generated code before using it in production environments.