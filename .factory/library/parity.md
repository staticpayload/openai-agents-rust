# Parity Guidance

How parity is evaluated and maintained in this mission.

**What belongs here:** source-of-truth precedence, ledger rules, and practical parity principles.
**What does NOT belong here:** per-feature todo lists or implementation scratch notes.

---

## Upstream truth

1. Python SDK is the source of truth for runtime semantics.
2. JS SDK is the source of truth for package/namespace shape and JS-specific transport/runtime patterns where those patterns should map into Rust.
3. Existing landed Rust behavior should be preserved unless it is demonstrably off-truth or blocks required parity.

## What counts as parity

- Runtime capability must exist where upstream behavior is practical in Rust.
- Test coverage must exercise the behavior being claimed.
- Export presence alone does not count as parity.
- Documentation-only updates do not count as parity closure.
- Environment-specific live suites may remain omitted only with explicit rationale.

## Ledger rules

- `docs/BEHAVIOR_PARITY.md` must stay synchronized with the pinned upstream family inventory.
- Never mark a family `covered` unless executable Rust validation exists.
- Omitted families must use explicit, narrow rationales.
- `docs/behavior_parity_overrides.json` must not accumulate stale entries.

## Documentation rules

- README and parity docs must describe the current shipped runtime truthfully.
- Do not leave machine-local absolute paths in committed docs.
- Keep `docs/ROOT_EXPORT_PARITY.md` aligned with actual facade exports and intentional omissions.

## Realtime transport parity notes

- Cloudflare realtime transport parity includes the JS common realtime header bundle on fetch-upgrade requests, including both `User-Agent` and `X-OpenAI-Agents-SDK`.

## MCP manager parity notes

- Python's MCP manager starts with the full configured server list in its active set before any connection attempts. For Rust parity, preserving the "prior active set" on strict failures when `drop_failed_servers = false` must therefore include the fresh-manager first-failure case, not just reconnect/retry flows.
