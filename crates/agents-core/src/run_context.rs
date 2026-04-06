use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::items::InputItem;
use crate::usage::Usage;

/// Per-run metadata passed through agent execution.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunContext {
    pub conversation_id: Option<String>,
    pub workflow_name: Option<String>,
}

/// Approval state recorded during a run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRecord {
    pub approved: bool,
    pub reason: Option<String>,
    pub tool_name: Option<String>,
}

/// Runtime context wrapper shared across callbacks.
#[derive(Clone, Debug)]
pub struct RunContextWrapper<TContext = RunContext> {
    pub context: TContext,
    pub usage: Usage,
    pub turn_input: Vec<InputItem>,
    pub approvals: HashMap<String, ApprovalRecord>,
    pub tool_input: Option<Value>,
    pub agent_tool_state_scope: Option<String>,
}

impl<TContext> RunContextWrapper<TContext> {
    pub fn new(context: TContext) -> Self {
        Self {
            context,
            usage: Usage::default(),
            turn_input: Vec::new(),
            approvals: HashMap::new(),
            tool_input: None,
            agent_tool_state_scope: None,
        }
    }
}

/// Agent-specific lifecycle context.
#[derive(Clone, Debug)]
pub struct AgentHookContext<TContext = RunContext> {
    pub context: TContext,
    pub turn: usize,
}

impl<TContext> AgentHookContext<TContext> {
    pub fn new(context: TContext, turn: usize) -> Self {
        Self { context, turn }
    }
}
