use std::collections::BTreeMap;

use agents_core::{AgentsError, Result, VERSION};
use agents_realtime::{RealtimeAudioFormat, RealtimeSessionModelSettings};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Cloudflare Workers websocket upgrade request metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudflareUpgradeRequest {
    pub url: String,
    pub method: String,
    pub headers: BTreeMap<String, String>,
}

/// A socket returned from a Cloudflare-style websocket upgrade.
pub trait CloudflareRealtimeSocket {
    fn accept(&mut self) -> Result<()>;
}

/// Cloudflare Workers transport adapter metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudflareRealtimeTransportLayer {
    pub url: String,
    pub extra_headers: BTreeMap<String, String>,
    pub skip_open_event_listeners: bool,
}

impl CloudflareRealtimeTransportLayer {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            extra_headers: BTreeMap::new(),
            skip_open_event_listeners: true,
        }
    }

    pub fn with_extra_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(key.into(), value.into());
        self
    }

    pub fn build_upgrade_request(&self, api_key: &str) -> Result<CloudflareUpgradeRequest> {
        if self.url.trim().is_empty() {
            return Err(AgentsError::message("Realtime URL is not defined"));
        }

        let url = if self.url.starts_with("wss://") {
            self.url.replacen("wss://", "https://", 1)
        } else if self.url.starts_with("ws://") {
            self.url.replacen("ws://", "http://", 1)
        } else {
            self.url.clone()
        };

        let mut headers = BTreeMap::from([
            ("Authorization".to_owned(), format!("Bearer {api_key}")),
            ("Connection".to_owned(), "Upgrade".to_owned()),
            ("Sec-WebSocket-Protocol".to_owned(), "realtime".to_owned()),
            ("Upgrade".to_owned(), "websocket".to_owned()),
            (
                "User-Agent".to_owned(),
                format!("openai-agents-rust/{VERSION}"),
            ),
            (
                "X-OpenAI-Agents-SDK".to_owned(),
                format!("openai-agents-sdk.{VERSION}"),
            ),
        ]);
        headers.extend(self.extra_headers.clone());

        Ok(CloudflareUpgradeRequest {
            url,
            method: "GET".to_owned(),
            headers,
        })
    }

    pub fn connect_with<S, F>(&self, api_key: &str, upgrader: F) -> Result<S>
    where
        S: CloudflareRealtimeSocket,
        F: FnOnce(CloudflareUpgradeRequest) -> Result<S>,
    {
        let request = self.build_upgrade_request(api_key)?;
        let mut socket = upgrader(request)?;
        socket.accept()?;
        Ok(socket)
    }
}

/// Commands emitted back to the Twilio media stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TwilioOutboundMessage {
    Clear {
        #[serde(rename = "streamSid")]
        stream_sid: Option<String>,
    },
    Media {
        #[serde(rename = "streamSid")]
        stream_sid: Option<String>,
        media: TwilioOutboundMedia,
    },
    Mark {
        #[serde(rename = "streamSid")]
        stream_sid: Option<String>,
        mark: TwilioOutboundMark,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwilioOutboundMedia {
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwilioOutboundMark {
    pub name: String,
}

impl TwilioOutboundMessage {
    pub fn to_json_value(&self) -> Value {
        serde_json::to_value(self).expect("twilio outbound message serializes")
    }
}

/// Normalized effects emitted when Twilio messages are processed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TwilioRealtimeTransportAction {
    ForwardInputAudio { bytes: Vec<u8> },
    StreamStarted { stream_sid: String },
    MarkObserved { name: String },
    InvalidMarkName { name: String },
}

/// Result of computing a Twilio interruption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwilioInterruptDecision {
    pub elapsed_time_ms: u64,
    pub cancel_ongoing_response: bool,
    pub messages: Vec<TwilioOutboundMessage>,
}

/// Twilio media-stream adapter state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwilioRealtimeTransportLayer {
    stream_sid: Option<String>,
    audio_chunk_count: usize,
    last_played_chunk_count: u64,
    previous_item_id: Option<String>,
}

impl TwilioRealtimeTransportLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stream_sid(&self) -> Option<&str> {
        self.stream_sid.as_deref()
    }

    pub fn last_played_chunk_count(&self) -> u64 {
        self.last_played_chunk_count
    }

    pub fn normalize_session_config(
        &self,
        partial: Option<RealtimeSessionModelSettings>,
    ) -> RealtimeSessionModelSettings {
        let mut config = partial.unwrap_or_default();
        config.input_audio_format = config
            .input_audio_format
            .or(Some(RealtimeAudioFormat::G711Ulaw));
        config.output_audio_format = config
            .output_audio_format
            .or(Some(RealtimeAudioFormat::G711Ulaw));
        config
    }

    pub fn handle_incoming_message(
        &mut self,
        message: &str,
        connected: bool,
    ) -> Result<Vec<TwilioRealtimeTransportAction>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|error| AgentsError::message(format!("Invalid Twilio message: {error}")))?;
        let Some(event_type) = value.get("event").and_then(Value::as_str) else {
            return Ok(Vec::new());
        };

        let mut actions = Vec::new();
        match event_type {
            "media" => {
                if connected {
                    if let Some(payload) = value
                        .get("media")
                        .and_then(|media| media.get("payload"))
                        .and_then(Value::as_str)
                    {
                        let bytes = BASE64.decode(payload).map_err(|error| {
                            AgentsError::message(format!("Invalid Twilio media payload: {error}"))
                        })?;
                        actions.push(TwilioRealtimeTransportAction::ForwardInputAudio { bytes });
                    }
                }
            }
            "mark" => {
                if let Some(name) = value
                    .get("mark")
                    .and_then(|mark| mark.get("name"))
                    .and_then(Value::as_str)
                {
                    if name.starts_with("done:") {
                        self.last_played_chunk_count = 0;
                        actions.push(TwilioRealtimeTransportAction::MarkObserved {
                            name: name.to_owned(),
                        });
                    } else if let Some((_, count)) = name.split_once(':') {
                        if let Ok(count) = count.parse::<u64>() {
                            self.last_played_chunk_count = count;
                            actions.push(TwilioRealtimeTransportAction::MarkObserved {
                                name: name.to_owned(),
                            });
                        } else {
                            actions.push(TwilioRealtimeTransportAction::InvalidMarkName {
                                name: name.to_owned(),
                            });
                        }
                    }
                }
            }
            "start" => {
                if let Some(stream_sid) = value
                    .get("start")
                    .and_then(|start| start.get("streamSid"))
                    .and_then(Value::as_str)
                {
                    self.stream_sid = Some(stream_sid.to_owned());
                    self.audio_chunk_count = 0;
                    self.last_played_chunk_count = 0;
                    self.previous_item_id = None;
                    actions.push(TwilioRealtimeTransportAction::StreamStarted {
                        stream_sid: stream_sid.to_owned(),
                    });
                }
            }
            _ => {}
        }

        Ok(actions)
    }

    pub fn interrupt_decision(&self, cancel_ongoing_response: bool) -> TwilioInterruptDecision {
        TwilioInterruptDecision {
            elapsed_time_ms: self.last_played_chunk_count + 50,
            cancel_ongoing_response,
            messages: vec![TwilioOutboundMessage::Clear {
                stream_sid: self.stream_sid.clone(),
            }],
        }
    }

    pub fn audio_messages(
        &mut self,
        current_item_id: Option<&str>,
        bytes: &[u8],
    ) -> Vec<TwilioOutboundMessage> {
        if current_item_id != self.previous_item_id.as_deref() {
            self.previous_item_id = current_item_id.map(ToOwned::to_owned);
            self.audio_chunk_count = 0;
        }

        self.audio_chunk_count += bytes.len() / 8;
        let mut messages = vec![TwilioOutboundMessage::Media {
            stream_sid: self.stream_sid.clone(),
            media: TwilioOutboundMedia {
                payload: BASE64.encode(bytes),
            },
        }];

        if let Some(item_id) = current_item_id {
            messages.push(TwilioOutboundMessage::Mark {
                stream_sid: self.stream_sid.clone(),
                mark: TwilioOutboundMark {
                    name: format!("{item_id}:{}", self.audio_chunk_count),
                },
            });
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default)]
    struct FakeSocket {
        accepted: bool,
        fail_accept: bool,
    }

    impl CloudflareRealtimeSocket for FakeSocket {
        fn accept(&mut self) -> Result<()> {
            if self.fail_accept {
                return Err(AgentsError::message("accept failed"));
            }
            self.accepted = true;
            Ok(())
        }
    }

    #[test]
    fn cloudflare_builds_upgrade_requests() {
        let transport =
            CloudflareRealtimeTransportLayer::new("wss://api.openai.com/v1/realtime?model=foo");
        let request = transport
            .build_upgrade_request("ek_test")
            .expect("upgrade request should build");
        let expected_sdk_header = format!("openai-agents-sdk.{VERSION}");

        assert_eq!(request.url, "https://api.openai.com/v1/realtime?model=foo");
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.headers.get("Authorization").map(String::as_str),
            Some("Bearer ek_test")
        );
        assert_eq!(
            request
                .headers
                .get("Sec-WebSocket-Protocol")
                .map(String::as_str),
            Some("realtime")
        );
        assert!(request.headers.contains_key("User-Agent"));
        assert_eq!(
            request
                .headers
                .get("X-OpenAI-Agents-SDK")
                .map(String::as_str),
            Some(expected_sdk_header.as_str())
        );
    }

    #[test]
    fn cloudflare_surfaces_accept_failures() {
        let transport =
            CloudflareRealtimeTransportLayer::new("wss://api.openai.com/v1/realtime?model=foo");
        let error = transport
            .connect_with("ek_test", |_request| {
                Ok(FakeSocket {
                    accepted: false,
                    fail_accept: true,
                })
            })
            .expect_err("accept error should propagate");
        assert!(error.to_string().contains("accept failed"));
    }

    #[test]
    fn twilio_defaults_session_audio_format() {
        let transport = TwilioRealtimeTransportLayer::new();
        let config = transport.normalize_session_config(None);
        assert_eq!(
            config.input_audio_format,
            Some(RealtimeAudioFormat::G711Ulaw)
        );
        assert_eq!(
            config.output_audio_format,
            Some(RealtimeAudioFormat::G711Ulaw)
        );
    }

    #[test]
    fn twilio_tracks_messages_and_interrupt_state() {
        let mut transport = TwilioRealtimeTransportLayer::new();
        let start_actions = transport
            .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-1"}}"#, true)
            .expect("start should parse");
        assert_eq!(
            start_actions,
            vec![TwilioRealtimeTransportAction::StreamStarted {
                stream_sid: "sid-1".to_owned()
            }]
        );

        let media_actions = transport
            .handle_incoming_message(r#"{"event":"media","media":{"payload":"YQ=="}}"#, true)
            .expect("media should parse");
        assert_eq!(
            media_actions,
            vec![TwilioRealtimeTransportAction::ForwardInputAudio { bytes: vec![b'a'] }]
        );

        let mark_actions = transport
            .handle_incoming_message(r#"{"event":"mark","mark":{"name":"u:7"}}"#, true)
            .expect("mark should parse");
        assert_eq!(
            mark_actions,
            vec![TwilioRealtimeTransportAction::MarkObserved {
                name: "u:7".to_owned()
            }]
        );
        assert_eq!(transport.last_played_chunk_count(), 7);

        transport
            .handle_incoming_message(r#"{"event":"mark","mark":{"name":"done:u"}}"#, true)
            .expect("done mark should parse");
        assert_eq!(transport.last_played_chunk_count(), 0);

        let invalid = transport
            .handle_incoming_message(r#"{"event":"mark","mark":{"name":"u:x"}}"#, true)
            .expect("invalid mark should still parse");
        assert_eq!(
            invalid,
            vec![TwilioRealtimeTransportAction::InvalidMarkName {
                name: "u:x".to_owned()
            }]
        );

        let decision = transport.interrupt_decision(true);
        assert_eq!(decision.elapsed_time_ms, 50);
        assert_eq!(
            decision.messages,
            vec![TwilioOutboundMessage::Clear {
                stream_sid: Some("sid-1".to_owned())
            }]
        );
    }

    #[test]
    fn twilio_start_resets_interrupt_tracking_state() {
        let mut transport = TwilioRealtimeTransportLayer::new();
        transport
            .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-1"}}"#, true)
            .expect("first start should parse");
        transport
            .handle_incoming_message(r#"{"event":"mark","mark":{"name":"u:9"}}"#, true)
            .expect("mark should parse");
        assert_eq!(transport.last_played_chunk_count(), 9);

        let messages = transport.audio_messages(Some("item-a"), &[0; 24]);
        assert_eq!(
            messages.last().map(TwilioOutboundMessage::to_json_value),
            Some(serde_json::json!({
                "event":"mark",
                "streamSid":"sid-1",
                "mark":{"name":"item-a:3"}
            }))
        );

        let start_actions = transport
            .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-2"}}"#, true)
            .expect("second start should parse");
        assert_eq!(
            start_actions,
            vec![TwilioRealtimeTransportAction::StreamStarted {
                stream_sid: "sid-2".to_owned()
            }]
        );
        assert_eq!(transport.last_played_chunk_count(), 0);

        let next_messages = transport.audio_messages(Some("item-a"), &[0; 8]);
        assert_eq!(
            next_messages
                .last()
                .map(TwilioOutboundMessage::to_json_value),
            Some(serde_json::json!({
                "event":"mark",
                "streamSid":"sid-2",
                "mark":{"name":"item-a:1"}
            }))
        );

        let decision = transport.interrupt_decision(true);
        assert_eq!(decision.elapsed_time_ms, 50);
        assert_eq!(
            decision.messages,
            vec![TwilioOutboundMessage::Clear {
                stream_sid: Some("sid-2".to_owned())
            }]
        );
    }

    #[test]
    fn twilio_audio_messages_reset_on_new_item() {
        let mut transport = TwilioRealtimeTransportLayer::new();
        transport.stream_sid = Some("sid-1".to_owned());

        let first = transport.audio_messages(Some("a"), &[0; 8]);
        let second = transport.audio_messages(Some("a"), &[0; 16]);
        let third = transport.audio_messages(Some("b"), &[0; 8]);

        assert_eq!(
            first.last().map(|message| message.to_json_value()),
            Some(serde_json::json!({
                "event":"mark",
                "streamSid":"sid-1",
                "mark":{"name":"a:1"}
            }))
        );
        assert_eq!(
            second.last().map(|message| message.to_json_value()),
            Some(serde_json::json!({
                "event":"mark",
                "streamSid":"sid-1",
                "mark":{"name":"a:3"}
            }))
        );
        assert_eq!(
            third.last().map(|message| message.to_json_value()),
            Some(serde_json::json!({
                "event":"mark",
                "streamSid":"sid-1",
                "mark":{"name":"b:1"}
            }))
        );
    }
}
