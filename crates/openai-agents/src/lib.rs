//! Public facade for the Rust port of the OpenAI Agents SDK.

pub use agents_core::{
    Agent, AgentBuilder, AgentsError, ApplyPatchOperation, ApplyPatchResult,
    CURRENT_RUN_STATE_SCHEMA_VERSION, Computer, DebugSettings, DocstringStyle, Editor,
    FunctionSchema, FunctionTool, FunctionToolResult, GuardrailFunctionOutput, Handoff,
    InputGuardrail, InputGuardrailResult, InputItem, MemorySession, Model, ModelProvider,
    ModelRequest, ModelResponse, OutputGuardrail, OutputGuardrailResult, OutputItem, Result,
    RunConfig, RunContext, RunInterruption, RunInterruptionKind, RunResult, RunState, Runner,
    SdkConfig, Session, Span, StaticTool, Tool, ToolCall, ToolContext, ToolDefinition,
    ToolGuardrailBehavior, ToolGuardrailFunctionOutput, ToolInputGuardrail, ToolInputGuardrailData,
    ToolInputGuardrailResult, ToolOutput, ToolOutputFileContent, ToolOutputGuardrail,
    ToolOutputGuardrailData, ToolOutputGuardrailResult, ToolOutputImage, ToolOutputText, Trace,
    Usage, VERSION, apply_diff, function_tool, input_guardrail, output_guardrail, run,
    tool_input_guardrail, tool_output_guardrail,
};
pub use agents_openai::{
    OpenAIApi, OpenAIChatCompletionsModel, OpenAIConversationsSession, OpenAIProvider,
    OpenAIResponsesCompactionMode, OpenAIResponsesCompactionSession, OpenAIResponsesModel,
    OpenAIResponsesWsModel, ResponsesWebSocketSession, code_interpreter_tool, default_openai_api,
    default_openai_key, file_search_tool, image_generation_tool, set_default_openai_api,
    set_default_openai_key, set_tracing_export_api_key, tool_search_tool, tracing_export_api_key,
    web_search_tool,
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
