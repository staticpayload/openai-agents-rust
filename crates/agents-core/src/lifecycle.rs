use async_trait::async_trait;

use crate::agent::Agent;
use crate::items::InputItem;
use crate::model::ModelResponse;
use crate::run_context::{AgentHookContext, RunContext, RunContextWrapper};
use crate::tool::ToolDefinition;

#[async_trait]
pub trait RunHooks<TContext = RunContext>: Send + Sync {
    async fn on_llm_start(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _system_prompt: Option<&str>,
        _input_items: &[InputItem],
    ) {
    }

    async fn on_llm_end(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _response: &ModelResponse,
    ) {
    }

    async fn on_agent_start(&self, _context: &AgentHookContext<TContext>, _agent: &Agent) {}

    async fn on_agent_end(
        &self,
        _context: &AgentHookContext<TContext>,
        _agent: &Agent,
        _output: Option<&str>,
    ) {
    }

    async fn on_handoff(
        &self,
        _context: &RunContextWrapper<TContext>,
        _from_agent: &Agent,
        _to_agent: &Agent,
    ) {
    }

    async fn on_tool_start(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _tool: &ToolDefinition,
    ) {
    }

    async fn on_tool_end(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _tool: &ToolDefinition,
        _result: &str,
    ) {
    }
}

#[async_trait]
pub trait AgentHooks<TContext = RunContext>: Send + Sync {
    async fn on_start(&self, _context: &AgentHookContext<TContext>, _agent: &Agent) {}

    async fn on_end(
        &self,
        _context: &AgentHookContext<TContext>,
        _agent: &Agent,
        _output: Option<&str>,
    ) {
    }

    async fn on_handoff(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _source: &Agent,
    ) {
    }

    async fn on_tool_start(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _tool: &ToolDefinition,
    ) {
    }

    async fn on_tool_end(
        &self,
        _context: &RunContextWrapper<TContext>,
        _agent: &Agent,
        _tool: &ToolDefinition,
        _result: &str,
    ) {
    }
}
