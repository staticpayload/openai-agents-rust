use std::sync::Arc;

use agents_core::{AgentsError, Result};
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

use crate::events::VoiceStreamEvent;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct StreamedAudioSnapshot {
    transcript: Vec<String>,
    audio_chunks: usize,
    events: Vec<VoiceStreamEvent>,
}

#[derive(Debug, Default)]
struct LiveAudioStreamState {
    snapshot: Mutex<StreamedAudioSnapshot>,
    completion: Mutex<Option<std::result::Result<StreamedAudioSnapshot, String>>>,
    notify: Notify,
}

impl LiveAudioStreamState {
    async fn push_event(&self, event: VoiceStreamEvent) {
        let mut snapshot = self.snapshot.lock().await;
        if matches!(event, VoiceStreamEvent::Audio(_)) {
            snapshot.audio_chunks = snapshot.audio_chunks.saturating_add(1);
        }
        snapshot.events.push(event);
        drop(snapshot);
        self.notify.notify_waiters();
    }

    async fn push_transcript(&self, text: String) {
        let mut snapshot = self.snapshot.lock().await;
        snapshot.transcript.push(text);
        drop(snapshot);
        self.notify.notify_waiters();
    }

    async fn event_at(&self, index: usize) -> Option<VoiceStreamEvent> {
        self.snapshot.lock().await.events.get(index).cloned()
    }

    async fn set_completion(&self, completion: std::result::Result<StreamedAudioSnapshot, String>) {
        *self.completion.lock().await = Some(completion);
        self.notify.notify_waiters();
    }

    async fn completion(&self) -> Option<std::result::Result<StreamedAudioSnapshot, String>> {
        self.completion.lock().await.clone()
    }

    async fn wait_for_change(&self) {
        self.notify.notified().await;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StreamedAudioResult {
    pub transcript: Vec<String>,
    pub audio_chunks: usize,
    pub events: Vec<VoiceStreamEvent>,
    #[serde(skip, default)]
    shared_state: Option<Arc<LiveAudioStreamState>>,
}

impl StreamedAudioResult {
    fn from_live(shared_state: Arc<LiveAudioStreamState>) -> Self {
        Self {
            transcript: Vec::new(),
            audio_chunks: 0,
            events: Vec::new(),
            shared_state: Some(shared_state),
        }
    }

    fn from_snapshot(snapshot: StreamedAudioSnapshot) -> Self {
        Self {
            transcript: snapshot.transcript,
            audio_chunks: snapshot.audio_chunks,
            events: snapshot.events,
            shared_state: None,
        }
    }

    pub fn stream_events(&self) -> BoxStream<'static, VoiceStreamEvent> {
        if let Some(shared_state) = &self.shared_state {
            let shared_state = shared_state.clone();
            stream::unfold((shared_state, 0usize), |(shared_state, index)| async move {
                loop {
                    if let Some(event) = shared_state.event_at(index).await {
                        return Some((event, (shared_state, index + 1)));
                    }
                    if shared_state.completion().await.is_some() {
                        return None;
                    }
                    shared_state.wait_for_change().await;
                }
            })
            .boxed()
        } else {
            stream::iter(self.events.clone()).boxed()
        }
    }

    pub async fn wait_for_completion(&self) -> Result<StreamedAudioResult> {
        if self.shared_state.is_none() {
            return Ok(self.clone());
        }

        let shared_state = self
            .shared_state
            .as_ref()
            .ok_or_else(|| AgentsError::message("missing live audio stream state"))?;
        loop {
            if let Some(completion) = shared_state.completion().await {
                return completion
                    .map(StreamedAudioResult::from_snapshot)
                    .map_err(AgentsError::message);
            }
            shared_state.wait_for_change().await;
        }
    }
}

pub(crate) struct VoiceStreamRecorder {
    shared_state: Arc<LiveAudioStreamState>,
    stream_audio: bool,
}

impl VoiceStreamRecorder {
    pub(crate) fn new(stream_audio: bool) -> Self {
        Self {
            shared_state: Arc::new(LiveAudioStreamState::default()),
            stream_audio,
        }
    }

    pub(crate) fn result(&self) -> StreamedAudioResult {
        StreamedAudioResult::from_live(self.shared_state.clone())
    }

    pub(crate) async fn push_transcript(&self, text: String) {
        self.shared_state.push_transcript(text).await;
    }

    pub(crate) async fn push_events(&self, events: Vec<VoiceStreamEvent>) {
        for event in events {
            if !self.stream_audio && matches!(event, VoiceStreamEvent::Audio(_)) {
                continue;
            }
            self.shared_state.push_event(event).await;
        }
    }

    pub(crate) async fn complete(&self) {
        let snapshot = self.shared_state.snapshot.lock().await.clone();
        self.shared_state.set_completion(Ok(snapshot)).await;
    }

    pub(crate) async fn fail(&self, error: agents_core::AgentsError) {
        self.shared_state
            .set_completion(Err(error.to_string()))
            .await;
    }
}
