# Contributing

## Development Setup

- Rust `1.85+`
- `cargo fmt`
- `cargo test`

Clone the repo and work from the workspace root:

```bash
cargo fmt --all
cargo test --workspace
```

If you change docs, also run:

```bash
docs/scripts/check_links.sh
docs/scripts/generate_llms_exports.sh
```

## Where To Make Changes

| Area | Crate |
| --- | --- |
| agents, runners, tools, sessions, tracing, guardrails, results | `crates/agents-core` |
| OpenAI provider, models, OpenAI-specific session behavior, hosted tools | `crates/agents-openai` |
| realtime runtime, events, session control, transport-backed models | `crates/agents-realtime` |
| voice workflows, pipelines, streamed audio results | `crates/agents-voice` |
| optional transports, provider adapters, extra memory backends, experimental APIs | `crates/agents-extensions` |
| top-level public imports and namespace surface | `crates/openai-agents` |

## General Rules

- Prefer the facade crate for consumption, not for implementation.
- Put behavior in the lowest crate that actually owns it.
- Keep the public API small and typed.
- Prefer additive, readable patches over wide refactors.
- Keep tests close to the behavior you changed.

## Before Opening A Change

Run:

```bash
cargo fmt --all
cargo test --workspace
```

If you changed a public surface, update the README and any affected docs pages.

## Documentation

The docs live in `docs/` and are written as product docs rather than internal notes.

- `docs/index.md`: docs home
- `docs/ref/`: curated API map
- `docs/examples.md`: example index
- `docs/scripts/`: docs maintenance utilities
- `docs/llms.txt` and `docs/llms-full.txt`: generated LLM-readable exports

Keep one canonical page per topic. Prefer updating an existing page over creating near-duplicates.

## Repo Health Files

This repository intentionally keeps the usual open-source project entrypoints in the root and `.github/`:

- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `SUPPORT.md`
- `CHANGELOG.md`
- `.github/ISSUE_TEMPLATE/`
- `.github/pull_request_template.md`
- `.github/CODEOWNERS`
- `.github/workflows/ci.yml`

## Style

- Keep modules focused.
- Prefer explicit builders/options over large implicit flows.
- Write code that is easy to skim.
- Avoid placeholder abstractions that do not yet carry real behavior.
- Write docs that start with a working example and then explain the concept.
