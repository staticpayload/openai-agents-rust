# Architecture

This mission should extend the existing Rust workspace, not create a parallel sandbox subsystem. Treat `Agent` as the stable role definition, `Runner`/`RunConfig`/`RunState` as the execution boundary, and use the current crate layering so sandbox parity lands in the same public facade and validation surfaces the rest of the SDK already uses.

## Workspace Layers
- `crates/agents-core` is the runtime kernel: agents, runner loop, tools, handoffs, sessions, tracing, durable run state, and the place where provider-neutral sandbox execution should plug in.
- `crates/agents-openai` is the OpenAI integration layer: providers, models, OpenAI-aware sessions, hosted tools, and request-shaping behavior that sits on top of core runtime contracts.
- `crates/agents-realtime` and `crates/agents-voice` are specialized execution modes built on shared core concepts instead of duplicating the base runner architecture.
- `crates/agents-extensions` is the optional integration layer for extra transports, memory backends, and provider-specific features; hosted sandbox providers belong here.
- `crates/openai-agents` is the user-facing facade crate; it should re-export finished sandbox surfaces rather than owning sandbox logic itself.
- `docs/`, `examples/`, and `crates/openai-agents/tests` are the outward-facing contract surfaces that should reflect any new sandbox capability once it is actually shipped.

## Public Entry Surfaces
- Standard execution enters through `openai_agents::{run, run_sync, run_streamed, Runner}`.
- Declarative runtime state enters through `openai_agents::{Agent, RunConfig, RunOptions, RunResult, RunResultStreaming, RunState}`.
- Specialized namespaces already live behind `openai_agents::realtime`, `openai_agents::voice`, and `openai_agents::extensions`.
- Sandbox parity should join these same public surfaces with an explicit `openai_agents::sandbox` namespace plus any top-level re-exports the facade needs for ergonomics.
- `SandboxAgent` should be treated as a first-class public type that preserves normal `Agent` role semantics while adding sandbox-specific defaults/config hooks; do not create a separate orchestration model that bypasses `Agent`/`Runner`.

## Crate Responsibilities
### openai-agents-rs
- Keep this crate as a thin facade.
- Re-export sandbox types once they are stable, but do not move sandbox runtime ownership here.
- Preserve the current pattern where the facade is the primary import path and semantics tests exercise it as the public contract.

### openai-agents-core-rs
- Continue owning the base runtime loop, agent definitions, tools, handoffs, sessions, tracing, and durable run state.
- This is the correct landing zone for provider-neutral sandbox architecture: `SandboxAgent`, grouped sandbox run config, manifest/entry abstractions, capability-driven tool injection, sandbox lifecycle orchestration, local resume state, and workspace safety rules.
- Local Unix and Docker sandbox backends should sit behind core sandbox traits/state because they are runtime mechanics rather than third-party hosted integrations.
- Docker support must stay feature-gated or dependency-isolated enough that default builds and crates.io publication do not require Docker-only dependencies.

### openai-agents-openai-rs
- Keep OpenAI-specific transport, model, hosted-tool, and OpenAI-session logic here.
- Only add sandbox-related work when OpenAI-specific request metadata, replay behavior, or session semantics must interoperate with sandboxed runs.
- Do not let sandbox runtime ownership drift into this crate.

### openai-agents-realtime-rs
- Keep realtime session/event transport concerns isolated here.
- Realtime should continue depending on shared core agent/runtime concepts instead of introducing a separate sandbox execution path.

### openai-agents-voice-rs
- Keep STT/TTS models, workflows, and streaming audio results isolated here.
- Voice should continue composing with core runner concepts rather than defining its own sandbox state model.

### openai-agents-extensions-rs
- Keep optional integrations and feature-gated provider adapters here.
- Hosted sandbox providers and their provider-specific client/session/state types should land here, mirroring the current role of extensions for optional infrastructure-backed features.
- Secret-bearing provider config must remain outside core serialized runtime state except for safe, resumable identifiers.
- Existing experimental Codex sandbox-adjacent code belongs here as adjacent optional infrastructure; workers should integrate with it deliberately or leave it isolated, but not accidentally duplicate its responsibilities.

## Key Data and Control Flows
### Standard agent run
- The facade or a caller-owned `Runner` starts execution.
- `agents-core` normalizes input, applies run config and session state, resolves the model/provider, and drives the model → tools/handoffs/guardrails → result loop.
- The run exits as `RunResult` or `RunResultStreaming`, with durable state represented by `RunState`.

### Nested agent, tool, and handoff flow
- Agents remain declarative role definitions.
- `Agent::as_tool()` and handoffs reuse the same core runner semantics, with nested results and approvals captured in run context and durable state.
- New sandbox behavior should compose with this flow rather than bypass it.

### Session and resume flow
- Session implementations own conversational history and replay boundaries.
- `RunState` is the serialized pause/resume boundary for turns, generated items, approvals, trace context, and provider-managed conversation metadata.
- Sandbox resume must preserve this contract by layering sandbox session/workspace state into the same resume story.

### Streaming, realtime, and voice flow
- Streaming uses the same underlying core execution semantics while exposing incremental events.
- Realtime and voice build adjacent live-session pipelines on top of shared agent/runtime concepts instead of redefining the base data model.
- Sandbox parity should preserve this single-source-of-truth runtime model.

### Target sandbox-enabled run flow
- A `SandboxAgent` should still execute through the normal runner entry points.
- Sandbox-specific runtime preparation should create or resume a sandbox session, materialize manifest/snapshot state, bind capabilities/tools, and then hand execution back to the existing runner loop.
- Completion or interruption should yield resumable sandbox state that stays aligned with `RunState`, approvals, handoffs, and nested-agent semantics.
- Sandbox capabilities should reuse the existing tool, approval, interruption, and resume abstractions rather than introducing a parallel tool/event/state system.

## Parity Expansion Targets
- Add a core sandbox namespace that mirrors upstream Python’s neutral sandbox concepts: agent type, run config, manifest, entries, capabilities, session lifecycle, snapshots, workspace path policy, and sandbox memory/state.
- Extend `agents-core` run configuration and durable run state so sandbox execution is a first-class runner concern rather than a side executor.
- Add local Unix and Docker sandbox implementations under the core sandbox architecture so they share the same lifecycle and resume model.
- Add hosted sandbox providers under `agents-extensions`, feature-gated and secret-safe, with facade exposure following the existing extensions pattern.
- Update the facade, examples, docs, and black-box semantics tests after the underlying crate boundaries are in place.

## Architectural Invariants
- The facade crate stays thin; runtime ownership lives below it.
- Stable role behavior belongs on `Agent`/`SandboxAgent`; request- and environment-specific execution belongs on `Runner`, `RunConfig`, and `SandboxRunConfig`, while `RunOptions` is reserved for per-call overrides rather than long-lived sandbox definition.
- `RunState` remains the canonical public pause/resume boundary, even if sandbox uses dedicated nested session-state types internally.
- When adding new default hooks or behavior to `ModelProvider`, audit wrapper providers such as `MultiProvider` and forward the new behavior explicitly so routed models preserve provider-specific metadata and request shaping.
- Runner-created sandbox sessions are runner-managed; caller-injected live sessions remain caller-managed.
- Sandbox file, shell, and patch operations must stay rooted to the sandbox workspace and must not escape onto the host filesystem.
- Optional hosted backends must not force third-party provider dependencies into the core runtime crate.
- Realtime and voice remain consumers of shared runtime semantics, not forks of them.
- Public behavior is enforced at the facade/test layer, so new sandbox capabilities should preserve the existing black-box contract style.
