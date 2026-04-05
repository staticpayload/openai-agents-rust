---
name: runtime-parity-worker
description: Implement and verify Rust runtime parity features across core, OpenAI, MCP, realtime, voice, and extensions.
---

# Runtime Parity Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for features that change Rust runtime behavior, add or deepen semantic tests, close parity-critical gaps in crate code, or tighten cross-crate behavior through executable tests.

## Required Skills

- `test-driven-development` — invoke before changing code or tests; write the failing test first, then implement.
- `systematic-debugging` — invoke whenever a new or existing validator fails unexpectedly and root cause is not already obvious.
- `verification-before-completion` — invoke after implementation and before handoff so the final report is backed by fresh command output.

## Work Procedure

1. Read the assigned feature, `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/parity.md`, and the relevant upstream reference tests under `reference/openai-agents-python` and `reference/openai-agents-js`.
2. Identify the exact Rust files and exact upstream behaviors the feature is supposed to close. Do not widen scope beyond the assigned feature.
3. Invoke `test-driven-development`.
4. Add or expand the narrowest failing Rust tests first:
   - prefer crate-local unit tests for helpers/state machines
   - prefer `crates/openai-agents/tests/*.rs` for cross-crate semantics
   - if the feature changes parity audits, add the failing audit/assertion test first
   - if the feature claims transport/runtime wiring, include at least one public or cross-crate assertion that proves the real runtime path consumes the configured values; helper-only normalization tests are insufficient
5. Run the smallest relevant test command and confirm it fails for the intended reason before implementation. If the feature is audit-oriented and the real gap is missing executable coverage around behavior that already works, capture that explicitly in the transcript and in the handoff instead of pretending there was a runtime failure.
6. Implement the runtime change using existing crate boundaries and public facade patterns:
   - preserve the current crate split and facade
   - preserve existing landed parity behavior unless it is off-truth
   - keep code small, explicit, and production-grade
   - never edit anything under `reference/`
7. Re-run the same targeted tests until they pass. If a failure is not immediately obvious, invoke `systematic-debugging` before changing more code.
8. Run adjacent targeted tests for the same surface so the feature is validated in context (for example core + facade tests, or extension + realtime tests).
9. Run `cargo fmt --all` if you changed Rust files.
10. Invoke `verification-before-completion`.
11. Before handoff, run the relevant commands from `.factory/services.yaml` for the touched surface, plus any feature-specific verification commands.
12. Update parity docs or ledgers only when the runtime/test evidence now justifies it. Never mark a family covered unless executable Rust validation now exists.
13. Produce a concrete handoff with exact files changed, exact commands run, and any remaining gaps.

## Example Handoff

```json
{
  "salientSummary": "Implemented server-conversation delta tracking and added websocket runtime tests for OpenAI Responses. The Rust runtime now preserves unsent filtered deltas, supports rewind on retry, and validates websocket request framing plus streamed completion semantics against the new tests.",
  "whatWasImplemented": "Added server-conversation tracker state transitions in crates/agents-core/src/internal/oai_conversation.rs, expanded websocket/runtime coverage in crates/agents-openai/src/models.rs and crates/agents-openai/src/websocket.rs, and added facade-level regression coverage so the new parity assertions are executable rather than ledger-only.",
  "whatWasLeftUndone": "Did not address MCP transport auth/session-id parity because that is tracked by a later feature.",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -q -p agents-core --lib oai_conversation",
        "exitCode": 0,
        "observation": "New delta-tracking and rewind tests passed."
      },
      {
        "command": "cargo test -q -p agents-openai --lib websocket",
        "exitCode": 0,
        "observation": "Websocket framing and runtime assertions passed, including the new continuity-field case."
      },
      {
        "command": "cargo test -q -p openai-agents --test openai_session_semantics",
        "exitCode": 0,
        "observation": "Facade-level continuity tests passed with the new multi-turn reuse assertion."
      },
      {
        "command": "cargo fmt --all",
        "exitCode": 0,
        "observation": "Workspace formatting succeeded after the Rust edits."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Compared the new Rust assertions against the pinned upstream Python and JS tests for the same behavior families.",
        "observed": "The Rust test coverage now exercises the same continuity and websocket semantics the feature was assigned to close."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "crates/agents-core/src/internal/oai_conversation.rs",
        "cases": [
          {
            "name": "tracker_only_replays_unsent_filtered_deltas",
            "verifies": "Only unsent filtered items are prepared for the next request."
          },
          {
            "name": "tracker_rewinds_sent_state_after_retry",
            "verifies": "Retry paths can safely resend the previous delta set."
          }
        ]
      },
      {
        "file": "crates/agents-openai/src/websocket.rs",
        "cases": [
          {
            "name": "websocket_omits_previous_response_id_when_conversation_is_present",
            "verifies": "Server-managed conversation continuity is not double-bound."
          }
        ]
      }
    ]
  },
  "discoveredIssues": [
    {
      "severity": "medium",
      "description": "MCP streamable HTTP auth/session-id params are still missing from the Rust transport surface.",
      "suggestedFix": "Track this in the dedicated MCP transport parity feature rather than expanding the current OpenAI feature."
    }
  ]
}
```

## When to Return to Orchestrator

- The feature requires changing mission boundaries, crate boundaries, or public-facade direction.
- The required parity behavior depends on an upstream family whose intended semantics are still ambiguous after reading both pinned references.
- The feature uncovers a pre-existing unrelated failure that blocks validation for this surface.
- The feature needs new external infrastructure or credentials, which this mission is not expected to require.
