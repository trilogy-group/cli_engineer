# cli_engineer

An autonomous CLI coding agent written in Rust. It features pluggable LLM providers, task interpretation, planning, execution, review and an agentic loop.

## Developer Setup

Clone the repository and build it locally:

```bash
cargo build
cargo test
```

Run the agent directly with:

```bash
cargo run -- --verbose "generate hello world"
```


## User Installation

The tool can be installed from crates.io with:

```bash
cargo install cli_engineer
cli_engineer --help
```

Use `--headless` to disable UI output.
