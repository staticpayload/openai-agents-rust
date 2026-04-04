use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::items::{InputItem, OutputItem};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RunItem {
    Input(InputItem),
    Output(OutputItem),
    ToolCall { name: String },
    Handoff { target: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawResponsesStreamEvent {
    pub type_name: String,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunItemStreamEvent {
    pub name: String,
    pub item: RunItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentUpdatedStreamEvent {
    pub new_agent: Agent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StreamEvent {
    RawResponseEvent(RawResponsesStreamEvent),
    RunItemEvent(RunItemStreamEvent),
    AgentUpdated(AgentUpdatedStreamEvent),
}
