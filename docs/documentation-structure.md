# Documentation Structure

This document outlines the structure and organization of the `cli_engineer` documentation. The goal is to provide a clear and consistent guide for users, developers, and contributors.

The documentation is organized into the following key files within the `docs/` directory, each catering to a specific audience:

- **[user-guide.md](./user-guide.md)**: A comprehensive guide for end-users. It covers installation, configuration of `cli_engineer.toml`, setting up API keys, and detailed usage of all available commands (`code`, `review`, `docs`, `security`, `refactor`) with examples.

- **[architecture.md](./architecture.md)**: A deep dive into the technical architecture of `cli_engineer`. This document explains the core components (Interpreter, Planner, Executor, Reviewer), the agentic loop, context management, and how they interact. It's intended for developers who want to understand the system's design principles.

- **[development.md](./development.md)**: Instructions for setting up a development environment, building the project, running tests, and contributing to the project. This is the primary resource for new contributors.

- **[api-reference.md](./api-reference.md)**: Detailed documentation of the internal Rust API, including public modules, structs, and functions. This is intended to be generated from source code comments (`cargo doc`).

- **[documentation-structure.md](./documentation-structure.md)**: This file, which describes the organization of the documentation itself.

This structure ensures that information is logically grouped and easy to find for different audiences, from casual users to core developers.