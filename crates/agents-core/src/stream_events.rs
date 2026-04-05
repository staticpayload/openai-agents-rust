use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::items::RunItem;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawResponsesStreamEvent {
    pub type_name: String,
    pub data: serde_json::Value,
}

impl RawResponsesStreamEvent {
    pub fn event_type(&self) -> &'static str {
        "raw_response_event"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunItemStreamEvent {
    pub name: String,
    pub item: RunItem,
}

impl RunItemStreamEvent {
    pub fn event_type(&self) -> &'static str {
        "run_item_stream_event"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentUpdatedStreamEvent {
    pub new_agent: Agent,
}

impl AgentUpdatedStreamEvent {
    pub fn event_type(&self) -> &'static str {
        "agent_updated_stream_event"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunLifecycleStreamEvent {
    pub name: String,
    pub data: Option<Value>,
}

impl RunLifecycleStreamEvent {
    pub fn event_type(&self) -> &'static str {
        "run_lifecycle_event"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StreamEvent {
    RawResponseEvent(RawResponsesStreamEvent),
    RunItemEvent(RunItemStreamEvent),
    AgentUpdated(AgentUpdatedStreamEvent),
    Lifecycle(RunLifecycleStreamEvent),
}
