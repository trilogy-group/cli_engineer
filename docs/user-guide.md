# CLI Engineer User Guide

Welcome to CLI Engineer, an autonomous software engineering agent that can interpret tasks, create plans, execute code generation, and review results through an iterative agentic loop.

## Table of Contents

1. [Installation](#installation)
2. [Configuration](#configuration)
3. [Usage](#usage)
4. [Command Types](#command-types)
5. [User Interface Options](#user-interface-options)
6. [Logging and Session Management](#logging-and-session-management)
7. [Examples and Workflows](#examples-and-workflows)
8. [Advanced Configuration](#advanced-configuration)
9. [Troubleshooting](#troubleshooting)

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

- `-v, --verbose`: Enable verbose logging (creates timestamped log files)
- `--no-dashboard`: Use simple text UI instead of dashboard (opt-out of default dashboard)
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
cli_engineer review "focus on code quality and performance"
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
cli_engineer -v security "analyze for SQL injection vulnerabilities"
cli_engineer -v security  # General security analysis
```

## User Interface Options

CLI Engineer offers two interface modes to suit different use cases and preferences.

### Dashboard UI (Default)

The dashboard provides a rich, interactive interface with real-time metrics and progress tracking. This is now the default experience:

```bash
# Dashboard UI (default - no flags needed)
cli_engineer code "create a calculator app"

# Dashboard UI with verbose logging (dual output)
cli_engineer -v security "analyze for SQL injection vulnerabilities"
# Or use general security analysis: cli_engineer -v security
# Logs appear in dashboard AND saved to cli_engineer_20240605_143022.log
```

**Dashboard Features:**
- **Real-time Metrics**: API calls, costs, artifacts created, context usage
- **Live Log Display**: Streaming log output with color coding
- **Progress Tracking**: Phase progression and completion status
- **Compact Layout**: Clean, box-bordered interface that doesn't scroll
- **Dual Output**: With `-v` flag, logs appear in UI AND get saved to timestamped files

### Simple Text Mode

Traditional command-line interface for scripts, CI/CD, or minimal environments:

```bash
# Simple text interface (opt-out of dashboard)
cli_engineer --no-dashboard code "create a web scraper"

# Simple text with file logging
cli_engineer --no-dashboard -v review "analyze code quality"
# Creates timestamped log file: cli_engineer_YYYYMMDD_HHMMSS.log
```

**Simple Text Features:**
- **Traditional Output**: Standard terminal output with colors and progress bars
- **Scrolling Logs**: Full log history remains visible in terminal
- **CI/CD Friendly**: Works well in automated environments
- **File Logging**: Verbose mode creates detailed session logs

### Choosing the Right Interface

**Use Dashboard UI when:**
- Working interactively with real-time feedback
- Monitoring resource usage (API costs, context limits)
- Want a clean, organized view of progress
- Need both screen display and file logging

**Use Simple Text when:**
- Running in CI/CD pipelines or scripts
- Working in environments with limited terminal capabilities
- Prefer traditional command-line experience
- Need full scrollback history in terminal

## Logging and Session Management

CLI Engineer provides comprehensive logging capabilities to help you track sessions, debug issues, and maintain records of your work.

### Verbose Mode File Logging

The `-v` flag enables detailed logging with automatic file creation:

```bash
# Creates timestamped log file with all session details
cli_engineer -v code "create a web application that shows the current weather in a specified city"
# Output file: cli_engineer_20250605_195432.log
```

**Log File Features:**
- **Timestamped Filenames**: `cli_engineer_YYYYMMDD_HHMMSS.log`
- **Session Headers**: Clear start time and session information
- **Detailed Logging**: All API calls, responses, decisions, and file operations
- **Structured Format**: Timestamped entries with log levels

### Dual Output Mode

With dashboard UI + verbose mode, logs appear in both places simultaneously:

```bash
# Best of both worlds: real-time dashboard + persistent file logging
cli_engineer -v security "analyze authentication mechanisms"
```

**Benefits:**
- **Live Monitoring**: See progress and logs in real-time dashboard
- **Persistent Records**: Complete session saved to timestamped file
- **No Trade-offs**: Full logging functionality in both outputs
- **Enhanced Debugging**: Dashboard for immediate feedback, files for detailed analysis

### Log File Contents

Verbose log files contain comprehensive session information:

```
=== CLI Engineer Session Started: 2025-06-05 19:54:32 UTC ===

19:54:32 [INFO] Verbose logging enabled. Session details will be logged to: cli_engineer_20250605_195432.log
19:54:32 [INFO] Added src/main.rs to context (20194 bytes)
19:54:33 [INFO] Starting agentic loop for input: analyze authentication mechanisms
19:54:33 [INFO] Creating plan for task...
19:54:34 [INFO] Plan created with 3 steps, complexity: Moderate
```

### Best Practices

**For Development Work:**
```bash
# Use dashboard + file logging for interactive development
cli_engineer -v code "create a web application that shows the current weather in a specified city"
```

**For CI/CD Integration:**
```bash
# Use simple text + file logging for automated environments
cli_engineer --no-dashboard -v docs "generate API documentation"
```

**For Quick Tasks:**
```bash
# Use dashboard only for quick, non-critical tasks
cli_engineer review "quick code review"
```

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
cli_engineer -v security "check for insecure dependencies"
cli_engineer -v security  # General security analysis
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