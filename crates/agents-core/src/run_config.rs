use std::sync::Arc;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::guardrail::{InputGuardrail, OutputGuardrail};
use crate::handoff::{HandoffHistoryMapper, HandoffInputFilter};
use crate::items::InputItem;
use crate::lifecycle::SharedRunHooks;
use crate::memory::SessionSettings;
use crate::model::ModelProvider;
use crate::model_settings::ModelSettings;
use crate::run_context::{RunContext, RunContextWrapper};
use crate::run_error_handlers::RunErrorHandlers;
use crate::sandbox::SandboxRunConfig;
use crate::session::Session;
use crate::tracing::TracingConfig;

pub const DEFAULT_MAX_TURNS: usize = 10;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelInputData {
    pub input: Vec<InputItem>,
    pub instructions: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CallModelData<TContext = RunContext> {
    pub model_data: ModelInputData,
    pub agent: Agent,
    pub context: Option<TContext>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningItemIdPolicy {
    #[default]
    Preserve,
    Omit,
}

#[derive(Clone, Debug)]
pub struct ToolErrorFormatterArgs<TContext = RunContext> {
    pub kind: &'static str,
    pub tool_type: &'static str,
    pub tool_name: String,
    pub call_id: String,
    pub default_message: String,
    pub run_context: RunContextWrapper<TContext>,
}

pub type ToolErrorFormatter = Arc<
    dyn Fn(
            ToolErrorFormatterArgs<RunContext>,
        ) -> BoxFuture<'static, crate::errors::Result<Option<String>>>
        + Send
        + Sync,
>;

pub type SessionInputCallback = Arc<
    dyn Fn(
            Vec<InputItem>,
            Vec<InputItem>,
        ) -> BoxFuture<'static, crate::errors::Result<Vec<InputItem>>>
        + Send
        + Sync,
>;

#[derive(Clone, Serialize, Deserialize)]
pub struct RunConfig {
    pub model: Option<String>,
    pub max_turns: usize,
    pub nest_handoff_history: bool,
    pub tracing_disabled: bool,
    pub trace_include_sensitive_data: bool,
    pub workflow_name: String,
    pub trace_id: Option<String>,
    pub group_id: Option<String>,
    pub trace_metadata: Option<std::collections::BTreeMap<String, Value>>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: bool,
    pub conversation_id: Option<String>,
    pub reasoning_item_id_policy: ReasoningItemIdPolicy,
    pub tracing: Option<TracingConfig>,
    pub model_settings: Option<ModelSettings>,
    pub sandbox: Option<SandboxRunConfig>,
    pub session_settings: Option<SessionSettings>,
    #[serde(skip, default)]
    pub model_provider: Option<Arc<dyn ModelProvider>>,
    #[serde(skip, default)]
    pub handoff_input_filter: Option<HandoffInputFilter>,
    #[serde(skip, default)]
    pub handoff_history_mapper: Option<HandoffHistoryMapper>,
    #[serde(skip, default)]
    pub input_guardrails: Option<Vec<InputGuardrail>>,
    #[serde(skip, default)]
    pub output_guardrails: Option<Vec<OutputGuardrail>>,
    #[serde(skip, default)]
    pub session_input_callback: Option<SessionInputCallback>,
    #[serde(skip, default)]
    pub call_model_input_filter: Option<CallModelInputFilter>,
    #[serde(skip, default)]
    pub tool_error_formatter: Option<ToolErrorFormatter>,
    #[serde(skip, default)]
    pub run_hooks: Option<SharedRunHooks>,
    #[serde(skip, default)]
    pub run_error_handlers: RunErrorHandlers,
}

impl std::fmt::Debug for RunConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunConfig")
            .field("model", &self.model)
            .field("max_turns", &self.max_turns)
            .field("nest_handoff_history", &self.nest_handoff_history)
            .field("tracing_disabled", &self.tracing_disabled)
            .field(
                "trace_include_sensitive_data",
                &self.trace_include_sensitive_data,
            )
            .field("workflow_name", &self.workflow_name)
            .field("trace_id", &self.trace_id)
            .field("group_id", &self.group_id)
            .field("trace_metadata", &self.trace_metadata)
            .field("previous_response_id", &self.previous_response_id)
            .field("auto_previous_response_id", &self.auto_previous_response_id)
            .field("conversation_id", &self.conversation_id)
            .field("reasoning_item_id_policy", &self.reasoning_item_id_policy)
            .field("tracing", &self.tracing)
            .field("model_settings", &self.model_settings)
            .field("sandbox", &self.sandbox)
            .field("session_settings", &self.session_settings)
            .field(
                "model_provider",
                &self.model_provider.as_ref().map(|_| "<provider>"),
            )
            .field(
                "handoff_input_filter",
                &self.handoff_input_filter.as_ref().map(|_| "<filter>"),
            )
            .field(
                "handoff_history_mapper",
                &self.handoff_history_mapper.as_ref().map(|_| "<mapper>"),
            )
            .field(
                "input_guardrails",
                &self.input_guardrails.as_ref().map(|value| value.len()),
            )
            .field(
                "output_guardrails",
                &self.output_guardrails.as_ref().map(|value| value.len()),
            )
            .field(
                "session_input_callback",
                &self.session_input_callback.as_ref().map(|_| "<callback>"),
            )
            .field(
                "call_model_input_filter",
                &self.call_model_input_filter.as_ref().map(|_| "<filter>"),
            )
            .field(
                "tool_error_formatter",
                &self.tool_error_formatter.as_ref().map(|_| "<formatter>"),
            )
            .field("run_hooks", &self.run_hooks.as_ref().map(|_| "<hooks>"))
            .field("run_error_handlers", &self.run_error_handlers)
            .finish()
    }
}

pub type CallModelInputFilter = Arc<
    dyn Fn(CallModelData<RunContext>) -> BoxFuture<'static, crate::errors::Result<ModelInputData>>
        + Send
        + Sync,
>;

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            model: None,
            max_turns: DEFAULT_MAX_TURNS,
            nest_handoff_history: false,
            tracing_disabled: false,
            trace_include_sensitive_data: false,
            workflow_name: "Agent workflow".to_owned(),
            trace_id: None,
            group_id: None,
            trace_metadata: None,
            previous_response_id: None,
            auto_previous_response_id: false,
            conversation_id: None,
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
            tracing: None,
            model_settings: None,
            sandbox: None,
            session_settings: None,
            model_provider: None,
            handoff_input_filter: None,
            handoff_history_mapper: None,
            input_guardrails: None,
            output_guardrails: None,
            session_input_callback: None,
            call_model_input_filter: None,
            tool_error_formatter: None,
            run_hooks: None,
            run_error_handlers: RunErrorHandlers::default(),
        }
    }
}

#[derive(Clone, Default)]
pub struct RunOptions<TContext = RunContext> {
    pub context: Option<TContext>,
    pub max_turns: Option<usize>,
    pub hooks: Option<SharedRunHooks>,
    pub error_handlers: Option<RunErrorHandlers>,
    pub run_config: Option<RunConfig>,
    pub session: Option<Arc<dyn Session + Sync>>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: Option<bool>,
    pub conversation_id: Option<String>,
    pub model_provider: Option<Arc<dyn ModelProvider>>,
}

impl<TContext> std::fmt::Debug for RunOptions<TContext>
where
    TContext: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunOptions")
            .field("context", &self.context)
            .field("max_turns", &self.max_turns)
            .field("hooks", &self.hooks.as_ref().map(|_| "<hooks>"))
            .field("error_handlers", &self.error_handlers)
            .field("run_config", &self.run_config)
            .field("session", &self.session.as_ref().map(|_| "<session>"))
            .field("previous_response_id", &self.previous_response_id)
            .field("auto_previous_response_id", &self.auto_previous_response_id)
            .field("conversation_id", &self.conversation_id)
            .field(
                "model_provider",
                &self.model_provider.as_ref().map(|_| "<provider>"),
            )
            .finish()
    }
}
