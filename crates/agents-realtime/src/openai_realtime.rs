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
    pub connected: bool,
}

#[async_trait]
impl RealtimeModel for OpenAIRealtimeSIPModel {
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
                payload: Some(serde_json::json!({
                    "transport": "sip",
                    "text": text,
                })),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_known_server_event_types() {
        assert_eq!(
            get_server_event_type_adapter("response.audio_transcript.delta"),
            "transcript_delta"
        );
        assert_eq!(
            get_server_event_type_adapter("response.done"),
            "response_done"
        );
        assert_eq!(get_server_event_type_adapter("custom"), "custom");
    }

    #[tokio::test]
    async fn websocket_model_tracks_connection_and_updates_session_model() {
        let mut model = OpenAIRealtimeWebSocketModel::default();
        model.connect().await.expect("connect should succeed");
        assert!(model.connected);

        let events = model.send_text("hello").await.expect("text should send");
        assert!(matches!(
            events.first(),
            Some(RealtimeModelEvent::TranscriptDelta(_))
        ));

        model
            .update_session(&RealtimeSessionModelSettings {
                model_name: Some("gpt-realtime-updated".to_owned()),
                ..RealtimeSessionModelSettings::default()
            })
            .await
            .expect("session should update");
        assert_eq!(model.config.model.as_deref(), Some("gpt-realtime-updated"));

        model.disconnect().await.expect("disconnect should succeed");
        assert!(!model.connected);
    }

    #[tokio::test]
    async fn sip_model_supports_text_audio_and_interrupt() {
        let mut model = OpenAIRealtimeSIPModel::default();
        model.connect().await.expect("connect should succeed");
        assert!(model.connected);

        let text_events = model.send_text("hello").await.expect("text should send");
        assert!(matches!(
            text_events.last(),
            Some(RealtimeModelEvent::ResponseDone(_))
        ));

        let audio_events = model
            .send_audio(&[1, 2, 3])
            .await
            .expect("audio should send");
        assert!(matches!(
            audio_events.first(),
            Some(RealtimeModelEvent::AudioDone(_))
        ));

        let interrupt_events = model.interrupt().await.expect("interrupt should succeed");
        assert!(matches!(
            interrupt_events.first(),
            Some(RealtimeModelEvent::AudioInterrupted(_))
        ));
    }
}
