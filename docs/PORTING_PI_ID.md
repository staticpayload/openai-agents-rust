# Porting PI / Implementation Doc

## Objective

Port the OpenAI Agents SDK into a Rust-first workspace that preserves the common conceptual surface shared by the Python and JS/TS SDKs:

- provider-agnostic core
- default OpenAI provider
- explicit `realtime` namespace
- first-class tools, handoffs, guardrails, sessions, tracing, and voice support
- one ergonomic public facade crate

This repository is intentionally bootstrapped in two layers:

1. `reference/` contains pinned local clones of the upstream SDKs.
2. `crates/` contains the Rust workspace scaffold and the first public API surface.

## Pinned Upstream Sources

- Python SDK
  - Repository: `openai/openai-agents-python`
  - Local path: `reference/openai-agents-python`
  - Pinned SHA: `7a3f6b70d1bd4e700f3a06547b673295fb52e74d`
  - Source inventory: `168` files under `src/agents`
- JS/TS SDK
  - Repository: `openai/openai-agents-js`
  - Local path: `reference/openai-agents-js`
  - Pinned SHA: `5fd5b5306df3014e0582a8ad483b03eca381fd02`

The generated matrix in [PORTING_MATRIX.md](./PORTING_MATRIX.md) maps every Python source file to an intended Rust crate/module target.

## Workspace Architecture

### Public facade

- `crates/openai-agents`
  - Re-exports the stable public surface.
  - Re-exports OpenAI defaults and hosted tool constructors.
  - Exposes `realtime`, `voice`, and `extensions` as explicit namespaces.

### Internal crates

- `crates/agents-core`
  - shared agent, run, result, tool, model, session, tracing, and utility abstractions
- `crates/agents-openai`
  - default OpenAI provider, OpenAI models, hosted tools, websocket session, OpenAI-specific memory
- `crates/agents-realtime`
  - realtime agent/session/event scaffolding
- `crates/agents-voice`
  - voice workflow and pipeline scaffolding
- `crates/agents-extensions`
  - optional integrations, experimental APIs, and non-core providers

## Porting Rules

- Preserve capability, not Python packaging trivia.
- Use JS/TS as the tiebreaker for cross-SDK shared concepts and package boundaries.
- Keep one dominant concept per Rust file.
- Prefer Rust builders, traits, enums, `Result`, and `Stream` over Python-style overloaded runtime behavior.
- Keep orchestration internals crate-private under `internal/` or similarly scoped modules.
- Record every fold, rename, and deferral in the matrix before implementing the code.

## Current Bootstrap Delivered

- Hybrid Cargo workspace with the planned crate boundaries.
- Initial compile-target API surface for core concepts:
  - `Agent`, `Runner`, `run`, `RunResult`
  - `Tool`, `Handoff`, `InputGuardrail`, `OutputGuardrail`
  - `Session`, `Model`, `ModelProvider`, `Trace`, `Span`, `Usage`
- OpenAI-specific scaffold:
  - `OpenAIProvider`
  - `OpenAIResponsesModel`
  - `OpenAIChatCompletionsModel`
  - hosted tool constructors
  - OpenAI memory session placeholders
- Realtime, voice, and extension scaffolding crates
- Regeneration script:
  - [`scripts/generate_porting_docs.py`](../scripts/generate_porting_docs.py)

## Implementation Sequence

The next implementation passes should follow this order:

1. `config`, `debug`, `exceptions`, `version`, `logger`
2. `agent`, `agent_output`, `agent_tool_input`, `agent_tool_state`
3. `tool`, `function_schema`, `computer`, `editor`, `apply_diff`
4. `items`, `result`, `run_config`, `run_context`, `run_state`
5. `models`, provider resolution, and OpenAI transport layers
6. runner internals
7. session backends and persistence
8. MCP
9. tracing
10. realtime
11. voice
12. experimental/extensions
13. translated tests, examples, and conformance fixtures

## Regeneration

Regenerate the file-by-file mapping after updating the reference clones:

```bash
python3 scripts/generate_porting_docs.py
```
