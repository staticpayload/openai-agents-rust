# Behavior Parity

This document is generated from the pinned Python and JS test trees plus
`docs/behavior_parity_overrides.json`.

Allowed statuses:

- `covered`: there is Rust coverage for the family and the runtime surface is materially present
- `omitted-with-rationale`: intentionally not closed yet or environment-specific; the omission is explicit

Tracked upstream families: `293`

### Core Runner

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_anthropic_thinking_blocks` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_asyncio_progress` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_call_model_input_filter` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Streamed and non-streamed `call_model_input_filter` rewrites plus invalid-return failures are covered. |
| `test_call_model_input_filter_unit` | `covered` | `crates/agents-core/src/run.rs` | Core unit coverage exercises sync/async filter rewrites and invalid-return rejection. |
| `test_cancel_streaming` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_config` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_debug` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_doc_parsing` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_example_workflows` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_extended_thinking_message_order` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_extra_headers` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_gemini_thought_signatures` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_gemini_thought_signatures_stream` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_global_hooks` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_guardrails` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_hitl_error_scenarios` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_hitl_session_scenario` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_hitl_utils` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_items_helpers` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_logprobs` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_max_turns` | `covered` | `crates/agents-core/src/run.rs` | Max-turn termination and handler behavior are covered in crate tests. |
| `test_model_payload_iterators` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_model_retry` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_pr_labels` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_pretty_print` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_process_model_response` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_reasoning_content` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_remove_openai_responses_api_incompatible_fields` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_repl` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_result_cast` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_config` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_context_approvals` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_context_wrapper` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_error_details` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_hooks` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_impl_resume_paths` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_internal_error_handlers` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_internal_items` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_state` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_step_execution` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_run_step_processing` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_runner_guardrail_resume` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_soft_cancel` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_source_compat_constructors` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_stream_events` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_stream_input_guardrail_timing` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_streaming_logging` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_streaming_tool_call_arguments` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_strict_schema` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_strict_schema_oneof` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `test_usage` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `utils/test_json` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |
| `utils/test_simple_session` | `omitted-with-rationale` | `n/a` | Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist. |

### Agent / Tool

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_agent_as_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_clone_shallow_copy` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_config` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_hooks` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_instructions_signature` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_llm_hooks` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_memory_leak` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_prompt` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_runner` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Core non-streamed runner, nested tools, resume paths, and default-runner behavior are exercised. |
| `test_agent_runner_streamed` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Live streamed runs, event ordering, and completion state are exercised. |
| `test_agent_runner_sync` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | Tokio bridging and runtime rejection are covered. |
| `test_agent_tool_input` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_tool_state` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agent_tracing` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_agents_logging` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_apply_diff` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_apply_diff_helpers` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_apply_patch_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_computer_action` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_computer_tool_lifecycle` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_function_schema` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_function_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_function_tool_decorator` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_handoff_history_duplication` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_handoff_prompt` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_handoff_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_local_shell_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_output_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_shell_call_serialization` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_shell_tool` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_choice_reset` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_context` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_converter` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_guardrails` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_metadata` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_output_conversion` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_use_behavior` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |
| `test_tool_use_tracker` | `omitted-with-rationale` | `n/a` | Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests. |

### Sessions

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `fastapi/test_streaming_context` | `omitted-with-rationale` | `n/a` | Session parity for this upstream family is not wired through the Rust runtime yet, so it stays omitted until the session behavior and tests land. |
| `memory/test_openai_responses_compaction_session` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Candidate selection, sanitization, threshold-aware compaction, previous-response-id mode, and runner-triggered compaction are covered. |
| `test_session` | `omitted-with-rationale` | `n/a` | Rust covers session shaping, limits, and duplicate JSON provenance except for the duplicate empty JSON object `session_input_callback` provenance edge: full Python-style object-identity parity is not safely achievable under the current Rust by-value callback API without an in-band marker or public API change. |
| `test_session_exceptions` | `covered` | `crates/agents-core/src/run.rs` | Session-owned persistence rejects conflicting explicit conversation-tracking settings. |
| `test_session_limit` | `covered` | `crates/agents-core/src/memory/util.rs`, `crates/agents-core/src/memory/session.rs` | Session limit helpers and MemorySession default-limit behavior are covered. |

### Model Settings / Providers

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `model_settings/test_serialization` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_any_llm_model` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_deepseek_reasoning_content` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_default_models` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_kwargs_functionality` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_litellm_chatcompletions_stream` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_litellm_extra_body` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_litellm_logging_patch` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_litellm_user_agent` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_map` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |
| `models/test_reasoning_content_replay_hook` | `omitted-with-rationale` | `n/a` | Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered. |

### OpenAI

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_openai_chatcompletions` | `covered` | `crates/agents-openai/src/models.rs` | Chat Completions payload shaping, tool choice defaults, logprobs, and response parsing are covered. |
| `test_openai_chatcompletions_converter` | `omitted-with-rationale` | `n/a` | OpenAI-specific parity for this upstream family remains open; leave it omitted until the corresponding provider/runtime behavior and tests ship. |
| `test_openai_chatcompletions_stream` | `covered` | `crates/agents-openai/src/models/chatcmpl_stream_handler.rs` | Chat Completions streaming delta ordering and final assembly are covered. |
| `test_openai_conversations_session` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Session state load/save, clear behavior, remote bootstrap, and runner continuity are covered. |
| `test_openai_responses` | `covered` | `crates/agents-openai/src/models.rs`, `crates/agents-openai/src/websocket.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Responses payload shaping, output conversion, conversation tracking, websocket framing, and response parsing are covered. |
| `test_openai_responses_converter` | `omitted-with-rationale` | `n/a` | OpenAI-specific parity for this upstream family remains open; leave it omitted until the corresponding provider/runtime behavior and tests ship. |
| `test_responses` | `omitted-with-rationale` | `n/a` | OpenAI-specific parity for this upstream family remains open; leave it omitted until the corresponding provider/runtime behavior and tests ship. |
| `test_responses_tracing` | `omitted-with-rationale` | `n/a` | OpenAI-specific parity for this upstream family remains open; leave it omitted until the corresponding provider/runtime behavior and tests ship. |
| `test_responses_websocket_session` | `covered` | `crates/agents-openai/src/websocket.rs` | Responses websocket URL building, headers, query handling, and request framing are covered. |
| `test_server_conversation_tracker` | `omitted-with-rationale` | `n/a` | Rust covers sent-state rewind and filtered delta replay except when `call_model_input_filter` drops/reorders siblings and returns multiple fresh replacement items: the current callback API returns only `ModelInputData`, so exact source mapping is not safely recoverable without a provenance-aware API channel. |

### MCP

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `mcp/test_caching` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_client_session_retries` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_connect_disconnect` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_mcp_approval` | `covered` | `crates/agents-core/src/mcp/util.rs` | `requires_approval` mapping to approval-gated MCP runtime tools is covered. |
| `mcp/test_mcp_auth_params` | `covered` | `crates/agents-core/src/mcp/server.rs` | SSE and streamable-HTTP auth/configuration handling are covered. |
| `mcp/test_mcp_resources` | `covered` | `crates/agents-core/src/mcp/server.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | Connection-gated resource listing, template listing, and resource reads are covered. |
| `mcp/test_mcp_server_manager` | `covered` | `crates/agents-core/src/mcp/manager.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | Connect, reconnect, deduplicated failures, active tool listing, and cleanup state are covered. |
| `mcp/test_mcp_tracing` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_mcp_util` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_message_handler` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_prompt_server` | `omitted-with-rationale` | `n/a` | MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage. |
| `mcp/test_runner_calls_mcp` | `covered` | `crates/openai-agents/tests/mcp_semantics.rs` | Non-streamed and streamed MCP tool execution through the runner are covered. |
| `mcp/test_server_errors` | `covered` | `crates/openai-agents/tests/mcp_semantics.rs` | Unknown, duplicate, and failing MCP tool paths fail clearly through the shared runner. |
| `mcp/test_streamable_http_client_factory` | `covered` | `crates/agents-core/src/mcp/server.rs` | Streamable-HTTP client-factory hooks influence the real transport client and are covered. |
| `mcp/test_streamable_http_session_id` | `covered` | `crates/agents-core/src/mcp/server.rs` | Streamable-HTTP session-id allocation, pinning, and reset semantics are covered. |
| `mcp/test_tool_filtering` | `covered` | `crates/agents-core/src/mcp/util.rs` | Static/callable tool filters, metadata resolution, and approval mapping are covered. |

### Realtime

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `realtime/test_agent` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_audio_formats_unit` | `covered` | `crates/agents-realtime/src/audio_formats.rs` | Realtime audio-format normalization covers known and custom format values. |
| `realtime/test_conversion_helpers` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_ga_session_update_normalization` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_item_parsing` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_model_events` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_openai_realtime` | `covered` | `crates/agents-realtime/src/openai_realtime.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Websocket and SIP model behavior, event-type normalization, and session updates are covered. |
| `realtime/test_openai_realtime_conversions` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_openai_realtime_sip_model` | `covered` | `crates/agents-realtime/src/openai_realtime.rs` | SIP realtime connect/send/update/disconnect requirements and call-id validation are covered. |
| `realtime/test_playback_tracker` | `covered` | `crates/agents-realtime/src/_default_tracker.rs` | Playback sample accumulation, derived duration, and reset behavior are covered. |
| `realtime/test_playback_tracker_manual_unit` | `covered` | `crates/agents-realtime/src/_default_tracker.rs` | Manual playback tracker state transitions are covered alongside the core tracker unit tests. |
| `realtime/test_realtime_handoffs` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_realtime_model_settings` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_runner` | `covered` | `crates/agents-realtime/src/runner.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Session creation, run-config model settings, and live session commands are covered. |
| `realtime/test_session` | `covered` | `crates/agents-realtime/src/session.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Live event streaming, lifecycle transitions, model-setting state, playback state, interrupts, and shutdown are covered. |
| `realtime/test_session_payload_and_formats` | `covered` | `crates/agents-realtime/src/openai_realtime.rs` | GA-style session payload normalization, transcription rejection, and audio-format extraction are covered. |
| `realtime/test_tracing` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |
| `realtime/test_twilio_sip_server` | `omitted-with-rationale` | `n/a` | Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist. |

### Voice

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `voice/test_input` | `covered` | `crates/agents-voice/src/input.rs` | Buffered and streamed voice input normalization into runtime transport formats is covered. |
| `voice/test_openai_stt` | `covered` | `crates/agents-voice/src/models/openai_stt.rs` | OpenAI STT handshake, completion, timeout, and error semantics are covered. |
| `voice/test_openai_tts` | `covered` | `crates/agents-voice/src/models/openai_tts.rs` | OpenAI TTS default/custom voice settings and runtime synthesis behavior are covered. |
| `voice/test_pipeline` | `covered` | `crates/agents-voice/src/pipeline.rs`, `crates/agents-voice/src/result.rs`, `crates/openai-agents/tests/voice_semantics.rs` | Live streamed audio results, transcript events, session lifecycle events, and streamed audio input are covered. |
| `voice/test_workflow` | `covered` | `crates/agents-voice/src/workflow.rs`, `crates/openai-agents/tests/voice_semantics.rs` | Single-agent workflow state, streamed core-runner output, and transcript extraction are covered. |

### Tracing

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `test_trace_processor` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `test_tracing` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `test_tracing_errors` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `test_tracing_errors_streamed` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `test_tracing_provider_safe_debug` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_import_side_effects` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_logger` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_processor_api_key` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_set_api_key_fix` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_setup` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_trace_context` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_traces_impl` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |
| `tracing/test_tracing_env_disable` | `omitted-with-rationale` | `n/a` | Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet. |

### Extensions

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `extensions/experiemental/codex/test_codex_exec_thread` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/experiemental/codex/test_codex_tool` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_advanced_sqlite_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_async_sqlite_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_dapr_redis_integration` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_dapr_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_encrypt_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_redis_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/memory/test_sqlalchemy_session` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `extensions/test_tool_output_trimmer` | `covered` | `crates/agents-extensions/src/extensions/tool_output_trimmer.rs` | Old tool outputs are trimmed while recent turns remain intact. |
| `test_extension_filters` | `omitted-with-rationale` | `n/a` | Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now. |
| `test_visualization` | `covered` | `crates/agents-extensions/src/extensions/visualization.rs` | DOT graph generation for tools and handoffs is covered. |

### JS Package Families

| Family | Status | Rust coverage | Notes |
| --- | --- | --- | --- |
| `js/agents/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents/metadata` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/agent` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/agentScenarios` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/agentToolInput` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/createSpans` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/defaultModel` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/errors` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/events` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/extensions/handoffFilters` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/extensions/handoffPrompt` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/guardrail` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/handoff` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/handoffs` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/helpers/message` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/hitlMemorySessionScenario` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/items` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/lifecycle` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/logger` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcp` | `covered` | `crates/agents-core/src/mcp/manager.rs`, `crates/agents-core/src/mcp/util.rs`, `crates/openai-agents/tests/mcp_semantics.rs` | JS MCP server, filter, and runner integration behavior maps to the Rust MCP runtime. |
| `js/agents-core/mcpCache` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcpProtocolCancellation` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcpServers` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcpToFunctionTool` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcpToolFilter.integration` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/mcpToolFilter` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/memorySession` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/metadata` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/model` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/providers` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/result` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/retryPolicy` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/run.stream` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | JS streamed-run behavior maps to the shared Rust runner. |
| `js/agents-core/run` | `covered` | `crates/agents-core/src/run.rs`, `crates/openai-agents/tests/runner_semantics.rs` | JS core run behavior maps to the shared Rust runner. |
| `js/agents-core/run.utils` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runContext` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runState` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/conversation` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/guardrails` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/items.helpers` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/mcpApprovals` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/modelOutputs` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/modelSettings` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/sessionPersistence.extended` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/sessionPersistence` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/toolExecution` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/toolUseTracker` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/tracing` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/runner/turnResolution` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/shims/browser-shims` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/shims/mcp-server/browser` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/shims/mcp-server/node` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/shims/mcp-server/streamableHttpRetry` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/tool` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/toolIdentity` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/tooling` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/tracing` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/usage` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/abortSignals` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/applyDiff` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/base64` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/binary` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/messages` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/safeExecute` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/serialize` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/smartString` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/tools` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/typeGuards` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-core/utils/zodJsonSchemaCompat` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/CloudflareRealtimeTransport` | `covered` | `crates/agents-extensions/src/extensions/realtime_transports.rs`, `crates/openai-agents/tests/extensions_semantics.rs` | Cloudflare Workers websocket-upgrade request shaping and accept flow are covered. |
| `js/agents-extensions/TwilioRealtimeTransport` | `covered` | `crates/agents-extensions/src/extensions/realtime_transports.rs`, `crates/openai-agents/tests/extensions_semantics.rs` | Twilio media-stream normalization, interruption timing, and outbound mark generation are covered. |
| `js/agents-extensions/ai-sdk/GoogleFormat` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/ai-sdk/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/ai-sdk-ui/textStream` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/ai-sdk-ui/uiMessageStream` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/experimental/codex/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-extensions/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/defaults` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/hitlOpenAIConversationsSession` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiChatCompletionsConverter` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiChatCompletionsModel.scenario` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiChatCompletionsModel` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiChatCompletionsStreaming` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiConversationsSession` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Conversation-backed session bootstrap and continuity are covered. |
| `js/agents-openai/openaiProvider` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiResponsesCompactionSession` | `covered` | `crates/agents-openai/src/memory.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Compaction sessions and threshold-aware previous-response behavior are covered. |
| `js/agents-openai/openaiResponsesModel.helpers` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiResponsesModel` | `covered` | `crates/agents-openai/src/models.rs`, `crates/openai-agents/tests/openai_session_semantics.rs` | Responses model request/response shaping and continuity are covered. |
| `js/agents-openai/openaiResponsesWSModel` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/openaiTracingExporter` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/rawModelEvents` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/responsesTransportUtils` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/responsesWebSocketSession` | `covered` | `crates/agents-openai/src/websocket.rs` | Responses websocket session shaping and framing are covered. |
| `js/agents-openai/tools` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-openai/utils/providerData` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/guardrail` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/index` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/openaiRealtimeBase` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/openaiRealtimeEvents` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/openaiRealtimeSip` | `covered` | `crates/agents-realtime/src/openai_realtime.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Realtime SIP model behavior maps to the Rust realtime transport model. |
| `js/agents-realtime/openaiRealtimeWebRtc.environment` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/openaiRealtimeWebRtc` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/openaiRealtimeWebsocket` | `covered` | `crates/agents-realtime/src/openai_realtime.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | Realtime websocket session updates and event normalization are covered. |
| `js/agents-realtime/realtimeAgentHandoffs` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/realtimeSession` | `covered` | `crates/agents-realtime/src/runner.rs`, `crates/agents-realtime/src/session.rs`, `crates/openai-agents/tests/realtime_semantics.rs` | JS realtime session behavior maps to the Rust realtime session runtime. |
| `js/agents-realtime/realtimeVoiceConfigRegression` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/tool` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
| `js/agents-realtime/utils` | `omitted-with-rationale` | `n/a` | This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted. |
