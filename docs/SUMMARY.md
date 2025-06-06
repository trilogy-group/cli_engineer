# CLI Engineer Documentation Index

This document serves as the complete navigation guide for the CLI Engineer codebase documentation.

## Table of Contents

### Core Documentation
- [Architecture Overview](architecture.md) - System design and component relationships
- [API Reference](api-reference.md) - Complete API documentation for all modules
- [User Guide](user-guide.md) - Installation, configuration, and usage instructions
- [Developer Guide](developer-guide.md) - Development setup, contributing guidelines, and code standards
- [Configuration Reference](configuration.md) - Complete configuration options and examples

## Documentation Navigation

### Getting Started
1. **New Users**: Start with [User Guide](user-guide.md) for installation and basic usage
2. **Developers**: Begin with [Developer Guide](developer-guide.md) for setup and contribution workflow
3. **System Administrators**: Review [Configuration Reference](configuration.md) for deployment options

### Architecture & Design
- **System Overview**: [Architecture Overview](architecture.md) provides the high-level system design
- **Component Details**: [API Reference](api-reference.md) contains detailed module documentation
- **Event System**: Event-driven architecture explained in the Architecture Overview
- **Provider Pattern**: LLM provider abstraction detailed in API Reference

### Core Components Documentation

#### LLM Management
- **LLMManager**: Central LLM provider coordination
- **Providers**: OpenAI, Anthropic, OpenRouter implementations
- **Context Management**: Token-aware conversation handling

#### Task Execution
- **AgenticLoop**: Main execution orchestrator
- **Planner**: Task breakdown and step generation
- **Executor**: Step execution and artifact creation
- **Reviewer**: Quality assessment and issue detection

#### User Interface
- **DashboardUI**: Real-time status dashboard
- **EnhancedUI**: Progress bars and colored output
- **EventBus**: Component communication system

#### Artifact Management
- **ArtifactManager**: File creation and organization
- **ContextManager**: Conversation state and compression
- **Configuration**: TOML-based system configuration

## Cross-References

### Related Components
- **Event System** connects all components (see Event Bus in API Reference)
- **Configuration** affects all modules (see Configuration Reference)
- **LLM Providers** used by Planner, Executor, and Reviewer
- **Artifact Management** integrated with Executor and ContextManager

### External Dependencies
- **Tokio**: Async runtime for all async operations
- **Clap**: Command-line argument parsing
- **Serde**: Serialization for configuration and artifacts
- **Reqwest**: HTTP client for LLM API calls
- **Crossterm**: Terminal UI manipulation

## Development Workflow
1. Read [Developer Guide](developer-guide.md) for setup
2. Review [Architecture Overview](architecture.md) for system understanding
3. Use [API Reference](api-reference.md) for implementation details
4. Follow [Configuration Reference](configuration.md) for testing setups

## Troubleshooting Quick Links
- **Configuration Issues**: See Configuration Reference troubleshooting section
- **API Errors**: Check API Reference for provider-specific error handling
- **Build Problems**: Refer to Developer Guide build section
- **Usage Questions**: User Guide FAQ section

## Documentation Standards
All documentation follows these principles:
- **Comprehensive**: Covers all features and use cases
- **Practical**: Includes working examples and code snippets
- **Current**: Reflects the latest codebase state
- **Accessible**: Written for both technical and non-technical users

## Contributing to Documentation
See the [Developer Guide](developer-guide.md) for:
- Documentation style guidelines
- How to add new documentation
- Review process for documentation changes
- Tools and workflows for doc maintenance

---

**Note**: This documentation covers CLI Engineer v0.6.0. For the latest updates, check the project repository at https://github.com/trilogy-group/cli_engineer