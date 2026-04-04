use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::guardrail::{InputGuardrailResult, OutputGuardrailResult};
use crate::items::{InputItem, OutputItem, RunItem};
use crate::model::ModelResponse;
use crate::run_state::{RunInterruption, RunState, RunStateContextSnapshot};
use crate::tool_guardrails::{ToolInputGuardrailResult, ToolOutputGuardrailResult};
use crate::tracing::Trace;
use crate::usage::Usage;

/// Result of an agent run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunResult {
    pub agent_name: String,
    pub last_agent: Option<Agent>,
    pub input: Vec<InputItem>,
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
}

impl RunResult {
    pub fn final_output_text(&self) -> Option<&str> {
        self.final_output.as_deref()
    }

    pub fn last_agent(&self) -> Option<&Agent> {
        self.last_agent.as_ref()
    }

    pub fn to_input_list(&self) -> Vec<InputItem> {
        let mut items = self.input.clone();
        items.extend(self.new_items.iter().filter_map(RunItem::to_input_item));
        items
    }

    pub fn durable_state(&self) -> Option<&RunState> {
        self.run_state.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::items::OutputItem;
    use crate::run_context::RunContext;

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
}
