# Environment

Environment variables, external dependencies, and setup notes.

**What belongs here:** required env vars, external services, setup quirks, and mission-level environmental constraints.
**What does NOT belong here:** service ports/commands (use `.factory/services.yaml`).

---

## External dependencies

- No external credentials are required for the planned parity mission.
- The pinned upstream reference repos under `reference/` are read-only comparison inputs.

## Mission constraints

- Preserve the existing dirty workspace baseline; do not revert unrelated local changes.
- Never edit anything under `reference/`.
- Do not rely on browser-only or environment-specific live suites unless the feature explicitly requires a gated omission rationale.

## Toolchain assumptions

- Validation is Cargo-based.
- Workspace root: `/Users/staticpayload/Mainframe/openai-agents-rust`
- Rust toolchain is already installed and working in this environment.
