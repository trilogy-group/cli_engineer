# cli_engineer

A skeletal implementation of an autonomous CLI coding agent written in Rust. It demonstrates the overall architecture with pluggable LLM providers, task interpretation, planning, execution, review and an agentic loop.

## Building

```bash
cargo build
```

(Requires network access on first build to download dependencies.)

## Usage

```bash
cargo run -- --verbose "generate hello world"
```

Use `--headless` to disable UI output.

### Environment variables

Select a provider by setting `LLM_PROVIDER` to one of `openai`, `anthropic`, `openrouter`, `xai`, or `ollama`. The `LLM_MODEL` variable chooses the model name. Each provider requires an API key in the corresponding variable (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENROUTER_API_KEY`, `XAI_API_KEY`).
