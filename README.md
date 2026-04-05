# openai-agents-rust

Rust-native agents runtime with a single ergonomic facade, async-first execution, OpenAI integrations, MCP, realtime sessions, voice workflows, and extension hooks.


This repository is for teams that want to build agent systems in Rust without wrapping another SDK and without giving up typed runtime building blocks.

## Start Here

- New to the library: [docs/index.md](docs/index.md)
- Want a first working program: [docs/quickstart.md](docs/quickstart.md)
- Need runnable code: [docs/examples.md](docs/examples.md)
- Want the public API map: [docs/ref/README.md](docs/ref/README.md)
- Contributing to the workspace: [CONTRIBUTING.md](CONTRIBUTING.md)

## What You Can Build

- one-shot runs with `run` and `run_sync`
- live incremental runs with `run_streamed`
- session-aware conversations with `Runner::run_with_session`
- nested agents as tools
- handoffs, approvals, guardrails, and replayable run results
- OpenAI Responses and Chat Completions integrations
- MCP-backed tool discovery and resource access
- realtime agents with long-lived sessions
- voice workflows and STT -> workflow -> TTS pipelines
- optional integrations through the extensions namespace

## Install

```toml
[dependencies]
openai-agents = { package = "openai-agents-rs", version = "0.1.2" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Smallest Useful Example

```rust
use openai_agents::{run, Agent};

#[tokio::main]
async fn main() -> Result<(), openai_agents::AgentsError> {
    let agent = Agent::builder("assistant")
        .instructions("Be concise, practical, and structured.")
        .build();

    let result = run(&agent, "Give me three production readiness checks.").await?;
    println!("{}", result.final_output.unwrap_or_default());
    Ok(())
}
```

For a fuller walkthrough, open [docs/quickstart.md](docs/quickstart.md).

## Choose A Path

| I want to... | Read this |
| --- | --- |
| build a first agent | [docs/quickstart.md](docs/quickstart.md) |
| understand agents, tools, and handoffs | [docs/agents.md](docs/agents.md), [docs/tools.md](docs/tools.md), [docs/handoffs.md](docs/handoffs.md) |
| run with sessions and replay state | [docs/sessions/README.md](docs/sessions/README.md), [docs/results.md](docs/results.md) |
| stream events live | [docs/streaming.md](docs/streaming.md) |
| integrate OpenAI-specific behavior | [docs/models/openai.md](docs/models/openai.md), [docs/sessions/openai.md](docs/sessions/openai.md) |
| use MCP servers and resources | [docs/mcp.md](docs/mcp.md) |
| build realtime or voice flows | [docs/realtime/README.md](docs/realtime/README.md), [docs/voice/README.md](docs/voice/README.md) |
| debug traces and runtime behavior | [docs/tracing.md](docs/tracing.md) |

## Public Surface

The top-level published package is `openai-agents-rs`, and the Rust crate name remains `openai_agents`.

- runtime: `Agent`, `Runner`, `RunConfig`, `RunOptions`, `RunResult`, `RunResultStreaming`
- OpenAI: `OpenAIProvider`, `OpenAIResponsesModel`, `OpenAIChatCompletionsModel`
- sessions: `MemorySession`, `SQLiteSession`, `OpenAIConversationsSession`, `OpenAIResponsesCompactionSession`
- namespaces:
  - `openai_agents::realtime`
  - `openai_agents::voice`
  - `openai_agents::extensions`

The curated API map lives in [docs/ref/README.md](docs/ref/README.md).

## Examples

Runnable examples live in `crates/openai-agents/examples`.

- [basic_run.rs](crates/openai-agents/examples/basic_run.rs)
- [memory_session.rs](crates/openai-agents/examples/memory_session.rs)
- [realtime_session.rs](crates/openai-agents/examples/realtime_session.rs)

The full example index is in [docs/examples.md](docs/examples.md).

## Workspace Layout

- `crates/openai-agents`: public facade crate
- `crates/agents-core`: shared runtime primitives
- `crates/agents-openai`: OpenAI-specific implementation
- `crates/agents-realtime`: realtime runtime
- `crates/agents-voice`: voice runtime
- `crates/agents-extensions`: optional and experimental integrations
- `docs/`: product docs, guides, and curated reference
- `examples/`: example landing page

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md).

Short version:

```bash
cargo fmt --all
cargo test --workspace
```

If you change docs, also run:

```bash
docs/scripts/check_links.sh
docs/scripts/generate_llms_exports.sh
```

## Status

The project is pre-1.0. The runtime is already broad, but APIs may still tighten as the library gets simpler and more stable.

## License

Apache License 2.0. See [LICENSE](LICENSE).
