use serde::{Deserialize, Serialize};

use crate::_tool_identity::tool_trace_name;
use crate::agent::Agent;
use crate::agent_tool_state::{get_agent_tool_state_scope, set_agent_tool_state_scope};
use crate::run_config::RunConfig;
use crate::run_context::{ApprovalRecord, RunContext, RunContextWrapper};

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

    pub fn trace_name(&self) -> String {
        tool_trace_name(&self.tool_name, self.tool_namespace.as_deref())
            .unwrap_or_else(|| self.tool_name.clone())
    }

    pub fn with_tool_call(mut self, tool_call: ToolCall) -> Self {
        self.tool_namespace = tool_call.namespace.clone();
        self.tool_call = Some(tool_call);
        self
    }

    pub fn with_namespace(mut self, tool_namespace: impl Into<String>) -> Self {
        self.tool_namespace = Some(tool_namespace.into());
        self
    }

    pub fn with_agent(mut self, agent: Agent) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn with_run_config(mut self, run_config: RunConfig) -> Self {
        self.run_config = Some(run_config);
        self
    }

    pub fn approval(&self, approval_id: &str) -> Option<&ApprovalRecord> {
        self.run_context.approvals.get(approval_id)
    }

    pub fn agent_tool_state_scope(&self) -> Option<String> {
        get_agent_tool_state_scope(&self.run_context)
    }

    pub fn set_agent_tool_state_scope(&mut self, scope_id: Option<String>) {
        set_agent_tool_state_scope(&mut self.run_context, scope_id);
    }
}

impl<TContext> ToolContext<TContext>
where
    TContext: Clone,
{
    pub fn from_run_context(
        run_context: &RunContextWrapper<TContext>,
        tool_name: impl Into<String>,
        tool_call_id: impl Into<String>,
        tool_arguments: impl Into<String>,
    ) -> Self {
        Self::new(run_context.clone(), tool_name, tool_call_id, tool_arguments)
    }

    pub fn from_tool_call(run_context: &RunContextWrapper<TContext>, tool_call: ToolCall) -> Self {
        Self {
            run_context: run_context.clone(),
            tool_name: tool_call.name.clone(),
            tool_call_id: tool_call.id.clone(),
            tool_arguments: tool_call.arguments.clone(),
            tool_namespace: tool_call.namespace.clone(),
            tool_call: Some(tool_call),
            agent: None,
            run_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_context_from_tool_call_and_copies_scope() {
        let mut run_context = RunContextWrapper::new(RunContext::default());
        set_agent_tool_state_scope(&mut run_context, Some("scope-1".to_owned()));

        let context = ToolContext::from_tool_call(
            &run_context,
            ToolCall {
                id: "call-1".to_owned(),
                name: "search".to_owned(),
                arguments: "{}".to_owned(),
                namespace: Some("knowledge".to_owned()),
            },
        );

        assert_eq!(context.qualified_tool_name(), "knowledge.search");
        assert_eq!(context.trace_name(), "knowledge.search");
        assert_eq!(context.agent_tool_state_scope().as_deref(), Some("scope-1"));
    }
}
