//! Core abstractions for the Rust port of the OpenAI Agents SDK.

pub mod _config;
pub mod _debug;
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
pub mod handoff;
pub(crate) mod internal;
pub mod items;
pub mod lifecycle;
pub mod logger;
pub mod model;
pub mod prompts;
pub mod result;
pub mod retry;
pub mod run;
pub mod run_config;
pub mod run_context;
pub mod run_state;
pub mod session;
pub mod stream_events;
pub mod strict_schema;
pub mod tool;
pub mod tool_context;
pub mod tool_guardrails;
pub mod tracing;
pub mod usage;
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
pub use agent::{Agent, AgentBuilder};
pub use agent_output::{AgentOutputSchema, AgentOutputSchemaBase};
pub use agent_tool_input::{
    AgentAsToolInput, ResolvedToolInput, StructuredInputSchemaInfo, default_tool_input_builder,
    resolve_agent_tool_input,
};
pub use agent_tool_state::{
    consume_agent_tool_run_result, drop_agent_tool_run_result, get_agent_tool_state_scope,
    peek_agent_tool_run_result, record_agent_tool_run_result, set_agent_tool_state_scope,
};
pub use apply_diff::apply_diff;
pub use computer::Computer;
pub use config::SdkConfig;
pub use debug::DebugSettings;
pub use editor::{ApplyPatchOperation, ApplyPatchResult, Editor};
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
pub use handoff::Handoff;
pub use items::{InputItem, OutputItem, RunItem};
pub use lifecycle::{AgentHooks, RunHooks};
pub use model::{Model, ModelProvider, ModelRequest, ModelResponse};
pub use prompts::{
    DynamicPromptFunction, GenerateDynamicPromptData, Prompt, PromptSpec, PromptUtil,
};
pub use result::RunResult;
pub use retry::{
    ModelRetryAdvice, ModelRetryAdviceRequest, ModelRetryBackoffSettings,
    ModelRetryNormalizedError, ModelRetrySettings, RetryDecision, RetryPolicyContext,
};
pub use run::{Runner, run};
pub use run_config::{
    CallModelData, DEFAULT_MAX_TURNS, ModelInputData, ReasoningItemIdPolicy, RunConfig, RunOptions,
    ToolErrorFormatterArgs,
};
pub use run_context::{AgentHookContext, ApprovalRecord, RunContext, RunContextWrapper};
pub use run_state::{
    CURRENT_RUN_STATE_SCHEMA_VERSION, RunInterruption, RunInterruptionKind, RunState,
    RunStateContextSnapshot,
};
pub use session::{MemorySession, Session};
pub use stream_events::{
    AgentUpdatedStreamEvent, RawResponsesStreamEvent, RunItemStreamEvent, StreamEvent,
};
pub use strict_schema::ensure_strict_json_schema;
pub use tool::{
    FunctionTool, FunctionToolResult, StaticTool, Tool, ToolDefinition, ToolOutput,
    ToolOutputFileContent, ToolOutputImage, ToolOutputText, function_tool,
};
pub use tool_context::{ToolCall, ToolContext};
pub use tool_guardrails::{
    ToolGuardrailBehavior, ToolGuardrailFunctionOutput, ToolInputGuardrail, ToolInputGuardrailData,
    ToolInputGuardrailResult, ToolOutputGuardrail, ToolOutputGuardrailData,
    ToolOutputGuardrailResult, tool_input_guardrail, tool_output_guardrail,
};
pub use tracing::{Span, Trace};
pub use usage::Usage;
pub use version::VERSION;
