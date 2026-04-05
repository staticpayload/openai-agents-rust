use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use agents_core::{Result, default_openai_key};

use crate::config::RealtimeSessionModelSettings;
use crate::model::{RealtimeModel, RealtimeModelConfig};
use crate::model_events::{
    RealtimeModelAudioDoneEvent, RealtimeModelAudioInterruptedEvent, RealtimeModelEvent,
    RealtimeModelResponseDoneEvent, RealtimeModelTranscriptDeltaEvent,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportConfig {
    pub api_key: Option<String>,
    pub websocket_url: Option<String>,
}

pub fn get_api_key(config: &TransportConfig) -> Option<String> {
    config.api_key.clone().or_else(default_openai_key)
}

pub fn get_server_event_type_adapter(event_type: &str) -> &str {
    match event_type {
        "response.audio_transcript.delta" => "transcript_delta",
        "response.done" => "response_done",
        other => other,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpenAIRealtimeWebSocketModel {
    pub config: RealtimeModelConfig,
    pub transport: TransportConfig,
    pub connected: bool,
}

#[async_trait]
impl RealtimeModel for OpenAIRealtimeWebSocketModel {
    async fn connect(&mut self) -> Result<()> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn send_text(&mut self, text: &str) -> Result<Vec<RealtimeModelEvent>> {
        Ok(vec![
            RealtimeModelEvent::TranscriptDelta(RealtimeModelTranscriptDeltaEvent {
                text: text.to_owned(),
            }),
            RealtimeModelEvent::ResponseDone(RealtimeModelResponseDoneEvent {
                response_id: None,
                request_id: None,
                payload: Some(Value::String(text.to_owned())),
            }),
        ])
    }

    async fn send_audio(&mut self, bytes: &[u8]) -> Result<Vec<RealtimeModelEvent>> {
        Ok(vec![RealtimeModelEvent::AudioDone(
            RealtimeModelAudioDoneEvent {
                total_bytes: bytes.len(),
            },
        )])
    }

    async fn interrupt(&mut self) -> Result<Vec<RealtimeModelEvent>> {
        Ok(vec![RealtimeModelEvent::AudioInterrupted(
            RealtimeModelAudioInterruptedEvent {
                reason: Some("interrupted".to_owned()),
            },
        )])
    }

    async fn update_session(
        &mut self,
        settings: &RealtimeSessionModelSettings,
    ) -> Result<Vec<RealtimeModelEvent>> {
        if let Some(model_name) = &settings.model_name {
            self.config.model = Some(model_name.clone());
        }
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpenAIRealtimeSIPModel {
    pub config: RealtimeModelConfig,
    pub transport: TransportConfig,
}
