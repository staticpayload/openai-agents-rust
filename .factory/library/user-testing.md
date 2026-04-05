# User Testing

Testing surface, validation tools, and concurrency guidance for this mission.

**What belongs here:** testable surfaces, validation tools, setup notes, and concurrency/resource guidance.
**What does NOT belong here:** implementation plans or feature decomposition.

---

## Validation Surface

This mission validates a Rust library/runtime surface rather than a browser UI.

Primary validation tools:

- `cargo test` for crate-local unit tests and facade/integration tests
- `cargo fmt --all` for formatting gate
- parity/doc contract tests under `crates/openai-agents/tests`

Primary assertion surfaces:

- crate-local runtime helpers in `agents-core`, `agents-openai`, `agents-realtime`, `agents-voice`, and `agents-extensions`
- cross-crate semantics in `crates/openai-agents/tests`
- parity/docs truthfulness in `crates/openai-agents/tests/behavior_parity.rs`, `root_export_parity.rs`, and future docs-contract tests

## Validation Concurrency

### Surface: Cargo semantic validation

- Max concurrent validators: **3**
- Rationale:
  - machine capacity: 10 CPU cores, 16 GiB RAM
  - dry-run max RSS for warm validation commands was ~45-83 MB, but real Rust test/build activity can spike substantially above warm-cache samples
  - Cargo/rustc are CPU-heavy and can fan out subprocess work, so CPU is the practical limiter before memory
  - using a conservative cap of 3 keeps validation under roughly 70% of likely CPU headroom while avoiding needless contention

## Readiness notes

- Dry run already confirmed that `cargo fmt --all -- --check`, targeted parity tests, and `cargo test -q --workspace --no-run` are executable in this environment.
- No browser setup, auth bootstrap, or long-running app services are required.
