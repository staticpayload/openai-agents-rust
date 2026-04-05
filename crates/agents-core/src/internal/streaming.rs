use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};

use crate::agent::Agent;
use crate::errors::{AgentsError, Result};
use crate::items::RunItem;
use crate::model::ModelResponse;
use crate::result::RunResult;
use crate::stream_events::{
    AgentUpdatedStreamEvent, RawResponsesStreamEvent, RunItemStreamEvent, RunLifecycleStreamEvent,
    StreamEvent,
};

#[derive(Debug, Default)]
pub(crate) struct LiveRunStreamState {
    events: Mutex<Vec<StreamEvent>>,
    completion: Mutex<Option<std::result::Result<RunResult, String>>>,
    notify: Notify,
}

impl LiveRunStreamState {
    pub(crate) async fn event_at(&self, index: usize) -> Option<StreamEvent> {
        self.events.lock().await.get(index).cloned()
    }

    pub(crate) async fn completion(&self) -> Option<std::result::Result<RunResult, String>> {
        self.completion.lock().await.clone()
    }

    pub(crate) async fn wait_for_completion(&self) -> Result<RunResult> {
        loop {
            if let Some(result) = self.completion().await {
                return result.map_err(AgentsError::message);
            }
            self.notify.notified().await;
        }
    }

    pub(crate) async fn wait_for_change(&self) {
        self.notify.notified().await;
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StreamRecorder {
    state: Arc<LiveRunStreamState>,
}

impl StreamRecorder {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(LiveRunStreamState::default()),
        }
    }

    pub(crate) fn shared_state(&self) -> Arc<LiveRunStreamState> {
        self.state.clone()
    }

    pub(crate) async fn push_event(&self, event: StreamEvent) {
        self.state.events.lock().await.push(event);
        self.state.notify.notify_waiters();
    }

    pub(crate) async fn push_lifecycle(&self, name: impl Into<String>, data: Option<Value>) {
        self.push_event(StreamEvent::Lifecycle(RunLifecycleStreamEvent {
            name: name.into(),
            data,
        }))
        .await;
    }

    pub(crate) async fn push_raw_response(&self, response: &ModelResponse) {
        self.push_event(StreamEvent::RawResponseEvent(RawResponsesStreamEvent {
            type_name: "model_response".to_owned(),
            data: serde_json::to_value(response).unwrap_or(Value::Null),
        }))
        .await;
    }

    pub(crate) async fn push_run_items(&self, items: &[RunItem]) {
        for item in items {
            self.push_event(StreamEvent::RunItemEvent(RunItemStreamEvent {
                name: run_item_name(item),
                item: item.clone(),
            }))
            .await;
        }
    }

    pub(crate) async fn push_agent_updated(&self, agent: &Agent) {
        self.push_event(StreamEvent::AgentUpdated(AgentUpdatedStreamEvent {
            new_agent: agent.clone(),
        }))
        .await;
    }

    pub(crate) async fn complete(&self, result: Result<RunResult>) {
        match &result {
            Ok(run_result) => {
                self.push_lifecycle(
                    "run_completed",
                    Some(json!({
                        "agent_name": run_result.agent_name,
                        "final_output": run_result.final_output,
                    })),
                )
                .await;
            }
            Err(error) => {
                self.push_lifecycle(
                    "run_failed",
                    Some(json!({
                        "message": error.to_string(),
                    })),
                )
                .await;
            }
        }

        let stored = result.map_err(|error| error.to_string());
        *self.state.completion.lock().await = Some(stored);
        self.state.notify.notify_waiters();
    }
}

pub(crate) fn result_to_stream_events(
    initial_agent: &Agent,
    result: &RunResult,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    events.push(StreamEvent::Lifecycle(RunLifecycleStreamEvent {
        name: "agent_start".to_owned(),
        data: Some(json!({
            "agent_name": initial_agent.name,
            "turn": 0,
        })),
    }));
    for response in &result.raw_responses {
        events.push(StreamEvent::RawResponseEvent(RawResponsesStreamEvent {
            type_name: "model_response".to_owned(),
            data: serde_json::to_value(response).unwrap_or(serde_json::Value::Null),
        }));
    }
    for item in &result.new_items {
        events.push(StreamEvent::RunItemEvent(RunItemStreamEvent {
            name: run_item_name(item),
            item: item.clone(),
        }));
    }
    if let Some(last_agent) = &result.last_agent {
        if last_agent.name != initial_agent.name {
            events.push(StreamEvent::Lifecycle(RunLifecycleStreamEvent {
                name: "handoff".to_owned(),
                data: Some(json!({
                    "from_agent": initial_agent.name,
                    "to_agent": last_agent.name,
                })),
            }));
            events.push(StreamEvent::AgentUpdated(AgentUpdatedStreamEvent {
                new_agent: last_agent.clone(),
            }));
        }
        events.push(StreamEvent::Lifecycle(RunLifecycleStreamEvent {
            name: "agent_end".to_owned(),
            data: Some(json!({
                "agent_name": last_agent.name,
                "final_output": result.final_output,
            })),
        }));
    }
    events.push(StreamEvent::Lifecycle(RunLifecycleStreamEvent {
        name: "run_completed".to_owned(),
        data: Some(json!({
            "agent_name": result.agent_name,
            "final_output": result.final_output,
        })),
    }));
    events
}

fn run_item_name(item: &crate::items::RunItem) -> String {
    match item {
        crate::items::RunItem::MessageOutput { .. } => "message_output".to_owned(),
        crate::items::RunItem::ToolCall { .. } => "tool_call".to_owned(),
        crate::items::RunItem::ToolCallOutput { .. } => "tool_call_output".to_owned(),
        crate::items::RunItem::HandoffCall { .. } => "handoff_call".to_owned(),
        crate::items::RunItem::HandoffOutput { .. } => "handoff_output".to_owned(),
        crate::items::RunItem::Reasoning { .. } => "reasoning".to_owned(),
    }
}
