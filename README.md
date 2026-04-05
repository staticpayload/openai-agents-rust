# openai-agents-rust

Rust-first port of the OpenAI Agents SDK.

This repository currently contains:

- A hybrid Cargo workspace mirroring the planned crate boundaries.
- Pinned local clones of the Python and JS/TS reference SDKs under `reference/`.
- Generated porting docs under `docs/`, including a file-by-file translation matrix for the Python SDK.
- A public Rust facade plus crate-split runtime surfaces for core runner flows, OpenAI integrations, realtime sessions, voice workflows, and extensions.

## Workspace layout

- `crates/agents-core`: shared agent, runner, tool, session, tracing, and result abstractions
- `crates/agents-openai`: default OpenAI provider, hosted tools, and OpenAI-specific sessions/models
- `crates/agents-realtime`: realtime-specific agent/session/event types
- `crates/agents-voice`: voice pipeline and workflow abstractions
- `crates/agents-extensions`: optional integrations and experimental features
- `crates/openai-agents`: public facade crate

## Facade surface

The `openai-agents` facade exposes the cross-crate runtime most users interact with:

- core runner entry points such as `run`, `run_streamed`, and `run_with_session`
- top-level OpenAI runtime exports including `OpenAIProvider`, `OpenAIResponsesModel`, and `OpenAIChatCompletionsModel`
- namespace modules for `realtime`, `voice`, and `extensions`

## Reference sources

- `reference/openai-agents-python`
- `reference/openai-agents-js`

## Current status

The workspace builds and ships executable parity coverage across the facade, core runner, MCP, OpenAI session/runtime slices, realtime, voice, and extension transport surfaces. Remaining upstream families are tracked in `docs/BEHAVIOR_PARITY.md` until the corresponding Rust runtime behavior and tests land.
