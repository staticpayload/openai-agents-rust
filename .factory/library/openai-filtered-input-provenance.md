# OpenAI filtered-input provenance

Rust server-conversation tracking still has a structural gap compared with the JS reference for `call_model_input_filter` rewrites.

- `crates/agents-core/src/run_config.rs` exposes `ModelInputData { input, instructions }` only.
- The JS reference carries explicit filtered-input provenance via `sourceItems` / `filterApplied` in `packages/agents-core/src/runner/conversation.ts`.
- Because Rust does not carry per-filtered-item source mappings, `OpenAIServerConversationTracker` has repeatedly fallen back to tracker-side identity heuristics when filtered input drops, reorders, or replaces prepared items.
- Replacement rewrites can therefore still misattribute sent-state even when simple in-place mutation cases pass.

This is a factual architecture note for future parity work; it does not prescribe the fix shape, only the current Rust/JS mismatch and the bug family it causes.
