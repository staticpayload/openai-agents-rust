# Behavior Parity

Behavior parity is tracked against the pinned Python test families in
`reference/openai-agents-python/tests` and the major JS package test families in
`reference/openai-agents-js/packages/*/test`.

Allowed statuses:

- `covered`: there is Rust coverage for the family and the runtime surface is materially present
- `omitted-with-rationale`: intentionally not ported because it is environment-specific or JS-only

## Family Ledger

### Core Runner

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_agent_runner` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Core non-streamed runner, nested tools, resume, and default-runner behavior are exercised. |
| `test_agent_runner_streamed` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Live streamed runs, event ordering, and completion state are exercised. |
| `test_agent_runner_sync` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Tokio bridging and runtime rejection are covered. |
| `test_max_turns` | `covered` | `crates/agents-core/src/run.rs` | Max-turn termination and handler behavior are covered in crate tests. |

### OpenAI

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_openai_conversations_session` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Session state load/save, clear behavior, remote bootstrap, and runner continuity are covered. |
| `memory/test_openai_responses_compaction_session` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Candidate selection, sanitization, threshold-sensitive compaction, previous-response-id mode, and runner-triggered auto compaction are covered. |
| `test_openai_responses` | `covered` | `crates/agents-openai/src/models.rs`, `crates/agents-openai/src/websocket.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Responses payload shaping, tool/output conversion, conversation tracking, websocket request framing, and response parsing are covered. |
| `test_openai_chatcompletions` | `covered` | `crates/agents-openai/src/models.rs` | Chat Completions payload shaping, tool choice defaults, logprobs, and response parsing are covered. |
| `test_responses_websocket_session` | `covered` | `crates/agents-openai/src/websocket.rs` | Responses websocket URL building, headers, query handling, and request framing are covered. |

### MCP

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `mcp/test_runner_calls_mcp` | `covered` | `crates/openai-agents/tests/mcp_semantics.rs` | Non-streamed and streamed MCP tool execution through the runner are covered. |
| `mcp/test_mcp_server_manager` | `covered` | `crates/agents-core/src/mcp/manager.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | Connect, reconnect, deduplicated failures, active tool listing, and cleanup state are covered. |
| `mcp/test_mcp_resources` | `covered` | `crates/agents-core/src/mcp/server.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | Connection-gated resource listing, template listing, and resource reads are covered. |

### Realtime

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `realtime/test_runner` | `covered` | `crates/agents-realtime/src/runner.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Session creation, run-config model settings, and live session commands are covered. |
| `realtime/test_session` | `covered` | `crates/agents-realtime/src/session.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Live event streaming, agent lifecycle transitions, model-setting state, playback state, interrupts, and shutdown are covered. |
| `realtime/test_openai_realtime` | `covered` | `crates/agents-realtime/src/openai_realtime.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Websocket/SIP model behavior, event-type normalization, and realtime model session updates are covered. |

### Voice

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `voice/test_pipeline` | `covered` | `crates/agents-voice/src/pipeline.rs`, `crates/agents-voice/src/result.rs`, `crates/openai-agents/tests/voice_semantics.rs` | Live streamed audio results, transcript events, session lifecycle events, and streamed audio input are covered. |
| `voice/test_workflow` | `covered` | `crates/agents-voice/src/workflow.rs`, `crates/openai-agents/tests/voice_semantics.rs` | Single-agent workflow state, streamed core-runner output, and transcript extraction are covered. |

### JS Package Families

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `js/agents-core/run_and_streaming` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | JS core run and streamed-run package behavior maps to the shared Rust runner. |
| `js/agents-core/mcp` | `covered` | `crates/agents-core/src/mcp/manager.rs`, `crates/agents-core/src/mcp/util.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | JS MCP server, cache/filter, and runner integration families map to the Rust MCP runtime. |
| `js/agents-openai/responses_and_sessions` | `covered` | `crates/agents-openai/src/models.rs`, `crates/agents-openai/src/memory.rs`, `crates/agents-openai/src/websocket.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | JS OpenAI Responses, conversation session, compaction session, and websocket session families map to the Rust OpenAI package. |
| `js/agents-realtime/session` | `covered` | `crates/agents-realtime/src/runner.rs`, `crates/agents-realtime/src/session.rs`, `crates/agents-realtime/src/openai_realtime.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | JS realtime session, websocket, and SIP behavior maps to the Rust realtime session runtime. |
| `js/agents-extensions/realtime_transports` | `omitted-with-rationale` | `n/a` | Cloudflare and Twilio transport adapters are JS-environment-specific transport layers and are not yet first-class Rust transports. |
