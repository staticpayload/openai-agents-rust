use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use agents_core::{AgentsError, Result, default_openai_key};

use crate::RealtimeAudioFormat;
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
    pub call_id: Option<String>,
    #[serde(default)]
    pub query_params: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NormalizedRealtimeSessionPayload {
    pub model: Option<String>,
    pub input_audio_format: Option<RealtimeAudioFormat>,
    pub output_audio_format: Option<RealtimeAudioFormat>,
    pub payload: Value,
}

pub fn get_api_key(config: &TransportConfig) -> Option<String> {
    config.api_key.clone().or_else(default_openai_key)
}

pub fn get_server_event_type_adapter(event_type: &str) -> &str {
    match event_type {
        "response.audio_transcript.delta" | "response.output_audio_transcript.delta" => {
            "transcript_delta"
        }
        "response.done" => "response_done",
        other => other,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpenAIRealtimeWebSocketModel {
    pub config: RealtimeModelConfig,
    pub transport: TransportConfig,
    pub connected: bool,
    pub last_session_payload: Option<NormalizedRealtimeSessionPayload>,
}

impl OpenAIRealtimeWebSocketModel {
    pub fn connection_url(&self) -> String {
        let base = self
            .transport
            .websocket_url
            .clone()
            .unwrap_or_else(|| "wss://api.openai.com/v1/realtime".to_owned());
        let mut url = if base.starts_with("https://") {
            base.replacen("https://", "wss://", 1)
        } else if base.starts_with("http://") {
            base.replacen("http://", "ws://", 1)
        } else {
            base
        };

        let mut query_params = self.transport.query_params.clone();
        if let Some(call_id) = &self.transport.call_id {
            query_params
                .entry("call_id".to_owned())
                .or_insert(call_id.clone());
        } else if let Some(model) = &self.config.model {
            query_params
                .entry("model".to_owned())
                .or_insert_with(|| model.clone());
        }

        if query_params.is_empty() {
            return url;
        }

        let separator = if url.contains('?') { '&' } else { '?' };
        let query = query_params
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        url.push(separator);
        url.push_str(&query);
        url
    }

    pub fn normalize_session_payload(payload: &Value) -> Option<NormalizedRealtimeSessionPayload> {
        let session_type = payload.get("type").and_then(Value::as_str)?;
        if session_type == "transcription" {
            return None;
        }
        if session_type != "realtime" {
            return None;
        }

        Some(NormalizedRealtimeSessionPayload {
            model: payload
                .get("model")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            input_audio_format: payload
                .get("audio")
                .and_then(|audio| audio.get("input"))
                .and_then(|input| input.get("format"))
                .and_then(Self::audio_format_from_value),
            output_audio_format: payload
                .get("audio")
                .and_then(|audio| audio.get("output"))
                .and_then(|output| output.get("format"))
                .and_then(Self::audio_format_from_value),
            payload: payload.clone(),
        })
    }

    fn audio_format_from_value(value: &Value) -> Option<RealtimeAudioFormat> {
        match value {
            Value::String(format) => Some(crate::to_realtime_audio_format(format)),
            Value::Object(map) => map
                .get("type")
                .and_then(Value::as_str)
                .map(crate::to_realtime_audio_format),
            _ => None,
        }
    }

    fn audio_format_payload(format: &RealtimeAudioFormat) -> Value {
        match format {
            RealtimeAudioFormat::Pcm16 => serde_json::json!({
                "type": "audio/pcm",
                "rate": 24_000,
            }),
            RealtimeAudioFormat::G711Ulaw => serde_json::json!({
                "type": "audio/pcmu",
            }),
            RealtimeAudioFormat::G711Alaw => serde_json::json!({
                "type": "audio/pcma",
            }),
            RealtimeAudioFormat::Custom(custom) => Value::String(custom.clone()),
        }
    }

    fn session_payload_from_settings(&self, settings: &RealtimeSessionModelSettings) -> Value {
        let input_audio_format = settings
            .audio
            .as_ref()
            .and_then(|audio| audio.input.as_ref())
            .and_then(|input| input.format.clone())
            .or_else(|| settings.input_audio_format.clone());
        let output_audio_format = settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.format.clone())
            .or_else(|| settings.output_audio_format.clone());

        let mut payload = serde_json::json!({
            "type": "realtime",
            "model": settings.model_name.clone().or_else(|| self.config.model.clone()),
        });
        let Some(session) = payload.as_object_mut() else {
            return payload;
        };

        let mut audio = serde_json::Map::new();
        if let Some(input_audio_format) = input_audio_format {
            audio.insert(
                "input".to_owned(),
                serde_json::json!({
                    "format": Self::audio_format_payload(&input_audio_format),
                }),
            );
        }
        if let Some(output_audio_format) = output_audio_format {
            audio.insert(
                "output".to_owned(),
                serde_json::json!({
                    "format": Self::audio_format_payload(&output_audio_format),
                    "voice": settings.voice.clone(),
                }),
            );
        }

        if !audio.is_empty() {
            session.insert("audio".to_owned(), Value::Object(audio));
        }

        payload
    }
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
        let payload = self.session_payload_from_settings(settings);
        self.last_session_payload = Self::normalize_session_payload(&payload);
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
        if self.transport.call_id.is_none() {
            return Err(AgentsError::message(
                "OpenAIRealtimeSIPModel requires `call_id` in the transport configuration.",
            ));
        }
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
        assert_eq!(
            get_server_event_type_adapter("response.output_audio_transcript.delta"),
            "transcript_delta"
        );
        assert_eq!(get_server_event_type_adapter("custom"), "custom");
    }

    #[test]
    fn normalizes_http_transport_url_and_query_parameters() {
        let model = OpenAIRealtimeWebSocketModel {
            config: RealtimeModelConfig {
                model: Some("gpt-realtime".to_owned()),
            },
            transport: TransportConfig {
                api_key: None,
                websocket_url: Some("https://api.openai.com/v1/realtime".to_owned()),
                call_id: None,
                query_params: BTreeMap::new(),
            },
            connected: false,
            last_session_payload: None,
        };

        assert_eq!(
            model.connection_url(),
            "wss://api.openai.com/v1/realtime?model=gpt-realtime"
        );
    }

    #[test]
    fn normalizes_realtime_session_payload_and_extracts_audio_format() {
        let payload = serde_json::json!({
            "type": "realtime",
            "model": "gpt-realtime-1.5",
            "audio": {
                "output": {
                    "format": { "type": "audio/pcmu" }
                }
            }
        });

        let normalized = OpenAIRealtimeWebSocketModel::normalize_session_payload(&payload)
            .expect("payload should normalize");
        assert_eq!(
            normalized.output_audio_format,
            Some(crate::RealtimeAudioFormat::G711Ulaw)
        );

        let transcription = serde_json::json!({ "type": "transcription" });
        assert!(OpenAIRealtimeWebSocketModel::normalize_session_payload(&transcription).is_none());
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
        let mut model = OpenAIRealtimeSIPModel {
            transport: TransportConfig {
                call_id: Some("call_123".to_owned()),
                ..TransportConfig::default()
            },
            ..OpenAIRealtimeSIPModel::default()
        };
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

    #[tokio::test]
    async fn sip_model_requires_call_id_before_connect() {
        let mut model = OpenAIRealtimeSIPModel::default();
        let error = model.connect().await.expect_err("connect should fail");
        assert!(error.to_string().contains("call_id"));
    }
}
