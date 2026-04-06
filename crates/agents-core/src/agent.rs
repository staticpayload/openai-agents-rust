use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_output::OutputSchemaDefinition;
use crate::agent_tool_input::{
    AgentAsToolInput, ResolvedToolInput, StructuredInputSchemaInfo, resolve_agent_tool_input,
};
use crate::agent_tool_state::record_agent_tool_run_result;
use crate::errors::{AgentsError, Result};
use crate::function_schema::FunctionSchema;
use crate::guardrail::{InputGuardrail, OutputGuardrail};
use crate::handoff::Handoff;
use crate::items::{OutputItem, RunItem};
use crate::lifecycle::SharedAgentHooks;
use crate::mcp::{MCPServer, MCPServerManager, MCPToolMetaResolver, MCPUtil, ToolFilter};
use crate::model_settings::ModelSettings;
use crate::result::{AgentToolInvocation, RunResult, RunResultStreaming};
use crate::run::get_default_agent_runner;
use crate::run_config::RunConfig;
use crate::run_context::{RunContext, RunContextWrapper};
use crate::session::Session;
use crate::stream_events::StreamEvent;
use crate::tool::{FunctionTool, FunctionToolResult, StaticTool, ToolEnabledFunction, ToolOutput};
use crate::tool_context::{ToolCall, ToolContext};

pub type AgentBase = Agent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopAtTools {
    #[serde(default)]
    pub stop_at_tool_names: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolsToFinalOutputResult {
    pub is_final_output: bool,
    pub final_output: Option<Value>,
}

impl ToolsToFinalOutputResult {
    pub fn not_final() -> Self {
        Self {
            is_final_output: false,
            final_output: None,
        }
    }

    pub fn final_output(final_output: Value) -> Self {
        Self {
            is_final_output: true,
            final_output: Some(final_output),
        }
    }
}

pub type ToolsToFinalOutputFunction = Arc<
    dyn Fn(
            RunContextWrapper<RunContext>,
            Vec<FunctionToolResult>,
        ) -> BoxFuture<'static, Result<ToolsToFinalOutputResult>>
        + Send
        + Sync,
>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentToolStreamEvent {
    pub event: StreamEvent,
    pub agent: Agent,
    pub tool_call: Option<ToolCall>,
}

#[derive(Clone, Debug)]
pub enum AgentToolRunResult {
    Run(RunResult),
    Streaming(RunResultStreaming),
}

pub type StructuredToolInputBuilder = Arc<
    dyn Fn(
            Value,
            Option<StructuredInputSchemaInfo>,
        ) -> BoxFuture<'static, Result<ResolvedToolInput>>
        + Send
        + Sync,
>;
pub type AgentToolOutputExtractor =
    Arc<dyn Fn(AgentToolRunResult) -> BoxFuture<'static, Result<String>> + Send + Sync>;
pub type AgentToolStreamHandler =
    Arc<dyn Fn(AgentToolStreamEvent) -> BoxFuture<'static, Result<()>> + Send + Sync>;
pub type AgentToolFailureFormatter =
    Arc<dyn Fn(String) -> BoxFuture<'static, Option<String>> + Send + Sync>;

fn default_agent_tool_failure_formatter() -> AgentToolFailureFormatter {
    Arc::new(|message| async move { Some(format!("Agent tool failed: {message}")) }.boxed())
}

fn default_agent_tool_output_text(result: &RunResult) -> String {
    if let Some(text) = result.final_output.clone().filter(|text| !text.is_empty()) {
        return text;
    }

    for item in result.new_items.iter().rev() {
        match item {
            RunItem::MessageOutput { content } => {
                if let Some(text) = output_item_text(content) {
                    return text;
                }
            }
            RunItem::ToolCallOutput { output, .. } => {
                if let Some(text) = output_item_text(output) {
                    return text;
                }
            }
            RunItem::ToolCall { .. }
            | RunItem::HandoffCall { .. }
            | RunItem::HandoffOutput { .. }
            | RunItem::Reasoning { .. } => {}
        }
    }

    result
        .output
        .iter()
        .rev()
        .find_map(output_item_text)
        .unwrap_or_default()
}

fn output_item_text(item: &OutputItem) -> Option<String> {
    match item {
        OutputItem::Text { text } => Some(text.clone()),
        OutputItem::Json { value } => serde_json::to_string(value).ok(),
        OutputItem::Reasoning { text } => Some(text.clone()),
        OutputItem::ToolCall { .. } | OutputItem::Handoff { .. } => None,
    }
}

#[derive(Clone)]
pub struct AgentAsToolOptions<TArgs = AgentAsToolInput> {
    pub custom_output_extractor: Option<AgentToolOutputExtractor>,
    pub enabled: bool,
    pub is_enabled: Option<ToolEnabledFunction>,
    pub on_stream: Option<AgentToolStreamHandler>,
    pub run_config: Option<RunConfig>,
    pub max_turns: Option<usize>,
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
    pub session: Option<Arc<dyn Session + Sync>>,
    pub failure_error_function: Option<AgentToolFailureFormatter>,
    pub needs_approval: bool,
    pub input_builder: Option<StructuredToolInputBuilder>,
    pub include_input_schema: bool,
    _phantom: PhantomData<TArgs>,
}

impl<TArgs> fmt::Debug for AgentAsToolOptions<TArgs> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentAsToolOptions")
            .field(
                "custom_output_extractor",
                &self.custom_output_extractor.as_ref().map(|_| "<function>"),
            )
            .field("enabled", &self.enabled)
            .field(
                "is_enabled",
                &self.is_enabled.as_ref().map(|_| "<function>"),
            )
            .field("on_stream", &self.on_stream.as_ref().map(|_| "<function>"))
            .field("run_config", &self.run_config)
            .field("max_turns", &self.max_turns)
            .field("previous_response_id", &self.previous_response_id)
            .field("conversation_id", &self.conversation_id)
            .field("session", &self.session.as_ref().map(|_| "<session>"))
            .field(
                "failure_error_function",
                &self.failure_error_function.as_ref().map(|_| "<function>"),
            )
            .field("needs_approval", &self.needs_approval)
            .field(
                "input_builder",
                &self.input_builder.as_ref().map(|_| "<function>"),
            )
            .field("include_input_schema", &self.include_input_schema)
            .finish()
    }
}

impl<TArgs> Default for AgentAsToolOptions<TArgs> {
    fn default() -> Self {
        Self {
            custom_output_extractor: None,
            enabled: true,
            is_enabled: None,
            on_stream: None,
            run_config: None,
            max_turns: None,
            previous_response_id: None,
            conversation_id: None,
            session: None,
            failure_error_function: Some(default_agent_tool_failure_formatter()),
            needs_approval: false,
            input_builder: None,
            include_input_schema: false,
            _phantom: PhantomData,
        }
    }
}

#[derive(Clone, Default)]
pub enum ToolUseBehavior {
    #[default]
    RunLlmAgain,
    StopOnFirstTool,
    StopAtTools(StopAtTools),
    Custom(ToolsToFinalOutputFunction),
}

impl fmt::Debug for ToolUseBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunLlmAgain => f.write_str("ToolUseBehavior::RunLlmAgain"),
            Self::StopOnFirstTool => f.write_str("ToolUseBehavior::StopOnFirstTool"),
            Self::StopAtTools(value) => f
                .debug_tuple("ToolUseBehavior::StopAtTools")
                .field(value)
                .finish(),
            Self::Custom(_) => f.write_str("ToolUseBehavior::Custom(<function>)"),
        }
    }
}

impl ToolUseBehavior {
    pub async fn evaluate(
        &self,
        context: &RunContextWrapper<RunContext>,
        tool_results: &[FunctionToolResult],
    ) -> Result<ToolsToFinalOutputResult> {
        if tool_results.is_empty() {
            return Ok(ToolsToFinalOutputResult::not_final());
        }

        match self {
            Self::RunLlmAgain => Ok(ToolsToFinalOutputResult::not_final()),
            Self::StopOnFirstTool => Ok(ToolsToFinalOutputResult::final_output(
                tool_results[0].final_output_value(),
            )),
            Self::StopAtTools(config) => {
                for result in tool_results {
                    if config.stop_at_tool_names.iter().any(|name| {
                        name == &result.tool_name
                            || result
                                .qualified_name
                                .as_ref()
                                .is_some_and(|qualified_name| qualified_name == name)
                    }) {
                        return Ok(ToolsToFinalOutputResult::final_output(
                            result.final_output_value(),
                        ));
                    }
                }
                Ok(ToolsToFinalOutputResult::not_final())
            }
            Self::Custom(handler) => handler(context.clone(), tool_results.to_vec()).await,
        }
    }
}

/// High-level agent definition.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub handoff_description: Option<String>,
    pub instructions: Option<String>,
    pub output_schema: Option<OutputSchemaDefinition>,
    pub model_settings: Option<ModelSettings>,
    pub tools: Vec<StaticTool>,
    #[serde(skip, default)]
    pub function_tools: Vec<FunctionTool>,
    #[serde(skip, default)]
    pub mcp_servers: Vec<Arc<dyn MCPServer>>,
    #[serde(skip, default)]
    pub mcp_tool_filter: Option<ToolFilter>,
    #[serde(skip, default)]
    pub mcp_tool_meta_resolver: Option<MCPToolMetaResolver>,
    pub handoffs: Vec<Handoff>,
    pub input_guardrails: Vec<InputGuardrail>,
    pub output_guardrails: Vec<OutputGuardrail>,
    pub model: Option<String>,
    #[serde(skip, default)]
    pub hooks: Option<SharedAgentHooks>,
    #[serde(skip, default)]
    pub tool_use_behavior: ToolUseBehavior,
}

impl fmt::Debug for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Agent")
            .field("name", &self.name)
            .field("handoff_description", &self.handoff_description)
            .field("instructions", &self.instructions)
            .field("output_schema", &self.output_schema)
            .field("model_settings", &self.model_settings)
            .field("tools", &self.tools)
            .field("function_tools", &self.function_tools.len())
            .field("mcp_servers", &self.mcp_servers.len())
            .field(
                "mcp_tool_filter",
                &self.mcp_tool_filter.as_ref().map(|_| "<filter>"),
            )
            .field(
                "mcp_tool_meta_resolver",
                &self.mcp_tool_meta_resolver.as_ref().map(|_| "<resolver>"),
            )
            .field("handoffs", &self.handoffs)
            .field("input_guardrails", &self.input_guardrails.len())
            .field("output_guardrails", &self.output_guardrails.len())
            .field("model", &self.model)
            .field("hooks", &self.hooks.as_ref().map(|_| "<hooks>"))
            .field("tool_use_behavior", &self.tool_use_behavior)
            .finish()
    }
}

impl Agent {
    pub fn builder(name: impl Into<String>) -> AgentBuilder {
        AgentBuilder::new(name)
    }

    pub fn tool_definitions(&self) -> Vec<crate::tool::ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    pub async fn get_all_function_tools(
        &self,
        run_context: &RunContextWrapper<RunContext>,
    ) -> Result<Vec<FunctionTool>> {
        let mut tools = self.function_tools.clone();
        if !self.mcp_servers.is_empty() {
            let mut manager = MCPServerManager::new(self.mcp_servers.iter().cloned());
            let mcp_tools = async {
                manager.connect_all().await?;
                MCPUtil::get_all_function_tools_connected(
                    &manager.active_servers(),
                    self.mcp_tool_filter.as_ref(),
                    run_context.clone(),
                    self.clone(),
                    self.mcp_tool_meta_resolver.clone(),
                )
                .await
            }
            .await;
            let cleanup_result = manager.cleanup_all().await;
            if let Err(error) = cleanup_result {
                return Err(error);
            }
            tools.extend(mcp_tools?);
        }

        let mut visible = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for tool in tools {
            if !tool.enabled_for(run_context, self).await {
                continue;
            }

            let qualified_name = tool.qualified_name();
            if !seen.insert(qualified_name.clone()) {
                return Err(AgentsError::message(format!(
                    "duplicate runtime tool name `{qualified_name}` for agent `{}`",
                    self.name
                )));
            }
            visible.push(tool);
        }

        Ok(visible)
    }

    pub async fn runtime_tool_definitions(
        &self,
        run_context: &RunContextWrapper<RunContext>,
    ) -> Result<Vec<crate::tool::ToolDefinition>> {
        Ok(self
            .get_all_function_tools(run_context)
            .await?
            .into_iter()
            .map(|tool| tool.definition.clone())
            .collect())
    }

    pub fn find_function_tool(&self, name: &str, namespace: Option<&str>) -> Option<&FunctionTool> {
        self.function_tools.iter().find(|tool| {
            tool.definition.name == name && tool.definition.namespace.as_deref() == namespace
        })
    }

    pub fn find_handoff(&self, target: &str) -> Option<&Handoff> {
        self.handoffs
            .iter()
            .find(|handoff| handoff.target == target)
    }

    pub fn clone_with<F>(&self, apply: F) -> Self
    where
        F: FnOnce(&mut Self),
    {
        let mut cloned = self.clone();
        apply(&mut cloned);
        cloned
    }

    pub fn as_tool<TArgs>(
        &self,
        tool_name: Option<&str>,
        tool_description: Option<&str>,
        options: AgentAsToolOptions<TArgs>,
    ) -> Result<FunctionTool>
    where
        TArgs: DeserializeOwned + Serialize + JsonSchema + Send + 'static,
    {
        let resolved_tool_name = tool_name
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| crate::util::transform_string_function_style(&self.name));
        let resolved_tool_description = tool_description.unwrap_or_default().to_owned();
        let schema = FunctionSchema::<TArgs>::from_type(
            resolved_tool_name.clone(),
            Some(resolved_tool_description.clone()),
            true,
        )
        .map_err(|error| AgentsError::message(error.message))?;

        let schema_info = StructuredInputSchemaInfo {
            summary: None,
            json_schema: options
                .include_input_schema
                .then(|| schema.params_json_schema.clone()),
        };

        let agent = self.clone();
        let enabled = options.enabled;
        let is_enabled = options.is_enabled.clone();
        let include_input_schema = options.include_input_schema;
        let needs_approval = options.needs_approval;
        let run_config = options.run_config.clone();
        let max_turns = options.max_turns;
        let previous_response_id = options.previous_response_id.clone();
        let conversation_id = options.conversation_id.clone();
        let session = options.session.clone();
        let failure_error_function = options.failure_error_function.clone();
        let on_stream = options.on_stream.clone();
        let custom_output_extractor = options.custom_output_extractor.clone();
        let input_builder = options.input_builder.clone();
        let should_capture_tool_input = include_input_schema
            || input_builder.is_some()
            || TypeId::of::<TArgs>() != TypeId::of::<AgentAsToolInput>();

        let executor = Arc::new(move |tool_context: ToolContext, raw_args: Value| {
            let agent = agent.clone();
            let run_config = run_config.clone();
            let previous_response_id = previous_response_id.clone();
            let conversation_id = conversation_id.clone();
            let session = session.clone();
            let failure_error_function = failure_error_function.clone();
            let on_stream = on_stream.clone();
            let custom_output_extractor = custom_output_extractor.clone();
            let input_builder = input_builder.clone();
            let schema_info = schema_info.clone();
            async move {
                let args_json =
                    serde_json::to_string(&raw_args).unwrap_or_else(|_| "{}".to_owned());
                let parsed: TArgs = serde_json::from_value(raw_args.clone()).map_err(|error| {
                    AgentsError::message(format!("Invalid JSON input for tool: {error}"))
                })?;
                let params_value = serde_json::to_value(parsed)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
                let resolved_input = if let Some(builder) = input_builder {
                    builder(params_value.clone(), Some(schema_info.clone())).await?
                } else {
                    resolve_agent_tool_input(
                        &params_value,
                        if include_input_schema {
                            Some(&schema_info)
                        } else {
                            None
                        },
                    )
                };

                let mut nested_run_config = run_config
                    .clone()
                    .or_else(|| tool_context.run_config.clone())
                    .unwrap_or_default();
                if let Some(max_turns) = max_turns {
                    nested_run_config.max_turns = max_turns;
                }
                if previous_response_id.is_some() {
                    nested_run_config.previous_response_id = previous_response_id.clone();
                }
                if conversation_id.is_some() {
                    nested_run_config.conversation_id = conversation_id.clone();
                }
                let resolved_max_turns = nested_run_config.max_turns;
                let mut nested_context = tool_context.run_context.clone();
                nested_context.approvals.clear();
                if should_capture_tool_input {
                    nested_context.tool_input = Some(params_value.clone());
                } else {
                    nested_context.tool_input = None;
                }
                let state_scope = nested_context.agent_tool_state_scope.clone();

                let nested_input = match resolved_input {
                    ResolvedToolInput::Text(text) => vec![crate::items::InputItem::from(text)],
                    ResolvedToolInput::Items(items) => items,
                };
                let runner = get_default_agent_runner().with_config(nested_run_config);

                let tool_invocation = AgentToolInvocation {
                    tool_name: tool_context.tool_name.clone(),
                    tool_call_id: Some(tool_context.tool_call_id.clone()),
                    tool_arguments: Some(args_json),
                    qualified_name: Some(tool_context.qualified_tool_name()),
                    output: None,
                    agent_name: Some(agent.name.clone()),
                };

                if on_stream.is_some() {
                    let mut streamed = if let Some(session) = &session {
                        runner
                            .run_items_streamed_with_session_and_context(
                                &agent,
                                nested_input.clone(),
                                session.clone(),
                                nested_context.clone(),
                            )
                            .await
                    } else {
                        runner
                            .run_items_streamed_with_context(
                                &agent,
                                nested_input.clone(),
                                nested_context,
                            )
                            .await
                    };

                    match streamed.as_mut() {
                        Ok(streamed) => {
                            streamed.current_turn = 0;
                            streamed.max_turns = resolved_max_turns;
                            streamed.agent_tool_invocation = Some(tool_invocation.clone());
                            if let Some(handler) = &on_stream {
                                let mut stream = Box::pin(streamed.stream_events());
                                while let Some(event) = stream.next().await {
                                    handler(AgentToolStreamEvent {
                                        event,
                                        agent: agent.clone(),
                                        tool_call: tool_context.tool_call.clone(),
                                    })
                                    .await?;
                                }
                            }
                            let completed = streamed.wait_for_completion().await;
                            if let Ok(result) = &completed {
                                record_agent_tool_run_result(
                                    tool_context.tool_call_id.clone(),
                                    result.clone(),
                                    state_scope.clone(),
                                );
                            }
                            if let Err(error) = completed {
                                if let Some(formatter) = &failure_error_function {
                                    if let Some(message) = formatter(error.to_string()).await {
                                        return Ok(ToolOutput::from(message));
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(formatter) = &failure_error_function {
                                if let Some(message) = formatter(error.to_string()).await {
                                    return Ok(ToolOutput::from(message));
                                }
                            }
                        }
                    }

                    let streamed = streamed?;
                    let extracted = if let Some(extractor) = &custom_output_extractor {
                        extractor(AgentToolRunResult::Streaming(streamed.clone())).await?
                    } else {
                        default_agent_tool_output_text(&streamed.wait_for_completion().await?)
                    };
                    return Ok(ToolOutput::from(extracted));
                }

                match runner
                    .run_items_with_context(&agent, nested_input, nested_context)
                    .await
                {
                    Ok(mut result) => {
                        record_agent_tool_run_result(
                            tool_context.tool_call_id.clone(),
                            result.clone(),
                            state_scope,
                        );
                        result.agent_tool_invocation = Some(tool_invocation);
                        let extracted = if let Some(extractor) = &custom_output_extractor {
                            extractor(AgentToolRunResult::Run(result.clone())).await?
                        } else {
                            default_agent_tool_output_text(&result)
                        };
                        Ok(ToolOutput::from(extracted))
                    }
                    Err(error) => {
                        if let Some(formatter) = &failure_error_function {
                            if let Some(message) = formatter(error.to_string()).await {
                                Ok(ToolOutput::from(message))
                            } else {
                                Err(error)
                            }
                        } else {
                            Err(error)
                        }
                    }
                }
            }
            .boxed()
        });

        let mut definition =
            crate::tool::ToolDefinition::new(resolved_tool_name, resolved_tool_description)
                .with_input_json_schema(schema.params_json_schema.clone());
        definition.strict_json_schema = schema.strict_json_schema;

        let mut tool = FunctionTool::new(definition, executor).with_needs_approval(needs_approval);
        tool.enabled = enabled;
        if let Some(is_enabled) = is_enabled {
            tool = tool.with_is_enabled(is_enabled);
        }
        Ok(tool)
    }
}

/// Builder for [`Agent`].
#[derive(Clone, Debug)]
pub struct AgentBuilder {
    agent: Agent,
}

impl AgentBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            agent: Agent {
                name: name.into(),
                ..Agent::default()
            },
        }
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.agent.instructions = Some(instructions.into());
        self
    }

    pub fn handoff_description(mut self, handoff_description: impl Into<String>) -> Self {
        self.agent.handoff_description = Some(handoff_description.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.agent.model = Some(model.into());
        self
    }

    pub fn model_settings(mut self, model_settings: ModelSettings) -> Self {
        self.agent.model_settings = Some(model_settings);
        self
    }

    pub fn output_schema(mut self, output_schema: OutputSchemaDefinition) -> Self {
        self.agent.output_schema = Some(output_schema);
        self
    }

    pub fn tool(mut self, tool: StaticTool) -> Self {
        self.agent.tools.push(tool);
        self
    }

    pub fn function_tool(mut self, tool: FunctionTool) -> Self {
        self.agent.tools.push(StaticTool {
            definition: tool.definition.clone(),
        });
        self.agent.function_tools.push(tool);
        self
    }

    pub fn handoff(mut self, handoff: Handoff) -> Self {
        self.agent.handoffs.push(handoff);
        self
    }

    pub fn mcp_server(mut self, server: Arc<dyn MCPServer>) -> Self {
        self.agent.mcp_servers.push(server);
        self
    }

    pub fn hooks(mut self, hooks: SharedAgentHooks) -> Self {
        self.agent.hooks = Some(hooks);
        self
    }

    pub fn handoff_to_agent(mut self, agent: Agent) -> Self {
        self.agent.handoffs.push(Handoff::to_agent(agent));
        self
    }

    pub fn tool_use_behavior(mut self, tool_use_behavior: ToolUseBehavior) -> Self {
        self.agent.tool_use_behavior = tool_use_behavior;
        self
    }

    pub fn input_guardrail(mut self, guardrail: InputGuardrail) -> Self {
        self.agent.input_guardrails.push(guardrail);
        self
    }

    pub fn output_guardrail(mut self, guardrail: OutputGuardrail) -> Self {
        self.agent.output_guardrails.push(guardrail);
        self
    }

    pub fn build(self) -> Agent {
        self.agent
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::agent_tool_state::{
        drop_agent_tool_run_result, peek_agent_tool_run_result, set_agent_tool_state_scope,
    };
    use crate::run_context::{ApprovalRecord, RunContext};
    use crate::tool::Tool;
    use crate::tool::function_tool;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct SearchArgs {
        query: String,
    }

    #[test]
    fn stores_runtime_function_tools_and_serialized_definitions() {
        let tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, crate::errors::AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");

        let agent = Agent::builder("assistant").function_tool(tool).build();

        assert_eq!(agent.tools.len(), 1);
        assert_eq!(agent.function_tools.len(), 1);
        assert!(agent.find_function_tool("search", None).is_some());
    }

    #[tokio::test]
    async fn stop_at_tools_matches_public_and_qualified_names() {
        let behavior = ToolUseBehavior::StopAtTools(StopAtTools {
            stop_at_tool_names: vec![
                "lookup_account".to_owned(),
                "billing.lookup_account".to_owned(),
            ],
        });
        let context = RunContextWrapper::new(RunContext::default());

        let result = behavior
            .evaluate(
                &context,
                &[crate::tool::FunctionToolResult {
                    tool_name: "lookup_account".to_owned(),
                    call_id: Some("call_123".to_owned()),
                    tool_arguments: Some("{\"account_id\":\"123\"}".to_owned()),
                    qualified_name: Some("billing.lookup_account".to_owned()),
                    output: crate::tool::ToolOutput::from("ok"),
                    run_item: None,
                    interruptions: Vec::new(),
                    agent_run_result: None,
                }],
            )
            .await
            .expect("tool behavior should evaluate");

        assert!(result.is_final_output);
        assert_eq!(result.final_output, Some(Value::String("ok".to_owned())));
    }

    #[tokio::test]
    async fn agent_as_tool_records_nested_run_result_with_scope() {
        let agent = Agent::builder("assistant").build();
        let tool = agent
            .as_tool::<AgentAsToolInput>(
                Some("assistant_tool"),
                Some("Runs the assistant"),
                AgentAsToolOptions::default(),
            )
            .expect("agent tool should build");
        let mut run_context = RunContextWrapper::new(RunContext::default());
        set_agent_tool_state_scope(&mut run_context, Some("scope-a".to_owned()));
        run_context.approvals.insert(
            "call-123".to_owned(),
            ApprovalRecord {
                approved: true,
                reason: Some("approved in parent".to_owned()),
            },
        );
        run_context.tool_input = Some(json!({"stale": true}));

        let output = tool
            .invoke(
                ToolContext::new(
                    run_context,
                    "assistant_tool",
                    "call-123",
                    "{\"input\":\"hello\"}",
                ),
                json!({"input":"hello"}),
            )
            .await
            .expect("agent tool should execute");

        assert_eq!(output, ToolOutput::from("hello"));

        let stored = peek_agent_tool_run_result("call-123", Some("scope-a".to_owned()))
            .expect("nested run result should be recorded");
        assert_eq!(stored.final_output.as_deref(), Some("hello"));
        assert_eq!(
            stored.context_snapshot.agent_tool_state_scope.as_deref(),
            Some("scope-a")
        );
        assert!(stored.context_snapshot.approvals.is_empty());
        assert!(stored.context_snapshot.tool_input.is_none());

        drop_agent_tool_run_result("call-123", Some("scope-a".to_owned()));
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct TranslateArgs {
        text: String,
        source: String,
        target: String,
    }

    #[tokio::test]
    async fn agent_as_tool_captures_structured_tool_input_in_nested_context() {
        let agent = Agent::builder("translator").build();
        let tool = agent
            .as_tool::<TranslateArgs>(
                Some("translate"),
                Some("Translate text"),
                AgentAsToolOptions::default(),
            )
            .expect("agent tool should build");
        let mut run_context = RunContextWrapper::new(RunContext::default());
        set_agent_tool_state_scope(&mut run_context, Some("scope-structured".to_owned()));

        tool.invoke(
            ToolContext::new(
                run_context,
                "translate",
                "call-translate",
                "{\"text\":\"hola\",\"source\":\"es\",\"target\":\"en\"}",
            ),
            json!({"text":"hola","source":"es","target":"en"}),
        )
        .await
        .expect("agent tool should execute");

        let stored =
            peek_agent_tool_run_result("call-translate", Some("scope-structured".to_owned()))
                .expect("structured nested run result should be recorded");
        assert_eq!(
            stored.context_snapshot.tool_input,
            Some(json!({"text":"hola","source":"es","target":"en"}))
        );

        drop_agent_tool_run_result("call-translate", Some("scope-structured".to_owned()));
    }
}
