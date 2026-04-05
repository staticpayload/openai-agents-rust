use std::fmt;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::strict_schema::ensure_strict_json_schema;
use crate::util::transform_string_function_style;

pub mod history;

pub use history::{
    DEFAULT_CONVERSATION_HISTORY_END, DEFAULT_CONVERSATION_HISTORY_START,
    default_handoff_history_mapper, get_conversation_history_wrappers, nest_handoff_history,
    nest_handoff_history_with_mapper, reset_conversation_history_wrappers,
    set_conversation_history_wrappers,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HandoffInputData {
    pub input_history: Vec<crate::items::InputItem>,
    pub pre_handoff_items: Vec<crate::items::RunItem>,
    pub new_items: Vec<crate::items::RunItem>,
    #[serde(default)]
    pub input_items: Option<Vec<crate::items::RunItem>>,
}

impl HandoffInputData {
    pub fn clone_with(&self, input_items: Option<Vec<crate::items::RunItem>>) -> Self {
        let mut cloned = self.clone();
        cloned.input_items = input_items;
        cloned
    }
}

pub type HandoffInputFilter =
    Arc<dyn Fn(HandoffInputData) -> BoxFuture<'static, HandoffInputData> + Send + Sync>;
pub type HandoffHistoryMapper =
    Arc<dyn Fn(Vec<crate::items::InputItem>) -> Vec<crate::items::InputItem> + Send + Sync>;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Handoff {
    pub target: String,
    pub description: Option<String>,
    pub tool_name: String,
    pub tool_description: String,
    pub input_json_schema: Value,
    pub nest_handoff_history: Option<bool>,
    pub strict_json_schema: bool,
    pub enabled: bool,
    #[serde(skip, default)]
    pub input_filter: Option<HandoffInputFilter>,
    #[serde(skip, default)]
    pub history_mapper: Option<HandoffHistoryMapper>,
    #[serde(skip, default)]
    pub agent: Option<Box<Agent>>,
}

impl fmt::Debug for Handoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handoff")
            .field("target", &self.target)
            .field("description", &self.description)
            .field("tool_name", &self.tool_name)
            .field("tool_description", &self.tool_description)
            .field("input_json_schema", &self.input_json_schema)
            .field("nest_handoff_history", &self.nest_handoff_history)
            .field("strict_json_schema", &self.strict_json_schema)
            .field("enabled", &self.enabled)
            .field("has_input_filter", &self.input_filter.is_some())
            .field("has_history_mapper", &self.history_mapper.is_some())
            .field("has_agent", &self.agent.is_some())
            .finish()
    }
}

impl Handoff {
    pub fn new(target: impl Into<String>) -> Self {
        let target = target.into();
        let tool_name = transform_string_function_style(&format!("transfer_to_{target}"));
        let tool_description = format!("Handoff to the {target} agent to handle the request.");
        Self {
            target,
            description: None,
            tool_name,
            tool_description,
            input_json_schema: Value::Object(Default::default()),
            nest_handoff_history: None,
            strict_json_schema: true,
            enabled: true,
            input_filter: None,
            history_mapper: None,
            agent: None,
        }
    }

    pub fn to_agent(agent: Agent) -> Self {
        let description = agent.instructions.clone();
        Self::new(agent.name.clone())
            .with_description(description.unwrap_or_default())
            .with_agent(agent)
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_agent(mut self, agent: Agent) -> Self {
        self.target = agent.name.clone();
        self.tool_name = Self::default_tool_name(&agent);
        self.tool_description = Self::default_tool_description(&agent);
        self.agent = Some(Box::new(agent));
        self
    }

    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = transform_string_function_style(&tool_name.into());
        self
    }

    pub fn with_tool_description(mut self, tool_description: impl Into<String>) -> Self {
        self.tool_description = tool_description.into();
        self
    }

    pub fn with_input_json_schema(mut self, schema: Value) -> Self {
        self.input_json_schema = ensure_strict_json_schema(schema.clone()).unwrap_or(schema);
        self
    }

    pub fn with_input_filter(mut self, input_filter: HandoffInputFilter) -> Self {
        self.input_filter = Some(input_filter);
        self
    }

    pub fn with_history_mapper(mut self, history_mapper: HandoffHistoryMapper) -> Self {
        self.history_mapper = Some(history_mapper);
        self
    }

    pub fn with_nest_handoff_history(mut self, nest_handoff_history: bool) -> Self {
        self.nest_handoff_history = Some(nest_handoff_history);
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn runtime_agent(&self) -> Option<&Agent> {
        self.agent.as_deref()
    }

    pub fn get_transfer_message(&self, agent: &Agent) -> String {
        serde_json::json!({ "assistant": agent.name }).to_string()
    }

    pub fn default_tool_name(agent: &Agent) -> String {
        transform_string_function_style(&format!("transfer_to_{}", agent.name))
    }

    pub fn default_tool_description(agent: &Agent) -> String {
        format!(
            "Handoff to the {} agent to handle the request. {}",
            agent.name,
            agent.instructions.clone().unwrap_or_default()
        )
        .trim()
        .to_owned()
    }
}

#[derive(Clone, Debug, Default)]
pub struct HandoffBuilder {
    handoff: Handoff,
}

impl HandoffBuilder {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            handoff: Handoff::new(target),
        }
    }

    pub fn agent(mut self, agent: Agent) -> Self {
        self.handoff = self.handoff.with_agent(agent);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.handoff = self.handoff.with_description(description);
        self
    }

    pub fn tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.handoff = self.handoff.with_tool_name(tool_name);
        self
    }

    pub fn tool_description(mut self, tool_description: impl Into<String>) -> Self {
        self.handoff = self.handoff.with_tool_description(tool_description);
        self
    }

    pub fn input_json_schema(mut self, schema: Value) -> Self {
        self.handoff = self.handoff.with_input_json_schema(schema);
        self
    }

    pub fn nest_handoff_history(mut self, value: bool) -> Self {
        self.handoff = self.handoff.with_nest_handoff_history(value);
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.handoff = self.handoff.with_enabled(value);
        self
    }

    pub fn build(self) -> Handoff {
        self.handoff
    }
}

pub fn handoff(agent: Agent) -> Handoff {
    Handoff::to_agent(agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_runtime_agent_target() {
        let specialist = Agent::builder("specialist").build();
        let handoff = Handoff::to_agent(specialist.clone());

        assert_eq!(handoff.target, "specialist");
        assert_eq!(handoff.tool_name, "transfer_to_specialist");
        assert_eq!(
            handoff.runtime_agent().map(|agent| agent.name.as_str()),
            Some("specialist")
        );
    }
}
