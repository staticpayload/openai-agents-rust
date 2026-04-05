use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeEventInfo {
    pub session_id: Option<String>,
    pub agent_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeAgentStartEvent {
    pub info: RealtimeEventInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeAgentEndEvent {
    pub info: RealtimeEventInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeHandoffEvent {
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeToolStart {
    pub call_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeToolEnd {
    pub call_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeToolApprovalRequired {
    pub call_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeRawModelEvent {
    pub event_type: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeTranscriptDeltaEvent {
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeInterruptedEvent {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionUpdatedEvent {
    pub info: RealtimeEventInfo,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionClosedEvent {
    pub info: RealtimeEventInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeErrorEvent {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RealtimeEvent {
    AgentStart(RealtimeAgentStartEvent),
    AgentEnd(RealtimeAgentEndEvent),
    Handoff(RealtimeHandoffEvent),
    ToolStart(RealtimeToolStart),
    ToolEnd(RealtimeToolEnd),
    ToolApprovalRequired(RealtimeToolApprovalRequired),
    RawModelEvent(RealtimeRawModelEvent),
    TranscriptDelta(RealtimeTranscriptDeltaEvent),
    Interrupted(RealtimeInterruptedEvent),
    SessionUpdated(RealtimeSessionUpdatedEvent),
    SessionClosed(RealtimeSessionClosedEvent),
    Error(RealtimeErrorEvent),
}
