use std::collections::HashMap;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::errors::{AgentsError, Result};
use crate::guardrail::{InputGuardrailResult, OutputGuardrailResult};
use crate::items::{InputItem, RunItem};
use crate::model::ModelResponse;
use crate::run_config::ReasoningItemIdPolicy;
use crate::run_context::{ApprovalRecord, RunContextWrapper};
use crate::tool_guardrails::{ToolInputGuardrailResult, ToolOutputGuardrailResult};
use crate::tracing::Trace;
use crate::usage::Usage;

pub const CURRENT_RUN_STATE_SCHEMA_VERSION: &str = "1.6";

/// Serializable snapshot of the runtime context carried across a run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunStateContextSnapshot {
    pub context: Value,
    pub usage: Usage,
    pub turn_input: Vec<InputItem>,
    pub approvals: HashMap<String, ApprovalRecord>,
    pub tool_input: Option<Value>,
    pub agent_tool_state_scope: Option<String>,
}

/// Current interruption state when a run pauses mid-flight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunInterruptionKind {
    ToolApproval,
    Handoff,
    InputGuardrail,
    OutputGuardrail,
    ToolInputGuardrail,
    ToolOutputGuardrail,
    External,
}

/// Durable interruption metadata stored inside a run snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunInterruption {
    pub kind: Option<RunInterruptionKind>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
}

/// Serializable pause/resume boundary for an agent run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunState {
    pub schema_version: String,
    pub current_turn: usize,
    pub current_agent: Option<Agent>,
    pub original_input: Vec<InputItem>,
    pub normalized_input: Option<Vec<InputItem>>,
    pub model_responses: Vec<ModelResponse>,
    pub generated_items: Vec<RunItem>,
    pub session_items: Vec<RunItem>,
    pub max_turns: usize,
    pub conversation_id: Option<String>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: bool,
    pub reasoning_item_id_policy: ReasoningItemIdPolicy,
    pub input_guardrail_results: Vec<InputGuardrailResult>,
    pub output_guardrail_results: Vec<OutputGuardrailResult>,
    pub tool_input_guardrail_results: Vec<ToolInputGuardrailResult>,
    pub tool_output_guardrail_results: Vec<ToolOutputGuardrailResult>,
    pub current_step: Option<RunInterruption>,
    pub persisted_item_count: usize,
    pub trace: Option<Trace>,
    pub context_snapshot: RunStateContextSnapshot,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_RUN_STATE_SCHEMA_VERSION.to_owned(),
            current_turn: 0,
            current_agent: None,
            original_input: Vec::new(),
            normalized_input: None,
            model_responses: Vec::new(),
            generated_items: Vec::new(),
            session_items: Vec::new(),
            max_turns: 10,
            conversation_id: None,
            previous_response_id: None,
            auto_previous_response_id: false,
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
            input_guardrail_results: Vec::new(),
            output_guardrail_results: Vec::new(),
            tool_input_guardrail_results: Vec::new(),
            tool_output_guardrail_results: Vec::new(),
            current_step: None,
            persisted_item_count: 0,
            trace: None,
            context_snapshot: RunStateContextSnapshot::default(),
        }
    }
}

impl RunState {
    pub fn new<TContext>(
        context: &RunContextWrapper<TContext>,
        original_input: Vec<InputItem>,
        starting_agent: Agent,
        max_turns: usize,
    ) -> Result<Self>
    where
        TContext: Clone + Serialize,
    {
        let context_value = serde_json::to_value(&context.context)
            .map_err(|error| AgentsError::message(error.to_string()))?;

        Ok(Self {
            current_agent: Some(starting_agent),
            original_input,
            max_turns,
            context_snapshot: RunStateContextSnapshot {
                context: context_value,
                usage: context.usage,
                turn_input: context.turn_input.clone(),
                approvals: context.approvals.clone(),
                tool_input: context.tool_input.clone(),
                agent_tool_state_scope: context.agent_tool_state_scope.clone(),
            },
            ..Self::default()
        })
    }

    pub fn restore_context<TContext>(&self) -> Result<RunContextWrapper<TContext>>
    where
        TContext: DeserializeOwned,
    {
        let context = serde_json::from_value::<TContext>(self.context_snapshot.context.clone())
            .map_err(|error| AgentsError::message(error.to_string()))?;

        Ok(RunContextWrapper {
            context,
            usage: self.context_snapshot.usage,
            turn_input: self.context_snapshot.turn_input.clone(),
            approvals: self.context_snapshot.approvals.clone(),
            tool_input: self.context_snapshot.tool_input.clone(),
            agent_tool_state_scope: self.context_snapshot.agent_tool_state_scope.clone(),
        })
    }

    pub fn current_agent_name(&self) -> Option<&str> {
        self.current_agent.as_ref().map(|agent| agent.name.as_str())
    }

    pub fn remaining_turns(&self) -> usize {
        self.max_turns.saturating_sub(self.current_turn)
    }

    pub fn can_continue(&self) -> bool {
        self.remaining_turns() > 0 && self.current_step.is_none()
    }

    pub fn is_interrupted(&self) -> bool {
        self.current_step.is_some()
    }

    pub fn mark_turn_started(&mut self) {
        self.current_turn += 1;
    }

    pub fn set_current_agent(&mut self, agent: Agent) {
        self.current_agent = Some(agent);
    }

    pub fn set_trace(&mut self, trace: Trace) {
        self.trace = Some(trace);
    }

    pub fn push_model_response(&mut self, response: ModelResponse) {
        self.model_responses.push(response);
    }

    pub fn push_generated_item(&mut self, item: RunItem) {
        self.generated_items.push(item);
    }

    pub fn extend_generated_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = RunItem>,
    {
        self.generated_items.extend(items);
    }

    pub fn push_session_item(&mut self, item: RunItem) {
        self.session_items.push(item);
    }

    pub fn extend_session_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = RunItem>,
    {
        self.session_items.extend(items);
    }

    pub fn approval(&self, id: &str) -> Option<&ApprovalRecord> {
        self.context_snapshot.approvals.get(id)
    }

    pub fn approve(&mut self, id: impl Into<String>, reason: Option<String>) {
        self.approve_for_tool(id, None, reason);
    }

    pub fn approve_for_tool(
        &mut self,
        id: impl Into<String>,
        tool_name: Option<String>,
        reason: Option<String>,
    ) {
        self.context_snapshot.approvals.insert(
            id.into(),
            ApprovalRecord {
                approved: true,
                reason,
                tool_name,
            },
        );
    }

    pub fn reject(&mut self, id: impl Into<String>, reason: Option<String>) {
        self.reject_for_tool(id, None, reason);
    }

    pub fn reject_for_tool(
        &mut self,
        id: impl Into<String>,
        tool_name: Option<String>,
        reason: Option<String>,
    ) {
        self.context_snapshot.approvals.insert(
            id.into(),
            ApprovalRecord {
                approved: false,
                reason,
                tool_name,
            },
        );
    }

    pub fn interrupt(
        &mut self,
        kind: RunInterruptionKind,
        call_id: Option<String>,
        tool_name: Option<String>,
        reason: Option<String>,
    ) {
        self.current_step = Some(RunInterruption {
            kind: Some(kind),
            call_id,
            tool_name,
            reason,
        });
    }

    pub fn clear_interruption(&mut self) {
        self.current_step = None;
    }

    pub fn record_input_guardrail_result(&mut self, result: InputGuardrailResult) {
        self.input_guardrail_results.push(result);
    }

    pub fn record_output_guardrail_result(&mut self, result: OutputGuardrailResult) {
        self.output_guardrail_results.push(result);
    }

    pub fn record_tool_input_guardrail_result(&mut self, result: ToolInputGuardrailResult) {
        self.tool_input_guardrail_results.push(result);
    }

    pub fn record_tool_output_guardrail_result(&mut self, result: ToolOutputGuardrailResult) {
        self.tool_output_guardrail_results.push(result);
    }

    pub fn resume_input(&self) -> Vec<InputItem> {
        crate::internal::items::compose_replay_input_items(
            self.normalized_input
                .as_deref()
                .unwrap_or(&self.original_input),
            &self.generated_items,
            self.reasoning_item_id_policy,
        )
    }

    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| AgentsError::message(error.to_string()))
    }

    pub fn from_json_str(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(|error| AgentsError::message(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::guardrail::{GuardrailFunctionOutput, InputGuardrailResult};
    use crate::run_context::RunContext;
    use crate::tool_guardrails::{ToolGuardrailFunctionOutput, ToolInputGuardrailResult};

    use super::*;

    #[test]
    fn snapshots_and_restores_context() {
        let mut context = RunContextWrapper::new(RunContext {
            conversation_id: Some("conv-1".to_owned()),
            workflow_name: Some("Workflow".to_owned()),
        });
        context.turn_input = vec![InputItem::from("hello")];
        context.tool_input = Some(json!({"query":"rust"}));
        context.agent_tool_state_scope = Some("scope-1".to_owned());
        context.usage = Usage {
            input_tokens: 10,
            output_tokens: 4,
        };

        let state = RunState::new(
            &context,
            vec![InputItem::from("start")],
            Agent::builder("router").build(),
            12,
        )
        .expect("run state should build");

        let restored = state
            .restore_context::<RunContext>()
            .expect("context should restore");

        assert_eq!(state.current_agent_name(), Some("router"));
        assert_eq!(state.max_turns, 12);
        assert_eq!(restored.context.conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(restored.tool_input, Some(json!({"query":"rust"})));
        assert_eq!(restored.agent_tool_state_scope.as_deref(), Some("scope-1"));
    }

    #[test]
    fn records_resume_items_and_approvals() {
        let context = RunContextWrapper::new(RunContext::default());
        let mut state = RunState::new(
            &context,
            vec![InputItem::from("start")],
            Agent::builder("router").build(),
            3,
        )
        .expect("run state should build");

        state.mark_turn_started();
        state.normalized_input = Some(vec![InputItem::from("normalized-start")]);
        state.push_generated_item(RunItem::Reasoning {
            text: "thinking".to_owned(),
        });
        state.push_generated_item(RunItem::ToolCallOutput {
            tool_name: "search".to_owned(),
            output: crate::items::OutputItem::Text {
                text: "found".to_owned(),
            },
            call_id: Some("call-1".to_owned()),
            namespace: None,
        });
        state.approve("tool-1", Some("approved".to_owned()));
        state.reject("tool-2", Some("blocked".to_owned()));
        state.interrupt(
            RunInterruptionKind::ToolApproval,
            Some("tool-1".to_owned()),
            Some("search".to_owned()),
            Some("waiting".to_owned()),
        );

        let resume_input = state.resume_input();

        assert_eq!(state.current_turn, 1);
        assert_eq!(resume_input.len(), 3);
        assert_eq!(resume_input[0].as_text(), Some("normalized-start"));
        assert!(state.is_interrupted());
        assert_eq!(
            state.approval("tool-1").map(|record| record.approved),
            Some(true)
        );
        assert_eq!(
            state.approval("tool-2").map(|record| record.approved),
            Some(false)
        );
        assert!(!state.can_continue());
    }

    #[test]
    fn resume_input_preserves_repeated_generated_items() {
        let context = RunContextWrapper::new(RunContext::default());
        let mut state = RunState::new(
            &context,
            vec![InputItem::from("done")],
            Agent::builder("router").build(),
            3,
        )
        .expect("run state should build");

        state.push_generated_item(RunItem::MessageOutput {
            content: crate::items::OutputItem::Text {
                text: "done".to_owned(),
            },
        });

        assert_eq!(
            state.resume_input(),
            vec![InputItem::from("done"), InputItem::from("done")]
        );
    }

    #[test]
    fn records_guardrail_results() {
        let context = RunContextWrapper::new(RunContext::default());
        let mut state = RunState::new(&context, vec![], Agent::builder("router").build(), 3)
            .expect("run state should build");

        state.record_input_guardrail_result(InputGuardrailResult {
            guardrail_name: "input-check".to_owned(),
            output: GuardrailFunctionOutput::tripwire(Some(json!({"blocked":true}))),
        });
        state.record_tool_input_guardrail_result(ToolInputGuardrailResult {
            guardrail_name: "tool-input".to_owned(),
            output: ToolGuardrailFunctionOutput::reject_content(
                "blocked",
                Some(json!({"tool":"search"})),
            ),
        });

        assert_eq!(state.input_guardrail_results.len(), 1);
        assert_eq!(state.tool_input_guardrail_results.len(), 1);
        assert_eq!(
            state.tool_input_guardrail_results[0]
                .output
                .rejection_message(),
            Some("blocked")
        );
    }

    #[test]
    fn round_trips_json() {
        let context = RunContextWrapper::new(RunContext::default());
        let mut state = RunState::new(
            &context,
            vec![InputItem::from("start")],
            Agent::builder("router").build(),
            3,
        )
        .expect("run state should build");
        state.mark_turn_started();
        state.push_generated_item(RunItem::Reasoning {
            text: "thinking".to_owned(),
        });

        let json = state.to_json_string().expect("json should serialize");
        let restored = RunState::from_json_str(&json).expect("json should deserialize");

        assert_eq!(restored.current_turn, 1);
        assert_eq!(restored.current_agent_name(), Some("router"));
        assert_eq!(restored.generated_items.len(), 1);
    }
}
