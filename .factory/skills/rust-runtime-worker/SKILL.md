---
name: rust-runtime-worker
description: Implement and verify runtime, provider, session, tool, MCP, realtime, and voice parity work in the Rust workspace.
---

# Rust Runtime Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

Use this skill for features that change core runtime behavior, provider logic, sessions, tool behavior, MCP, realtime, voice, extension transports, or other non-sandbox code in the Rust SDK.

## Required Skills

None.

## Work Procedure

1. Read `mission.md`, mission `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/parity.md`, `.factory/library/environment.md`, and the feature’s `fulfills` assertions before editing code.
2. Map the feature to the narrowest crate boundary that should own the change. Keep facade re-exports thin and preserve the existing crate layering.
3. Write failing tests first. Prefer crate-local unit tests for core behavior and facade semantics tests for public-contract behavior. If the feature changes public imports or docs-visible behavior, also add a temp-crate or example-based smoke test.
4. Verification-only exception: if the assigned behavior is already present and the real gap is missing exact regression coverage, contract-facing test names, or facade-level proof, you may take a verification-first path instead of forcing an artificial red/green cycle. In that case:
   - prove the shipped behavior already exists with the strongest available targeted commands
   - add or rename the missing regression coverage so the feature verification commands are exact and durable
   - state clearly in the handoff that you used the verification-only path and why no product code change was needed
5. Implement only after the new/updated tests fail for the intended reason when the feature actually requires code changes.
6. Re-run the targeted tests until they pass, then run broader validation from `.factory/services.yaml` appropriate to the blast radius:
   - always run the narrowest relevant `cargo test ...`
   - run `cargo check --workspace` for public API or cross-crate changes
   - run `cargo build --workspace --examples` if examples or facade imports are affected
7. Perform one manual shell verification when behavior is user-visible from the public facade and there is a stable shell path to exercise it (for example: temp Cargo project import, example build, or targeted `cargo run` smoke). If you intentionally rely on automated cargo validation only, say so explicitly in the handoff and explain why a manual shell check was not the right tool for that feature.
8. Before handoff, confirm no uncommitted generated junk or temp files remain.

## Example Handoff

```json
{
  "salientSummary": "Aligned OpenAI provider routing and session replay behavior with the parity contract, then verified the facade still exposes the same public entry surface.",
  "whatWasImplemented": "Added failing tests first for per-call RunOptions override behavior, session_input_callback replay rewriting, and provider metadata forwarding. Implemented the fixes in agents-core and agents-openai, then verified the facade-level semantics tests still pass without moving public ownership into the facade crate.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -p openai-agents-rs --test openai_session_semantics run_options_override_conversation_tracking_for_one_call_only -- --exact",
        "exitCode": 0,
        "observation": "Targeted parity regression test passed."
      },
      {
        "command": "cargo test -p openai-agents-core-rs runner_uses_session_input_callback_to_prepare_history -- --exact",
        "exitCode": 0,
        "observation": "Replay callback behavior now matches the contract."
      },
      {
        "command": "cargo check --workspace",
        "exitCode": 0,
        "observation": "Workspace typecheck stayed green after the API changes."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Built a temporary Cargo snippet importing `openai_agents::{Agent, Runner, OpenAIProvider}` from the facade.",
        "observed": "The snippet compiled without importing leaf crates directly."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "crates/agents-core/src/run.rs",
        "cases": [
          {
            "name": "runner_uses_session_input_callback_to_prepare_history",
            "verifies": "session_input_callback rewrites replay input without corrupting persisted items"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires a new public crate boundary or a major architectural choice not settled in `.factory/library/architecture.md`.
- Live validation needs credentials or external infrastructure that are not currently available.
- The change appears to belong in sandbox-specific architecture rather than the standard runtime/integration path.
- Existing unrelated test failures block verification and cannot be fixed within the feature scope.
