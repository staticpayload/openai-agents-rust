//! Core abstractions for the Rust port of the OpenAI Agents SDK.

pub mod _config;
pub mod _debug;
pub mod _mcp_tool_metadata;
pub mod _tool_identity;
pub mod agent;
pub mod agent_output;
pub mod agent_tool_input;
pub mod agent_tool_state;
pub mod apply_diff;
pub mod computer;
pub mod config;
pub mod debug;
pub mod editor;
pub mod errors;
pub mod exceptions;
pub mod function_schema;
pub mod guardrail;
#[path = "handoff/mod.rs"]
pub mod handoff;
pub(crate) mod internal;
pub mod items;
pub mod lifecycle;
pub mod logger;
pub mod mcp;
pub mod memory;
#[path = "model/mod.rs"]
pub mod model;
pub mod model_settings;
pub mod models;
pub mod prompts;
pub mod repl;
pub mod result;
pub mod retry;
pub mod run;
pub mod run_config;
pub mod run_context;
pub mod run_error_handlers;
pub mod run_state;
pub mod session;
pub mod stream_events;
pub mod strict_schema;
pub mod tool;
pub mod tool_context;
pub mod tool_guardrails;
#[path = "tracing/mod.rs"]
pub mod tracing;
pub mod usage;
pub mod util;
pub mod version;

pub use _config::{
    DefaultOpenAIApi, DefaultOpenAIResponsesTransport, default_openai_api, default_openai_key,
    default_openai_responses_transport, default_tracing_export_api_key, set_default_openai_api,
    set_default_openai_key, set_default_openai_responses_transport,
    set_default_tracing_export_api_key,
};
pub use _debug::{
    debug_flag_enabled, dont_log_model_data, dont_log_tool_data, load_dont_log_model_data,
    load_dont_log_tool_data,
};
pub use _tool_identity::{
    FunctionToolLookupKey, build_function_tool_lookup_map, get_function_tool_approval_keys,
    get_function_tool_lookup_key, get_function_tool_lookup_key_for_call,
    get_function_tool_lookup_key_for_definition, get_function_tool_lookup_keys,
    get_function_tool_qualified_name, get_function_tool_trace_name, get_tool_call_name,
    get_tool_call_namespace, get_tool_call_qualified_name, get_tool_call_trace_name,
    is_reserved_synthetic_tool_namespace, tool_qualified_name, tool_trace_name,
    validate_function_tool_namespace_shape,
};
pub use agent::{
    Agent, AgentAsToolOptions, AgentBase, AgentBuilder, AgentToolFailureFormatter,
    AgentToolOutputExtractor, AgentToolRunResult, AgentToolStreamEvent, AgentToolStreamHandler,
    StopAtTools, StructuredToolInputBuilder, ToolUseBehavior, ToolsToFinalOutputFunction,
    ToolsToFinalOutputResult,
};
pub use agent_output::{AgentOutputSchema, AgentOutputSchemaBase, OutputSchemaDefinition};
pub use agent_tool_input::{
    AgentAsToolInput, ResolvedToolInput, StructuredInputSchemaInfo, default_tool_input_builder,
    resolve_agent_tool_input,
};
pub use agent_tool_state::{
    consume_agent_tool_run_result, drop_agent_tool_run_result, get_agent_tool_state_scope,
    peek_agent_tool_run_result, record_agent_tool_run_result, set_agent_tool_state_scope,
};
pub use apply_diff::apply_diff;
pub use computer::{AsyncComputer, Button, Computer, Environment};
pub use config::SdkConfig;
pub use debug::DebugSettings;
pub use editor::{ApplyPatchEditor, ApplyPatchOperation, ApplyPatchResult, Editor};
pub use errors::AgentsError as AgentsException;
pub use errors::{AgentsError, Result};
pub use exceptions::{
    InputGuardrailTripwireTriggered, MaxTurnsExceeded, ModelBehaviorError,
    OutputGuardrailTripwireTriggered, RunErrorDetails, ToolInputGuardrailTripwireTriggered,
    ToolOutputGuardrailTripwireTriggered, ToolTimeoutError, UserError,
};
pub use function_schema::{DocstringStyle, FunctionSchema};
pub use guardrail::{
    GuardrailFunctionOutput, InputGuardrail, InputGuardrailResult, OutputGuardrail,
    OutputGuardrailResult, input_guardrail, output_guardrail,
};
pub use handoff::{
    DEFAULT_CONVERSATION_HISTORY_END, DEFAULT_CONVERSATION_HISTORY_START, Handoff, HandoffBuilder,
    HandoffHistoryMapper, HandoffInputData, HandoffInputFilter, default_handoff_history_mapper,
    get_conversation_history_wrappers, handoff, nest_handoff_history,
    nest_handoff_history_with_mapper, reset_conversation_history_wrappers,
    set_conversation_history_wrappers,
};
pub use items::{
    CompactionItem, HandoffCallItem, HandoffOutputItem, InputItem, ItemHelpers,
    MCPApprovalRequestItem, MCPApprovalResponseItem, MessageOutputItem, OutputItem, ReasoningItem,
    RunItem, TResponseInputItem, ToolApprovalItem, ToolCallItem, ToolCallOutputItem,
};
pub use lifecycle::{AgentHooks, RunHooks, SharedAgentHooks, SharedRunHooks};
pub use logger::{LOGGER_TARGET, enable_verbose_stdout_logging};
pub use mcp::{
    MCPBlobResourceContents, MCPListResourceTemplatesResult, MCPListResourcesResult,
    MCPReadResourceResult, MCPResource, MCPResourceContents, MCPResourceTemplate, MCPServer,
    MCPServerManager, MCPServerSse, MCPServerSseParams, MCPServerStdio, MCPServerStdioParams,
    MCPServerStreamableHttp, MCPServerStreamableHttpParams, MCPTextResourceContents, MCPTool,
    MCPToolAnnotations, MCPToolMetaContext, MCPToolMetaResolver, MCPTransportAuth,
    MCPTransportClientConfig, MCPTransportClientFactory, MCPTransportKind, MCPUtil,
    RequireApprovalObject, RequireApprovalToolList, ToolFilter, ToolFilterCallable,
    ToolFilterContext, ToolFilterStatic, create_static_tool_filter,
};
pub use memory::Session as SessionABC;
pub use memory::{
    MemorySession, OpenAIConversationAwareSession, OpenAIConversationSessionState,
    OpenAIResponsesCompactionArgs, OpenAIResponsesCompactionAwareSession, SQLiteSession, Session,
    SessionSettings, is_openai_conversation_aware_session,
    is_openai_responses_compaction_aware_session,
};
pub use model::{
    Model, ModelProvider, ModelRequest, ModelResponse, ModelTracing, get_default_model,
    get_default_model_settings, gpt_5_reasoning_settings_required, is_gpt_5_default,
};
pub use model_settings::{ModelSettings, ReasoningSettings};
pub use models::{
    MultiProvider, MultiProviderMap, MultiProviderOpenAIPrefixMode, MultiProviderUnknownPrefixMode,
};
pub use prompts::{
    DynamicPromptFunction, GenerateDynamicPromptData, Prompt, PromptSpec, PromptUtil,
};
pub use repl::run_demo_loop;
pub use result::{AgentToolInvocation, RunResult, RunResultStreaming, ToInputListMode};
pub use retry::{
    ModelRetryAdvice, ModelRetryAdviceRequest, ModelRetryBackoffSettings,
    ModelRetryNormalizedError, ModelRetrySettings, RetryDecision, RetryPolicy, RetryPolicyContext,
    retry_policies,
};
pub use run::{
    AgentRunner, Runner, get_default_agent_runner, run, run_streamed, run_streamed_with_options,
    run_sync, run_sync_with_options, run_with_options, run_with_session, set_default_agent_runner,
};
pub use run_config::{
    CallModelData, DEFAULT_MAX_TURNS, ModelInputData, ReasoningItemIdPolicy, RunConfig, RunOptions,
    SessionInputCallback, ToolErrorFormatter, ToolErrorFormatterArgs,
};
pub use run_context::{AgentHookContext, ApprovalRecord, RunContext, RunContextWrapper};
pub use run_error_handlers::{
    RunErrorData, RunErrorHandler, RunErrorHandlerInput, RunErrorHandlerResult, RunErrorHandlers,
};
pub use run_state::{
    CURRENT_RUN_STATE_SCHEMA_VERSION, RunInterruption, RunInterruptionKind, RunState,
    RunStateContextSnapshot,
};
pub use stream_events::{
    AgentUpdatedStreamEvent, RawResponsesStreamEvent, RunItemStreamEvent, StreamEvent,
};
pub use strict_schema::ensure_strict_json_schema;
pub use tool::{
    ApplyPatchTool, ComputerProvider, ComputerTool, FunctionTool, FunctionToolResult,
    HostedMCPTool, LocalShellCommandRequest, LocalShellExecutor, LocalShellTool,
    MCPToolApprovalFunction, MCPToolApprovalFunctionResult, MCPToolApprovalRequest,
    ShellActionRequest, ShellCallData, ShellCallOutcome, ShellCommandOutput, ShellCommandRequest,
    ShellExecutor, ShellResult, ShellTool, ShellToolContainerAutoEnvironment,
    ShellToolContainerNetworkPolicy, ShellToolContainerNetworkPolicyAllowlist,
    ShellToolContainerNetworkPolicyDisabled, ShellToolContainerNetworkPolicyDomainSecret,
    ShellToolContainerReferenceEnvironment, ShellToolContainerSkill, ShellToolEnvironment,
    ShellToolHostedEnvironment, ShellToolInlineSkill, ShellToolInlineSkillSource,
    ShellToolLocalEnvironment, ShellToolLocalSkill, ShellToolSkillReference, StaticTool, Tool,
    ToolDefinition, ToolOutput, ToolOutputFileContent, ToolOutputImage, ToolOutputText,
    default_tool_error_function, dispose_resolved_computers, function_tool, resolve_computer,
    tool_namespace,
};
pub use tool_context::{ToolCall, ToolContext};
pub use tool_guardrails::{
    ToolGuardrailBehavior, ToolGuardrailFunctionOutput, ToolInputGuardrail, ToolInputGuardrailData,
    ToolInputGuardrailResult, ToolOutputGuardrail, ToolOutputGuardrailData,
    ToolOutputGuardrailResult, tool_input_guardrail, tool_output_guardrail,
};
pub use tracing::{
    AgentSpanData, CustomSpanData, FunctionSpanData, GenerationSpanData, GuardrailSpanData,
    HandoffSpanData, MCPListToolsSpanData, ResponseSpanData, Span, SpanData, SpanError,
    SpeechGroupSpanData, SpeechSpanData, Trace, TracingProcessor, TranscriptionSpanData,
    add_trace_processor, agent_span, custom_span, flush_traces, function_span, gen_group_id,
    gen_span_id, gen_trace_id, generation_span, get_current_span, get_current_trace,
    guardrail_span, handoff_span, mcp_tools_span, set_trace_processors, set_trace_provider,
    set_tracing_disabled, speech_group_span, speech_span, trace, transcription_span,
};
pub use usage::Usage;
pub use util::{
    MaybeAwaitable, attach_error_to_current_span, attach_error_to_span,
    evaluate_needs_approval_setting, noop_coroutine, pretty_print_result,
    pretty_print_run_error_details, pretty_print_run_result_streaming,
    transform_string_function_style, validate_json,
};
pub use version::VERSION;
