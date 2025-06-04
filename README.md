# cli_engineer

An experimental autonomous CLI coding agent written in Rust. It features pluggable LLM providers, task interpretation, planning, execution, review and an agentic loop.

## Developer Setup

Clone the repository and build it locally:

```bash
cargo build
cargo test
```

### Example invocations

Run the agent in verbose mode directly with:

```bash
cargo run -- -v "generate a Python hello world script"
```

Run the agent with a CLI Dashboard for live review:

```bash
cargo run -- -v -d "generate a FizzBuzz implementation"
```


## User Installation

The tool can be installed from crates.io with:

```bash
cargo install cli_engineer
cli_engineer --help
```

