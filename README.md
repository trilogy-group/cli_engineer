# CLI Engineer

Agentic CLI for software engineering automation. It features pluggable LLM providers, task interpretation, planning, execution, review and an agentic loop.

## Quick Start

### Installation

```bash
cargo install cli_engineer
cli_engineer --help
```

### Basic Usage

CLI Engineer provides specialized commands for different types of software engineering tasks:

```bash
# Code generation
cli_engineer code "create a full-stack task management app with React frontend, Node.js/Express backend, PostgreSQL database, user authentication, and REST API"

# Code review and analysis
cli_engineer review "focus on error handling and performance"

# Documentation generation
cli_engineer docs

# Security analysis
cli_engineer security

# Refactoring assistance
cli_engineer refactor "improve code organization and performance"
```

### UI Options

**Dashboard UI (Default)** - Interactive real-time interface with metrics and progress tracking:
```bash
# Dashboard with live metrics (default behavior)
cli_engineer docs "document the architecture"

# Dashboard + File Logging - logs appear in UI AND saved to file
cli_engineer -v security "analyze code security"
# Creates: cli_engineer_YYYYMMDD_HHMMSS.log with all session details
```

**Simple Text Mode** - Traditional command-line output:
```bash
# Simple text output (disable dashboard)
cli_engineer --no-dashboard docs "document the architecture"

# Simple text + File logging
cli_engineer --no-dashboard -v review "analyze code quality"
```

**Key Features:**
- 🎛️ **Dashboard UI**: Default experience with real-time metrics, progress bars, and live log display
- 📄 **Simple Text**: Clean terminal output for scripts or minimal environments
- 📝 **File Logging**: Verbose mode (`-v`) automatically creates timestamped session logs
- 🔄 **Dual Output**: Dashboard mode with verbose shows logs in UI AND saves to file simultaneously

## Documentation

 **For comprehensive guides and detailed information, see the [docs/](./docs/) directory:**

- **[User Guide](./docs/user-guide.md)** - Complete installation, configuration, and usage instructions
- **[Architecture](./docs/architecture.md)** - Technical architecture and design decisions  
- **[API Reference](./docs/api-reference.md)** - Detailed API documentation
- **[Development Guide](./docs/development.md)** - Contributing and development setup
- **[Documentation Structure](./docs/documentation-structure.md)** - Guide to the documentation organization

## Command Reference

| Command | Purpose | Output |
|---------|---------|--------|
| `code` | Generate new code | Source files in current directory |
| `review` | Analyze existing code | `code_review.md` with findings |
| `docs` | Generate documentation | Documentation files in `docs/` |
| `security` | Security analysis | `security_report.md` with vulnerabilities |
| `refactor` | Code improvement | Refactored source files |

## Developer Setup

```bash
git clone https://github.com/trilogy-group/cli_engineer
cd cli_engineer
cargo build
cargo test
```

## Features

- 🤖 **4 LLM Providers**: OpenAI (Responses API), Anthropic (Claude 4), Google Gemini, Ollama (local)
- 🧠 **Real-Time Thinking**: Live reasoning traces from Claude 4, Gemini, and local models; reasoning summaries from o1/o3/o4-mini
- 📊 **Dashboard UI (Default)**: Interactive interface with streaming thoughts and cost tracking
- 📝 **Smart Buffering**: Intelligent chunking at sentence boundaries for smooth reasoning display
- 💰 **Accurate Costs**: Real-time token usage and cost calculation from streaming events
- 🔄 **Agentic Loop**: Iterative planning, execution, and review with transparent reasoning
- 🔒 **Local Option**: Ollama support for privacy-focused, offline LLM inference (no API keys)
- 📁 **Smart Artifacts**: Context-aware file generation with proper restrictions
- 🔒 **Command-Specific Behavior**: Different file permissions and outputs per command type
- 📖 **Comprehensive Documentation**: Auto-generated docs with examples and API references

---

See the [User Guide](./docs/user-guide.md) for detailed setup and advanced usage instructions.
