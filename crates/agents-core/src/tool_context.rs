use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::run_config::RunConfig;
use crate::run_context::{RunContext, RunContextWrapper};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub namespace: Option<String>,
}

/// Context passed to local tool handlers.
#[derive(Clone, Debug)]
pub struct ToolContext<TContext = RunContext> {
    pub run_context: RunContextWrapper<TContext>,
    pub tool_name: String,
    pub tool_call_id: String,
    pub tool_arguments: String,
    pub tool_call: Option<ToolCall>,
    pub tool_namespace: Option<String>,
    pub agent: Option<Agent>,
    pub run_config: Option<RunConfig>,
}

impl<TContext> ToolContext<TContext> {
    pub fn new(
        run_context: RunContextWrapper<TContext>,
        tool_name: impl Into<String>,
        tool_call_id: impl Into<String>,
        tool_arguments: impl Into<String>,
    ) -> Self {
        Self {
            run_context,
            tool_name: tool_name.into(),
            tool_call_id: tool_call_id.into(),
            tool_arguments: tool_arguments.into(),
            tool_call: None,
            tool_namespace: None,
            agent: None,
            run_config: None,
        }
    }

    pub fn qualified_tool_name(&self) -> String {
        match &self.tool_namespace {
            Some(namespace) => format!("{namespace}.{}", self.tool_name),
            None => self.tool_name.clone(),
        }
    }
}
