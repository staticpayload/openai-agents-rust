---
name: parity-audit-worker
description: Maintain parity ledgers, documentation truthfulness, facade/export contracts, and final release gates.
---

# Parity Audit Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for features that update parity ledgers, audit generators, export/docs contract tests, README/runtime accuracy, or final workspace-green release work.

## Required Skills

- `test-driven-development` — invoke before adding or changing audit tests or doc-contract checks.
- `systematic-debugging` — invoke when parity/documentation tests fail in a non-obvious way or the generator/doc outputs disagree.
- `verification-before-completion` — invoke before handoff so ledger/docs claims are backed by fresh test output.

## Work Procedure

1. Read the assigned feature, `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/parity.md`, and the relevant docs/tests under `docs/`, `scripts/`, and `crates/openai-agents/tests/`.
2. Identify whether the feature is:
   - audit harness work (new tests/generator checks)
   - ledger refresh work (updating `docs/BEHAVIOR_PARITY.md` / overrides)
   - facade/export/doc accuracy work (`README.md`, `docs/ROOT_EXPORT_PARITY.md`, related contract tests)
3. Invoke `test-driven-development`.
4. Add the failing audit/doc-contract tests first. Good targets are:
   - `crates/openai-agents/tests/behavior_parity.rs`
   - `crates/openai-agents/tests/root_export_parity.rs`
   - a dedicated docs contract test file if needed
5. Run the smallest relevant test command and confirm the failure is for the intended reason.
6. Implement the minimal generator/doc/test updates needed to make the assertions true:
   - keep the parity ledger honest and exhaustive
   - never mark a family covered without executable Rust evidence
   - keep docs portable; no machine-local absolute paths
   - keep README aligned with the actual shipped runtime
7. Re-run the targeted audit tests until green.
8. If the feature refreshes ledger rows after runtime work, cross-check the referenced Rust coverage paths before updating any status or note text.
9. Run adjacent audit tests so generator, overrides, docs, and facade/export checks agree with each other.
10. Run `cargo fmt --all` if Rust test files were changed.
11. Invoke `verification-before-completion`.
12. For final release-gate work, run the exact mission-end commands: `cargo fmt --all` and `cargo test -q --workspace`.
13. Produce a handoff that clearly separates harness changes, doc/ledger changes, and any remaining omissions with exact rationale.

## Example Handoff

```json
{
  "salientSummary": "Added docs-contract coverage for README/runtime drift and root-export doc portability, then refreshed the parity ledger so covered rows now match executable Rust evidence. The audit surface now rejects placeholder omission notes and machine-local doc links.",
  "whatWasImplemented": "Created a docs contract test suite under crates/openai-agents/tests, expanded behavior_parity assertions to validate override liveness and executable coverage surfaces, updated docs/ROOT_EXPORT_PARITY.md and README.md to match the shipped facade/runtime, and refreshed docs/BEHAVIOR_PARITY.md from the current runtime evidence.",
  "whatWasLeftUndone": "A small set of browser-only JS families remain omitted with explicit rationale because they are not meaningful Rust runtime targets.",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -q -p openai-agents --test behavior_parity",
        "exitCode": 0,
        "observation": "Inventory, override-liveness, executable-surface, and omission-rationale checks all passed."
      },
      {
        "command": "cargo test -q -p openai-agents --test root_export_parity --test docs_contract",
        "exitCode": 0,
        "observation": "Root-export, portability, and README/runtime-alignment checks passed."
      },
      {
        "command": "cargo fmt --all",
        "exitCode": 0,
        "observation": "Formatting succeeded after the new Rust test file changes."
      },
      {
        "command": "cargo test -q --workspace",
        "exitCode": 0,
        "observation": "Full workspace remained green after the ledger/docs refresh."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Manually inspected updated parity rows and README status text against the actual changed runtime tests and exports.",
        "observed": "Every covered row and status summary now points at executable evidence and the docs no longer describe the repo as bootstrap-only."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "crates/openai-agents/tests/docs_contract.rs",
        "cases": [
          {
            "name": "readme_status_matches_runtime_audit",
            "verifies": "README wording reflects the shipped runtime breadth instead of outdated scaffold language."
          },
          {
            "name": "parity_docs_do_not_contain_machine_local_absolute_links",
            "verifies": "Parity-adjacent docs remain portable across machines."
          }
        ]
      }
    ]
  },
  "discoveredIssues": [
    {
      "severity": "low",
      "description": "Some JS-only browser families remain intentionally omitted because they are not Rust runtime targets.",
      "suggestedFix": "Keep them omitted with explicit rationale in the final ledger rather than trying to emulate browser-only surfaces."
    }
  ]
}
```

## When to Return to Orchestrator

- The feature would require marking a family covered without executable Rust validation.
- The relevant upstream family is ambiguous enough that the ledger/doc wording cannot be made honest without a product decision.
- README or parity docs would need to contradict the mission contract or current runtime evidence.
- The final workspace gate fails because of unrelated pre-existing issues outside the assigned feature’s scope.
