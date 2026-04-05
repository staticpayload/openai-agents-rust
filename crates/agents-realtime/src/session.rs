use std::sync::Arc;

use agents_core::Result;
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use tokio::sync::{Mutex, Notify};

use crate::agent::RealtimeAgent;
use crate::config::RealtimeSessionModelSettings;
use crate::events::{
    RealtimeAgentEndEvent, RealtimeAgentStartEvent, RealtimeErrorEvent, RealtimeEvent,
    RealtimeEventInfo, RealtimeInterruptedEvent, RealtimeRawModelEvent, RealtimeSessionClosedEvent,
    RealtimeSessionUpdatedEvent, RealtimeToolStart, RealtimeTranscriptDeltaEvent,
};
use crate::model::{RealtimeModel, RealtimePlaybackState};
use crate::model_events::RealtimeModelEvent;

#[derive(Clone, Debug, Default)]
struct RealtimeSessionState {
    connected: bool,
    closed: bool,
    transcript: String,
    events: Vec<RealtimeEvent>,
    active_agent: Option<RealtimeAgent>,
    model_settings: Option<RealtimeSessionModelSettings>,
    playback_state: RealtimePlaybackState,
}

#[derive(Debug, Default)]
struct LiveRealtimeSessionState {
    state: Mutex<RealtimeSessionState>,
    notify: Notify,
}

impl LiveRealtimeSessionState {
    async fn push_event(&self, event: RealtimeEvent) {
        let mut state = self.state.lock().await;
        if let RealtimeEvent::TranscriptDelta(delta) = &event {
            state.transcript.push_str(&delta.text);
        }
        match &event {
            RealtimeEvent::RawModelEvent(raw) if raw.event_type == "audio" => {
                state.playback_state.playing = true;
                state.playback_state.buffered_audio_ms = raw
                    .payload
                    .get("bytes")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default();
            }
            RealtimeEvent::RawModelEvent(raw) if raw.event_type == "audio_done" => {
                state.playback_state.playing = false;
                state.playback_state.buffered_audio_ms = 0;
            }
            RealtimeEvent::Interrupted(_) => {
                state.playback_state.playing = false;
                state.playback_state.buffered_audio_ms = 0;
            }
            RealtimeEvent::AgentEnd(_) => {
                state.playback_state.playing = false;
            }
            _ => {}
        }
        state.events.push(event);
        drop(state);
        self.notify.notify_waiters();
    }

    async fn push_events(&self, events: Vec<RealtimeEvent>) {
        for event in events {
            self.push_event(event).await;
        }
    }

    async fn event_at(&self, index: usize) -> Option<RealtimeEvent> {
        self.state.lock().await.events.get(index).cloned()
    }

    async fn is_closed(&self) -> bool {
        self.state.lock().await.closed
    }

    async fn wait_for_change(&self) {
        self.notify.notified().await;
    }
}

#[derive(Clone)]
pub struct RealtimeSession {
    pub model: Option<String>,
    session_id: String,
    shared_state: Arc<LiveRealtimeSessionState>,
    model_driver: Arc<Mutex<Option<Box<dyn RealtimeModel>>>>,
}

impl std::fmt::Debug for RealtimeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealtimeSession")
            .field("model", &self.model)
            .field("session_id", &self.session_id)
            .finish()
    }
}

impl RealtimeSession {
    pub fn new(model: Option<String>) -> Self {
        let session_id = format!("realtime:{}", model.as_deref().unwrap_or("default"));
        Self {
            model,
            session_id,
            shared_state: Arc::new(LiveRealtimeSessionState::default()),
            model_driver: Arc::new(Mutex::new(None)),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) async fn attach_model_driver(&self, model_driver: Box<dyn RealtimeModel>) {
        *self.model_driver.lock().await = Some(model_driver);
    }

    pub fn stream_events(&self) -> BoxStream<'static, RealtimeEvent> {
        let shared_state = self.shared_state.clone();
        stream::unfold((shared_state, 0usize), |(shared_state, index)| async move {
            loop {
                if let Some(event) = shared_state.event_at(index).await {
                    return Some((event, (shared_state, index + 1)));
                }
                if shared_state.is_closed().await {
                    return None;
                }
                shared_state.wait_for_change().await;
            }
        })
        .boxed()
    }

    pub async fn transcript(&self) -> String {
        self.shared_state.state.lock().await.transcript.clone()
    }

    pub async fn events(&self) -> Vec<RealtimeEvent> {
        self.shared_state.state.lock().await.events.clone()
    }

    pub async fn connected(&self) -> bool {
        self.shared_state.state.lock().await.connected
    }

    pub async fn active_agent(&self) -> Option<RealtimeAgent> {
        self.shared_state.state.lock().await.active_agent.clone()
    }

    pub async fn model_settings(&self) -> Option<RealtimeSessionModelSettings> {
        self.shared_state.state.lock().await.model_settings.clone()
    }

    pub async fn playback_state(&self) -> RealtimePlaybackState {
        self.shared_state.state.lock().await.playback_state.clone()
    }

    pub async fn connect(&self, agent: Option<RealtimeAgent>) -> Result<()> {
        if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
            model_driver.connect().await?;
        }

        {
            let mut state = self.shared_state.state.lock().await;
            state.connected = true;
            if let Some(agent) = agent.clone() {
                state.active_agent = Some(agent);
            }
        }

        if let Some(agent) = agent {
            self.shared_state
                .push_event(RealtimeEvent::AgentStart(RealtimeAgentStartEvent {
                    info: RealtimeEventInfo {
                        session_id: Some(self.session_id.clone()),
                        agent_name: Some(agent.name),
                    },
                }))
                .await;
        }
        Ok(())
    }

    pub async fn send_text(&self, text: &str) -> Result<Vec<RealtimeEvent>> {
        if !self.connected().await {
            self.connect(None).await?;
        }

        let model_events = if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
            model_driver.send_text(text).await?
        } else {
            Vec::new()
        };

        let events = if model_events.is_empty() {
            vec![RealtimeEvent::TranscriptDelta(
                RealtimeTranscriptDeltaEvent {
                    text: text.to_owned(),
                },
            )]
        } else {
            realtime_events_from_model_events(model_events)
        };
        self.shared_state.push_events(events.clone()).await;
        Ok(events)
    }

    pub async fn send_audio(&self, bytes: &[u8]) -> Result<Vec<RealtimeEvent>> {
        if !self.connected().await {
            self.connect(None).await?;
        }

        let mut events = if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
            realtime_events_from_model_events(model_driver.send_audio(bytes).await?)
        } else {
            Vec::new()
        };
        if events.is_empty() {
            events.push(RealtimeEvent::RawModelEvent(RealtimeRawModelEvent {
                event_type: "audio_input".to_owned(),
                payload: serde_json::json!({ "bytes": bytes.len() }),
            }));
        }
        self.shared_state.push_events(events.clone()).await;
        Ok(events)
    }

    pub async fn interrupt(&self, reason: Option<String>) -> Result<RealtimeEvent> {
        let mut events = if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
            realtime_events_from_model_events(model_driver.interrupt().await?)
        } else {
            Vec::new()
        };
        let interrupted = RealtimeEvent::Interrupted(RealtimeInterruptedEvent { reason });
        events.push(interrupted.clone());
        self.shared_state.push_events(events).await;
        Ok(interrupted)
    }

    pub async fn update_agent(&self, agent: RealtimeAgent) -> Result<RealtimeEvent> {
        let previous_agent = self.active_agent().await;
        if let Some(model_settings) = &agent.model_settings {
            if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
                let model_events = model_driver.update_session(model_settings).await?;
                self.shared_state
                    .push_events(realtime_events_from_model_events(model_events))
                    .await;
            }
        }

        {
            let mut state = self.shared_state.state.lock().await;
            state.active_agent = Some(agent.clone());
            state.model_settings = agent.model_settings.clone();
        }

        if previous_agent.as_ref().map(|current| current.name.as_str()) != Some(agent.name.as_str())
        {
            if let Some(previous_agent) = previous_agent {
                self.shared_state
                    .push_event(RealtimeEvent::AgentEnd(RealtimeAgentEndEvent {
                        info: RealtimeEventInfo {
                            session_id: Some(self.session_id.clone()),
                            agent_name: Some(previous_agent.name),
                        },
                    }))
                    .await;
            }
            self.shared_state
                .push_event(RealtimeEvent::AgentStart(RealtimeAgentStartEvent {
                    info: RealtimeEventInfo {
                        session_id: Some(self.session_id.clone()),
                        agent_name: Some(agent.name.clone()),
                    },
                }))
                .await;
        }

        let event = RealtimeEvent::SessionUpdated(RealtimeSessionUpdatedEvent {
            info: RealtimeEventInfo {
                session_id: Some(self.session_id.clone()),
                agent_name: Some(agent.name),
            },
            model: agent
                .model_settings
                .as_ref()
                .and_then(|settings| settings.model_name.clone())
                .or_else(|| self.model.clone()),
        });
        self.shared_state.push_event(event.clone()).await;
        Ok(event)
    }

    pub async fn close(&self) -> Result<RealtimeEvent> {
        if let Some(model_driver) = self.model_driver.lock().await.as_mut() {
            model_driver.disconnect().await?;
        }

        let active_agent = self.active_agent().await;
        {
            let mut state = self.shared_state.state.lock().await;
            state.connected = false;
            state.closed = true;
        }

        if let Some(active_agent) = active_agent {
            self.shared_state
                .push_event(RealtimeEvent::AgentEnd(RealtimeAgentEndEvent {
                    info: RealtimeEventInfo {
                        session_id: Some(self.session_id.clone()),
                        agent_name: Some(active_agent.name),
                    },
                }))
                .await;
        }

        let event = RealtimeEvent::SessionClosed(RealtimeSessionClosedEvent {
            info: RealtimeEventInfo {
                session_id: Some(self.session_id.clone()),
                agent_name: None,
            },
        });
        self.shared_state.push_event(event.clone()).await;
        Ok(event)
    }
}

fn realtime_events_from_model_events(events: Vec<RealtimeModelEvent>) -> Vec<RealtimeEvent> {
    events
        .into_iter()
        .flat_map(|event| match event {
            RealtimeModelEvent::Error(error) => vec![RealtimeEvent::Error(RealtimeErrorEvent {
                message: error.message,
            })],
            RealtimeModelEvent::ToolCall(call) => vec![
                RealtimeEvent::ToolStart(RealtimeToolStart {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                }),
                RealtimeEvent::RawModelEvent(RealtimeRawModelEvent {
                    event_type: "tool_call".to_owned(),
                    payload: serde_json::json!({
                        "call_id": call.call_id,
                        "name": call.name,
                        "arguments": call.arguments,
                    }),
                }),
            ],
            RealtimeModelEvent::Audio(audio) => {
                vec![RealtimeEvent::RawModelEvent(RealtimeRawModelEvent {
                    event_type: "audio".to_owned(),
                    payload: serde_json::json!({ "bytes": audio.bytes.len() }),
                })]
            }
            RealtimeModelEvent::AudioInterrupted(interrupted) => {
                vec![RealtimeEvent::Interrupted(RealtimeInterruptedEvent {
                    reason: interrupted.reason,
                })]
            }
            RealtimeModelEvent::AudioDone(done) => {
                vec![RealtimeEvent::RawModelEvent(RealtimeRawModelEvent {
                    event_type: "audio_done".to_owned(),
                    payload: serde_json::json!({ "total_bytes": done.total_bytes }),
                })]
            }
            RealtimeModelEvent::TranscriptDelta(delta) => {
                vec![RealtimeEvent::TranscriptDelta(
                    RealtimeTranscriptDeltaEvent { text: delta.text },
                )]
            }
            RealtimeModelEvent::ResponseDone(done) => {
                vec![RealtimeEvent::RawModelEvent(RealtimeRawModelEvent {
                    event_type: "response_done".to_owned(),
                    payload: serde_json::json!({
                        "response_id": done.response_id,
                        "request_id": done.request_id,
                        "payload": done.payload,
                    }),
                })]
            }
        })
        .collect()
}
