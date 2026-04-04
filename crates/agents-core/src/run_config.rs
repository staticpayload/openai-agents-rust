use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::items::InputItem;
use crate::run_context::{RunContext, RunContextWrapper};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    pub model: Option<String>,
    pub max_turns: usize,
    pub tracing_disabled: bool,
    pub trace_include_sensitive_data: bool,
    pub workflow_name: String,
    pub trace_id: Option<String>,
    pub group_id: Option<String>,
    pub reasoning_item_id_policy: ReasoningItemIdPolicy,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            model: None,
            max_turns: DEFAULT_MAX_TURNS,
            tracing_disabled: false,
            trace_include_sensitive_data: true,
            workflow_name: "Agent workflow".to_owned(),
            trace_id: None,
            group_id: None,
            reasoning_item_id_policy: ReasoningItemIdPolicy::Preserve,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunOptions<TContext = RunContext> {
    pub context: Option<TContext>,
    pub max_turns: Option<usize>,
    pub run_config: Option<RunConfig>,
}
