//! Core abstractions for the Rust port of the OpenAI Agents SDK.

pub mod _config;
pub mod _debug;
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
pub mod guardrail;
pub mod handoff;
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
    InputGuardrailTripwireTriggered, MaxTurnsExceeded, ModelBehaviorError, RunErrorDetails,
    ToolTimeoutError, UserError,
};
pub use guardrail::{InputGuardrail, OutputGuardrail};
pub use handoff::Handoff;
pub use items::{InputItem, OutputItem};
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
pub use run_state::RunState;
pub use session::{MemorySession, Session};
pub use stream_events::{
    AgentUpdatedStreamEvent, RawResponsesStreamEvent, RunItem, RunItemStreamEvent, StreamEvent,
};
pub use strict_schema::ensure_strict_json_schema;
pub use tool::{StaticTool, Tool, ToolDefinition};
pub use tool_context::{ToolCall, ToolContext};
pub use tracing::{Span, Trace};
pub use usage::Usage;
pub use version::VERSION;
