# Contributing to CLI Engineer

First off, thank you for considering contributing to CLI Engineer! We welcome any help, from bug reports and feature requests to code contributions. This document provides guidelines to help you get started.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- [Git](https://git-scm.com/)

### Development Setup

The setup process is straightforward. You can get the project up and running with a few commands:

```bash
# 1. Clone the repository
git clone https://github.com/trilogy-group/cli_engineer.git

# 2. Navigate to the project directory
cd cli_engineer

# 3. Build the project
cargo build

# 4. Run the tests to ensure everything is working
cargo test
```

## How to Contribute

We recommend following this process for contributions:

1.  **Fork the repository** on GitHub.
2.  **Create a new branch** for your feature or bugfix. Use a descriptive name, like `feat/new-provider` or `fix/ui-rendering-bug`.
    ```bash
    git checkout -b feat/your-awesome-feature
    ```
3.  **Make your changes.** Write clean, readable, and well-documented code.
4.  **Format and lint your code.** We use standard Rust tooling to maintain code quality.
    ```bash
    # Auto-format the code
    cargo fmt

    # Run the linter to catch common issues
    cargo clippy -- -D warnings
    ```
5.  **Ensure all tests pass** before submitting your changes.
    ```bash
    cargo test
    ```
6.  **Commit your changes** with a clear and concise commit message.
7.  **Push your branch** to your forked repository.
8.  **Open a Pull Request (PR)** to the `main` branch of the original `cli_engineer` repository.
    - Provide a clear title and description for your PR.
    - Explain the "why" and "what" of your changes.
    - If your PR addresses an existing issue, link to it (e.g., `Fixes #123`).

## Coding Standards

- **Follow Rust best practices:** Adhere to the guidelines in [The Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
- **Modularity:** The project is structured into modules with specific responsibilities (e.g., `planner`, `executor`, `providers`). New functionality should follow this pattern.
- **Asynchronous Code:** We use `tokio` for asynchronous operations. Ensure your async code is efficient and non-blocking.
- **Error Handling:** Use `anyhow::Result` for functions that can fail, providing context to errors.
- **Comments:** Add comments to explain complex logic, design decisions, or anything that isn't immediately obvious from the code itself.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct. We are committed to providing a welcoming and inclusive environment for everyone. Please be respectful and considerate in all interactions.

Thank you for your contribution!