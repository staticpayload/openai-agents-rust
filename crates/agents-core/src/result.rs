use futures::StreamExt;
use futures::stream::{self, BoxStream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::guardrail::{InputGuardrailResult, OutputGuardrailResult};
use crate::items::{InputItem, OutputItem, RunItem};
use crate::model::ModelResponse;
use crate::run_config::ReasoningItemIdPolicy;
use crate::run_state::{RunInterruption, RunState, RunStateContextSnapshot};
use crate::stream_events::StreamEvent;
use crate::tool_guardrails::{ToolInputGuardrailResult, ToolOutputGuardrailResult};
use crate::tracing::Trace;
use crate::usage::Usage;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentToolInvocation {
    pub tool_name: String,
    pub tool_call_id: Option<String>,
    pub tool_arguments: Option<String>,
    pub qualified_name: Option<String>,
    pub output: Option<Value>,
    pub agent_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToInputListMode {
    #[default]
    PreserveAll,
    Normalized,
}

/// Result of an agent run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunResult {
    pub agent_name: String,
    pub last_agent: Option<Agent>,
    pub input: Vec<InputItem>,
    pub normalized_input: Option<Vec<InputItem>>,
    pub output: Vec<OutputItem>,
    pub new_items: Vec<RunItem>,
    pub raw_responses: Vec<ModelResponse>,
    pub final_output: Option<String>,
    pub input_guardrail_results: Vec<InputGuardrailResult>,
    pub output_guardrail_results: Vec<OutputGuardrailResult>,
    pub tool_input_guardrail_results: Vec<ToolInputGuardrailResult>,
    pub tool_output_guardrail_results: Vec<ToolOutputGuardrailResult>,
    pub context_snapshot: RunStateContextSnapshot,
    pub run_state: Option<RunState>,
    pub interruptions: Vec<RunInterruption>,
    pub usage: Usage,
    pub trace: Option<Trace>,
    pub conversation_id: Option<String>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: bool,
    pub reasoning_item_id_policy: ReasoningItemIdPolicy,
    pub normalized_new_items: Option<Vec<RunItem>>,
    pub agent_tool_invocation: Option<AgentToolInvocation>,
}

impl RunResult {
    pub fn final_output_text(&self) -> Option<&str> {
        self.final_output.as_deref()
    }

    pub fn last_agent(&self) -> Option<&Agent> {
        self.last_agent.as_ref()
    }

    pub fn to_input_list(&self) -> Vec<InputItem> {
        self.to_input_list_mode(ToInputListMode::PreserveAll)
    }

    pub fn to_input_list_mode(&self, mode: ToInputListMode) -> Vec<InputItem> {
        let new_items = match mode {
            ToInputListMode::PreserveAll => &self.new_items,
            ToInputListMode::Normalized => self
                .normalized_new_items
                .as_ref()
                .unwrap_or(&self.new_items),
        };
        let mut items = match mode {
            ToInputListMode::PreserveAll => self.input.clone(),
            ToInputListMode::Normalized => self
                .normalized_input
                .clone()
                .unwrap_or_else(|| self.input.clone()),
        };
        items.extend(run_items_to_input_items(
            new_items,
            self.reasoning_item_id_policy,
        ));
        items
    }

    pub fn durable_state(&self) -> Option<&RunState> {
        self.run_state.as_ref()
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub fn previous_response_id(&self) -> Option<&str> {
        self.previous_response_id.as_deref()
    }

    pub fn last_response(&self) -> Option<&ModelResponse> {
        self.raw_responses.last()
    }

    pub fn agent_tool_invocation(&self) -> Option<&AgentToolInvocation> {
        self.agent_tool_invocation.as_ref()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunResultStreaming {
    pub current_agent: Option<Agent>,
    pub current_turn: usize,
    pub max_turns: usize,
    pub is_complete: bool,
    pub final_output: Option<Value>,
    pub normalized_input: Option<Vec<InputItem>>,
    pub new_items: Vec<RunItem>,
    pub raw_responses: Vec<ModelResponse>,
    pub input_guardrail_results: Vec<InputGuardrailResult>,
    pub output_guardrail_results: Vec<OutputGuardrailResult>,
    pub events: Vec<StreamEvent>,
    pub final_run_result: Option<RunResult>,
    pub reasoning_item_id_policy: ReasoningItemIdPolicy,
    pub normalized_new_items: Option<Vec<RunItem>>,
    pub agent_tool_invocation: Option<AgentToolInvocation>,
}

impl RunResultStreaming {
    pub fn from_run_result(
        result: RunResult,
        current_turn: usize,
        max_turns: usize,
        events: Vec<StreamEvent>,
    ) -> Self {
        let final_output = result
            .final_output
            .as_ref()
            .map(|text| Value::String(text.clone()));
        Self {
            current_agent: result.last_agent.clone(),
            current_turn,
            max_turns,
            is_complete: true,
            final_output,
            normalized_input: result.normalized_input.clone(),
            new_items: result.new_items.clone(),
            raw_responses: result.raw_responses.clone(),
            input_guardrail_results: result.input_guardrail_results.clone(),
            output_guardrail_results: result.output_guardrail_results.clone(),
            events,
            reasoning_item_id_policy: result.reasoning_item_id_policy,
            normalized_new_items: result.normalized_new_items.clone(),
            agent_tool_invocation: result.agent_tool_invocation.clone(),
            final_run_result: Some(result),
        }
    }

    pub fn to_input_list(&self) -> Vec<InputItem> {
        self.to_input_list_mode(ToInputListMode::PreserveAll)
    }

    pub fn to_input_list_mode(&self, mode: ToInputListMode) -> Vec<InputItem> {
        self.final_run_result
            .as_ref()
            .map(|result| result.to_input_list_mode(mode))
            .unwrap_or_else(|| {
                let new_items = match mode {
                    ToInputListMode::PreserveAll => &self.new_items,
                    ToInputListMode::Normalized => self
                        .normalized_new_items
                        .as_ref()
                        .unwrap_or(&self.new_items),
                };
                let mut items = match mode {
                    ToInputListMode::PreserveAll => Vec::new(),
                    ToInputListMode::Normalized => {
                        self.normalized_input.clone().unwrap_or_default()
                    }
                };
                items.extend(run_items_to_input_items(
                    new_items,
                    self.reasoning_item_id_policy,
                ));
                items
            })
    }

    pub fn stream_events(&self) -> BoxStream<'static, StreamEvent> {
        stream::iter(self.events.clone()).boxed()
    }

    pub fn agent_tool_invocation(&self) -> Option<&AgentToolInvocation> {
        self.agent_tool_invocation.as_ref()
    }

    pub fn last_response(&self) -> Option<&ModelResponse> {
        self.final_run_result
            .as_ref()
            .and_then(RunResult::last_response)
            .or_else(|| self.raw_responses.last())
    }
}

fn run_items_to_input_items(
    run_items: &[RunItem],
    reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Vec<InputItem> {
    run_items
        .iter()
        .filter_map(|run_item| match (run_item, reasoning_item_id_policy) {
            (RunItem::Reasoning { .. }, ReasoningItemIdPolicy::Omit) => run_item.to_input_item(),
            _ => run_item.to_input_item(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::items::OutputItem;
    use crate::model::ModelResponse;
    use crate::run_context::RunContext;
    use crate::usage::Usage;

    use super::*;

    #[test]
    fn converts_run_items_back_to_input() {
        let result = RunResult {
            input: vec![InputItem::from("start")],
            new_items: vec![
                RunItem::Reasoning {
                    text: "thinking".to_owned(),
                },
                RunItem::ToolCallOutput {
                    tool_name: "search".to_owned(),
                    output: OutputItem::Text {
                        text: "found".to_owned(),
                    },
                    call_id: Some("call-1".to_owned()),
                    namespace: None,
                },
            ],
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
            ..RunResult::default()
        };

        let replay = result.to_input_list();

        assert_eq!(replay.len(), 3);
        assert_eq!(replay[0].as_text(), Some("start"));
        assert_eq!(
            replay[1],
            InputItem::Json {
                value: json!({
                    "type": "reasoning",
                    "text": "thinking"
                })
            }
        );
        assert_eq!(
            replay[2],
            InputItem::Json {
                value: json!({
                    "type": "tool_call_output",
                    "tool_name": "search",
                    "output": {
                        "type": "text",
                        "text": "found"
                    },
                    "call_id": "call-1",
                    "namespace": null
                })
            }
        );
    }

    #[test]
    fn exposes_durable_state() {
        let context = crate::run_context::RunContextWrapper::new(RunContext::default());
        let state = RunState::new(
            &context,
            vec![InputItem::from("hello")],
            Agent::builder("assistant").build(),
            4,
        )
        .expect("state should build");
        let result = RunResult {
            run_state: Some(state),
            ..RunResult::default()
        };

        assert!(result.durable_state().is_some());
    }

    #[test]
    fn exposes_conversation_metadata_and_response_replay() {
        let result = RunResult {
            conversation_id: Some("conv_123".to_owned()),
            previous_response_id: Some("resp_123".to_owned()),
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
            ..RunResult::default()
        };
        let response = ModelResponse {
            output: vec![OutputItem::Reasoning {
                text: "thinking".to_owned(),
            }],
            usage: Usage::default(),
            model: Some("gpt-5".to_owned()),
            response_id: Some("resp_123".to_owned()),
            request_id: Some("req_123".to_owned()),
        };

        assert_eq!(result.conversation_id(), Some("conv_123"));
        assert_eq!(result.previous_response_id(), Some("resp_123"));
        assert_eq!(response.to_input_items().len(), 1);
    }

    #[test]
    fn exposes_normalized_replay_separately_from_preserve_all_history() {
        let result = RunResult {
            input: vec![InputItem::from("start")],
            normalized_input: Some(vec![InputItem::from("normalized-start")]),
            new_items: vec![
                RunItem::MessageOutput {
                    content: OutputItem::Text {
                        text: "preserve".to_owned(),
                    },
                },
                RunItem::HandoffOutput {
                    source_agent: "router".to_owned(),
                },
            ],
            normalized_new_items: Some(vec![RunItem::MessageOutput {
                content: OutputItem::Text {
                    text: "normalized".to_owned(),
                },
            }]),
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
            ..RunResult::default()
        };

        let preserve_all = result.to_input_list_mode(ToInputListMode::PreserveAll);
        let normalized = result.to_input_list_mode(ToInputListMode::Normalized);

        assert_eq!(preserve_all.len(), 3);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].as_text(), Some("normalized-start"));
        assert_eq!(normalized[1].as_text(), Some("normalized"));
    }
}
