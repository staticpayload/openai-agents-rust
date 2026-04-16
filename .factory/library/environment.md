# Environment

Environment variables, external dependencies, and setup notes.

**What belongs here:** required env vars, external API keys/services, dependency quirks, platform-specific notes.
**What does NOT belong here:** service ports/commands (use `.factory/services.yaml`).

---

## Required Local Tooling
- Rust toolchain with `cargo`, `rustfmt`, and current workspace-compatible toolchain installed.
- Network access to crates.io for `cargo fetch`, `cargo test` on cold machines, packaging, and publish verification.
- Docker is currently unavailable in this environment; Docker-backed sandbox validation is blocked until that changes.
- `rg` is currently unavailable in this environment; `docs/scripts/check_links.sh` depends on it today and must be fixed or shimmed during the mission.
- Run `.factory/init.sh` before heavy mission work when you need a quick preflight: it performs `cargo fetch --locked` and warns if `rg` or Docker are unavailable in the current environment.

## Environment Variables
- `OPENAI_API_KEY`: required for live OpenAI-backed example/integration validation.
- `CARGO_REGISTRY_TOKEN`: required for crates.io publication.
- Hosted sandbox provider credentials are not part of the default validation path for this mission; hosted providers are code-parity-only unless the user later provides provider-specific credentials.

## Mission-Specific External Context
- Upstream Python source of truth clone: `/tmp/openai-agents-python-upstream-nph6i2p0`
- Mission state directory: `/Users/staticpayload/.factory/missions/a79cf586-bde6-417e-92a4-dc7681913eac`

## Local Boundaries
- Do not touch or depend on local services already using ports such as `5000`, `7000`, `8790`, or `8791`.
- Do not store credentials in repo files or commit generated secrets.
- Do not assume Docker, ripgrep, provider CLIs, audio hardware, or hosted sandbox accounts exist unless the feature explicitly establishes that prerequisite.

## Validator Quirks
- `cargo test <filter> -- --exact` does **not** match crate-local unit tests by bare test name alone; for `src/lib.rs` unit tests you need the fully qualified module path, or an integration test with a bare exact name if the mission contract expects a short filter.
