//! Public facade for the Rust port of the OpenAI Agents SDK.

pub use agents_core::VERSION as __version__;
pub use agents_core::{
    Agent, AgentAsToolInput, AgentAsToolOptions, AgentBase, AgentBuilder, AgentHookContext,
    AgentHooks, AgentOutputSchema, AgentOutputSchemaBase, AgentRunner, AgentSpanData,
    AgentToolFailureFormatter, AgentToolInvocation, AgentToolOutputExtractor, AgentToolRunResult,
    AgentToolStreamEvent, AgentToolStreamHandler, AgentUpdatedStreamEvent, AgentsError,
    AgentsException, ApplyPatchEditor, ApplyPatchOperation, ApplyPatchResult, ApplyPatchTool,
    AsyncComputer, Button, CURRENT_RUN_STATE_SCHEMA_VERSION, CompactionItem, Computer,
    ComputerProvider, ComputerTool, CustomSpanData, DEFAULT_CONVERSATION_HISTORY_END,
    DEFAULT_CONVERSATION_HISTORY_START, DebugSettings, DocstringStyle, DynamicPromptFunction,
    Environment, FunctionSchema, FunctionSpanData, FunctionTool, FunctionToolResult,
    GenerateDynamicPromptData, GenerationSpanData, GuardrailFunctionOutput, GuardrailSpanData,
    Handoff, HandoffBuilder, HandoffCallItem, HandoffHistoryMapper, HandoffInputData,
    HandoffInputFilter, HandoffOutputItem, HandoffSpanData, HostedMCPTool, InputGuardrail,
    InputGuardrailResult, InputGuardrailTripwireTriggered, InputItem, ItemHelpers,
    LocalShellCommandRequest, LocalShellExecutor, LocalShellTool, MCPApprovalRequestItem,
    MCPApprovalResponseItem, MCPListToolsSpanData, MCPToolApprovalFunction,
    MCPToolApprovalFunctionResult, MCPToolApprovalRequest, MaxTurnsExceeded, MaybeAwaitable,
    MemorySession, MessageOutputItem, Model, ModelBehaviorError, ModelProvider, ModelRequest,
    ModelResponse, ModelRetryAdvice, ModelRetryAdviceRequest, ModelRetryBackoffSettings,
    ModelRetryNormalizedError, ModelRetrySettings, ModelSettings, ModelTracing, MultiProvider,
    MultiProviderMap, MultiProviderOpenAIPrefixMode, MultiProviderUnknownPrefixMode,
    OpenAIResponsesCompactionArgs, OpenAIResponsesCompactionAwareSession, OutputGuardrail,
    OutputGuardrailResult, OutputGuardrailTripwireTriggered, OutputItem, Prompt, PromptSpec,
    PromptUtil, RawResponsesStreamEvent, ReasoningItem, ReasoningItemIdPolicy, ReasoningSettings,
    ResponseSpanData, Result, RetryDecision, RetryPolicy, RetryPolicyContext, RunConfig,
    RunContext, RunContextWrapper, RunErrorData, RunErrorDetails, RunErrorHandler,
    RunErrorHandlerInput, RunErrorHandlerResult, RunErrorHandlers, RunHooks, RunInterruption,
    RunInterruptionKind, RunItem, RunItemStreamEvent, RunOptions, RunResult, RunResultStreaming,
    RunState, Runner, SQLiteSession, SdkConfig, Session, SessionABC, SessionInputCallback,
    SessionSettings, SharedAgentHooks, SharedRunHooks, ShellActionRequest, ShellCallData,
    ShellCallOutcome, ShellCommandOutput, ShellCommandRequest, ShellExecutor, ShellResult,
    ShellTool, ShellToolContainerAutoEnvironment, ShellToolContainerNetworkPolicy,
    ShellToolContainerNetworkPolicyAllowlist, ShellToolContainerNetworkPolicyDisabled,
    ShellToolContainerNetworkPolicyDomainSecret, ShellToolContainerReferenceEnvironment,
    ShellToolContainerSkill, ShellToolEnvironment, ShellToolHostedEnvironment,
    ShellToolInlineSkill, ShellToolInlineSkillSource, ShellToolLocalEnvironment,
    ShellToolLocalSkill, ShellToolSkillReference, Span, SpanData, SpanError, SpeechGroupSpanData,
    SpeechSpanData, StaticTool, StopAtTools, StreamEvent, StructuredInputSchemaInfo,
    StructuredToolInputBuilder, TResponseInputItem, ToInputListMode, Tool, ToolApprovalItem,
    ToolCall, ToolCallItem, ToolCallOutputItem, ToolContext, ToolDefinition, ToolErrorFormatter,
    ToolErrorFormatterArgs, ToolGuardrailBehavior, ToolGuardrailFunctionOutput, ToolInputGuardrail,
    ToolInputGuardrailData, ToolInputGuardrailResult, ToolInputGuardrailTripwireTriggered,
    ToolOutput, ToolOutputFileContent, ToolOutputGuardrail, ToolOutputGuardrailData,
    ToolOutputGuardrailResult, ToolOutputGuardrailTripwireTriggered, ToolOutputImage,
    ToolOutputText, ToolTimeoutError, ToolUseBehavior, ToolsToFinalOutputFunction,
    ToolsToFinalOutputResult, Trace, TracingProcessor, TranscriptionSpanData, Usage, UserError,
    VERSION, add_trace_processor, agent_span, apply_diff, attach_error_to_current_span,
    attach_error_to_span, custom_span, default_handoff_history_mapper, default_tool_error_function,
    default_tool_input_builder, dispose_resolved_computers, drop_agent_tool_run_result,
    enable_verbose_stdout_logging, flush_traces, function_span, function_tool, gen_group_id,
    gen_span_id, gen_trace_id, generation_span, get_agent_tool_state_scope,
    get_conversation_history_wrappers, get_current_span, get_current_trace,
    get_default_agent_runner, get_default_model, get_default_model_settings,
    gpt_5_reasoning_settings_required, guardrail_span, handoff, handoff_span, input_guardrail,
    is_gpt_5_default, is_openai_responses_compaction_aware_session, mcp_tools_span,
    nest_handoff_history, nest_handoff_history_with_mapper, noop_coroutine, output_guardrail,
    peek_agent_tool_run_result, pretty_print_result, pretty_print_run_error_details,
    pretty_print_run_result_streaming, record_agent_tool_run_result,
    reset_conversation_history_wrappers, resolve_computer, retry_policies, run, run_demo_loop,
    run_streamed, run_streamed_with_options, run_sync, run_sync_with_options, run_with_options,
    run_with_session, set_agent_tool_state_scope, set_conversation_history_wrappers,
    set_default_agent_runner, set_default_openai_responses_transport, set_trace_processors,
    set_trace_provider, set_tracing_disabled, speech_group_span, speech_span, tool_input_guardrail,
    tool_namespace, tool_output_guardrail, trace, transcription_span,
    transform_string_function_style, validate_json,
};
pub use agents_openai::{
    ChatCmplHelpers, ChatCmplStreamHandler, CodeInterpreterTool, Converter, FAKE_RESPONSES_ID,
    FileSearchTool, ImageGenerationTool, OPENAI_DEFAULT_BASE_URL,
    OPENAI_DEFAULT_WEBSOCKET_BASE_URL, OpenAIApi, OpenAIChatCompletionsModel, OpenAIClientOptions,
    OpenAIConversationsSession, OpenAIProvider, OpenAIResponsesCompactionMode,
    OpenAIResponsesCompactionSession, OpenAIResponsesModel, OpenAIResponsesTransport,
    OpenAIResponsesWSModel, OpenAIResponsesWsModel, Part, ReasoningContentReplayContext,
    ReasoningContentSource, ResponsesWebSocketSession, SequenceNumber, StreamingState,
    ToolSearchTool, WebSearchTool, code_interpreter_tool, default_openai_api,
    default_openai_base_url, default_openai_key, default_openai_websocket_base_url,
    default_should_replay_reasoning_content, fake_id, file_search_tool, get_default_openai_client,
    get_default_openai_key, get_default_openai_websocket_base_url, get_openai_base_url,
    get_openai_retry_advice, get_use_responses_by_default, get_use_responses_websocket_by_default,
    image_generation_tool, provider_managed_retries_disabled, responses_websocket_session,
    set_default_openai_api, set_default_openai_client, set_default_openai_key,
    set_default_openai_key_shared, set_default_openai_websocket_base_url, set_openai_base_url,
    set_tracing_export_api_key, set_use_responses_by_default,
    set_use_responses_websocket_by_default, should_disable_provider_managed_retries,
    should_disable_websocket_pre_event_retries, tool_search_tool, tracing_export_api_key,
    web_search_tool, websocket_pre_event_retries_disabled,
};

pub mod realtime {
    pub use agents_realtime::*;
}

pub mod voice {
    pub use agents_voice::*;
}

pub mod extensions {
    pub use agents_extensions::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn facade_run_uses_core_runner() {
        let agent = Agent::builder("assistant")
            .instructions("Be brief.")
            .build();

        let result = run(&agent, "hello").await.expect("run should succeed");

        assert_eq!(result.agent_name, "assistant");
        assert_eq!(result.final_output.as_deref(), Some("hello"));
    }
}
