# openai-agents-rust

Native Rust agents runtime with a clean top-level facade, OpenAI integrations, realtime sessions, voice workflows, MCP support, and extension hooks.

This project is built as a real Rust implementation, not a thin wrapper over another SDK.

## Why This Exists

- Rust teams should be able to build agent systems without crossing an FFI boundary.
- Realtime, tool use, sessions, and streaming should feel native in async Rust.
- The public API should stay small at the top and deep when you need to drop into subsystem crates.

## What You Get

- `run`, `run_streamed`, `run_with_session`, and `run_sync` entry points
- agent composition, nested agents as tools, handoffs, guardrails, and replayable run results
- OpenAI Responses and Chat Completions integrations
- session-aware runs with memory and OpenAI conversation state
- MCP servers, approvals, resources, and runtime tool discovery
- realtime agents and live session control
- voice workflows and pipelines on top of streamed runs
- optional extensions for extra providers, memory backends, visualization, Codex flows, and transport adapters

## Workspace

- `crates/openai-agents`: public facade crate
- `crates/agents-core`: runner, agents, tools, sessions, tracing, results
- `crates/agents-openai`: OpenAI provider, models, hosted tools, OpenAI-specific sessions
- `crates/agents-realtime`: realtime agents, sessions, events, model adapters
- `crates/agents-voice`: voice workflow and pipeline runtime
- `crates/agents-extensions`: optional integrations and experimental surfaces

## Quick Start

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

## Session-Aware Runs

```rust
use openai_agents::{Agent, MemorySession, Runner};

#[tokio::main]
async fn main() -> Result<(), openai_agents::AgentsError> {
    let agent = Agent::builder("assistant")
        .instructions("Track the conversation and answer briefly.")
        .build();

    let session = MemorySession::new("demo");
    let runner = Runner::new();

    runner.run_with_session(&agent, "My name is Ada.", &session).await?;
    let result = runner
        .run_with_session(&agent, "What is my name?", &session)
        .await?;

    println!("{}", result.final_output.unwrap_or_default());
    Ok(())
}
```

## Realtime And Voice

The facade also exposes:

- `openai_agents::realtime` for long-lived realtime sessions, events, interrupts, agent updates, and transport-backed model adapters
- `openai_agents::voice` for STT -> workflow -> TTS pipelines and streamed audio results
- `openai_agents::extensions` for optional transports, provider adapters, memory backends, graph rendering, and experimental integrations

## Design Goals

- Rust-first ergonomics
- explicit async flows
- small top-level API, deeper subsystem modules when needed
- production-quality typed surfaces instead of loosely-shaped blobs
- composable runners, providers, sessions, and tools

## Project Status

The codebase is pre-1.0. The runtime surface is large and usable, but APIs may still evolve as the implementation is tightened and simplified.

## Development

```bash
cargo fmt --all
cargo test --workspace
```

Pinned upstream references used during development live under `reference/`.

## License

MIT
