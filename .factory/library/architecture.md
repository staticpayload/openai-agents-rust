# Architecture

High-level system model for the Rust parity port.

**What belongs here:** crate boundaries, cross-crate data flow, runtime invariants, and where parity evidence lives.
**What does NOT belong here:** step-by-step feature tasks, port-by-port implementation notes, or service commands.

---

## Workspace shape

The workspace is intentionally split into focused crates with `openai-agents` as the public facade:

- `agents-core`: shared runner, agent/tool model, session abstractions, guardrails, handoffs, MCP abstractions, tracing hooks, and common model/provider interfaces.
- `agents-openai`: OpenAI-specific provider/model/session implementations, including Responses, Chat Completions, websocket transports, and OpenAI-backed session persistence helpers.
- `agents-realtime`: realtime session/runtime types, event normalization, audio-format helpers, playback tracking, and websocket/SIP model adapters.
- `agents-voice`: voice pipeline/workflow abstractions plus STT/TTS/input adapters layered on top of core/realtime behavior.
- `agents-extensions`: optional runtime helpers and extensions, including handoff filters/prompts, visualization helpers, and JS-inspired realtime transport adapters.
- `openai-agents`: the user-facing facade that re-exports the stable Rust surface and provides the integration-test surface for cross-crate parity.

## Source-of-truth model

Behavioral truth comes from the pinned upstream SDKs under `reference/`.

Precedence order for this mission:

1. Python SDK wins for runtime semantics and behavioral expectations.
2. JS SDK wins for package shape, namespace shape, and JS-specific transport/runtime patterns only where adopting that shape does not change the Python-defined runtime behavior.
3. Existing landed Rust behavior should be preserved unless it is demonstrably off-truth or blocks required parity.
4. Rust-first API cleanup is allowed only when it does not weaken parity commitments or remove upstream runtime capability.

The parity mission should preserve existing landed Rust behavior, only broadening or correcting it toward upstream truth.

## Runtime flow

### Core execution flow

1. User code enters through the facade (`openai-agents`) or directly through `agents-core`.
2. `agents-core` runner builds normalized model input from user input, session history, run config, and handoff state.
3. The selected model/provider implementation executes:
   - generic providers via `agents-core`
   - OpenAI-backed providers via `agents-openai`
   - realtime flows via `agents-realtime`
4. Tool calls, handoffs, guardrails, approvals, and session persistence are mediated by `agents-core`.
5. Results flow back through shared run/result types and are re-exported by the facade.

### Session and continuity flow

- `agents-core` owns normalized runner history, replay/resume behavior, handoff-visible history, and generic session-visible state.
- `agents-openai` owns provider-specific continuity tokens and behaviors such as `conversation_id`, `previous_response_id`, and compaction, but must plug into the shared core history model rather than create a divergent one.
- Realtime session state lives in `agents-realtime` and should stay compatible with extension transports and voice workflows.

### Voice and realtime flow

- `agents-voice` composes workflows and pipeline behavior on top of core agent execution and/or realtime semantics.
- `agents-realtime` owns normalized realtime events, audio formats, session lifecycle, and playback state.
- `agents-extensions` transport adapters must emit payloads and normalized types that are compatible with the realtime crate’s expectations.

## Architectural invariants

- The public facade must stay coherent with crate internals: no facade-only aliases or exports that drift from the underlying runtime behavior.
- Already-covered parity areas must not regress while expanding uncovered areas.
- Behavioral parity counts only when the runtime capability exists and is exercised by tests/audits; file presence or export presence alone is not enough.
- Omitted parity families must remain narrow, explicit, and justified.
- Reference sources under `reference/` are read-only inputs, never edited.
- Keep the existing crate split and Rust-first ergonomics unless upstream behavior requires a broader runtime surface.

## Validation architecture

The main validation surface is Cargo-based rather than browser-based. The executable validation contract and its tests are the definition of done; parity docs and ledgers are downstream evidence that must stay aligned with those executable checks.


- crate-local unit tests validate helpers, converters, schemas, and state machines
- facade/integration tests under `crates/openai-agents/tests` validate cross-crate semantics
- parity-ledger tests validate inventory truthfulness, export parity, and documentation alignment
- final release gate is `cargo fmt --all` plus `cargo test -q --workspace`

## Documentation and parity evidence flow

- `docs/BEHAVIOR_PARITY.md` is the ledger of upstream Python/JS families and current Rust status.
- `docs/behavior_parity_overrides.json` and `scripts/generate_behavior_parity.py` define how the ledger is generated and curated.
- `docs/ROOT_EXPORT_PARITY.md` documents Python root-export mapping into the Rust facade.
- README and parity docs must describe the shipped runtime truthfully, not historical bootstrap state.
