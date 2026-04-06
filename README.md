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

## Package And Import Names

The top-level published package is `openai-agents-rs`, and the Rust crate name remains `openai_agents`.

```toml
openai-agents = { package = "openai-agents-rs", version = "0.1.2" }
```

```rust
use openai_agents::{Agent, Runner};
```

## Public Surface

- runtime: `Agent`, `Runner`, `RunConfig`, `RunOptions`, `RunResult`, `RunResultStreaming`
- OpenAI: `OpenAIProvider`, `OpenAIResponsesModel`, `OpenAIChatCompletionsModel`
- sessions: `MemorySession`, `SQLiteSession`, `OpenAIConversationsSession`, `OpenAIResponsesCompactionSession`
- namespaces:
  - `openai_agents::realtime`
  - `openai_agents::voice`
  - `openai_agents::extensions`

The curated API map lives in [docs/ref/README.md](docs/ref/README.md).

## Crate Map

| Crate | Responsibility |
| --- | --- |
| `openai-agents-rs` | public facade and normal application entry point |
| `openai-agents-core-rs` | agents, runners, tools, sessions, handoffs, tracing |
| `openai-agents-openai-rs` | OpenAI provider, sessions, hosted tools |
| `openai-agents-realtime-rs` | realtime runner, session, event, and audio flow |
| `openai-agents-voice-rs` | voice workflow, STT/TTS, streamed audio results |
| `openai-agents-extensions-rs` | optional transports, adapters, backends, and extras |

## Examples

Runnable examples live in `crates/openai-agents/examples`.

- [basic_run.rs](crates/openai-agents/examples/basic_run.rs)
- [memory_session.rs](crates/openai-agents/examples/memory_session.rs)
- [streamed_run.rs](crates/openai-agents/examples/streamed_run.rs)
- [realtime_session.rs](crates/openai-agents/examples/realtime_session.rs)
- [voice_pipeline.rs](crates/openai-agents/examples/voice_pipeline.rs)

The full example index is in [docs/examples.md](docs/examples.md).

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

## Release Hygiene

- CI lives in `.github/workflows/ci.yml`
- issue and PR templates live in `.github/`
- release notes live in [CHANGELOG.md](CHANGELOG.md)
- security and support guidance live in [SECURITY.md](SECURITY.md) and [SUPPORT.md](SUPPORT.md)

## Status

The project is pre-1.0. The runtime is already broad, but APIs may still tighten as the library gets simpler and more stable.

## License

Apache License 2.0. See [LICENSE](LICENSE).
