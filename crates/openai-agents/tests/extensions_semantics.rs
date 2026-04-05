use openai_agents::VERSION;
use openai_agents::extensions::{
    CloudflareRealtimeSocket, CloudflareRealtimeTransportLayer, TwilioOutboundMessage,
    TwilioRealtimeTransportAction, TwilioRealtimeTransportLayer,
};
use openai_agents::realtime::RealtimeAudioFormat;

#[derive(Clone, Debug, Default)]
struct FakeCloudflareSocket {
    accepted: bool,
}

impl CloudflareRealtimeSocket for FakeCloudflareSocket {
    fn accept(&mut self) -> openai_agents::Result<()> {
        self.accepted = true;
        Ok(())
    }
}

#[test]
fn cloudflare_transport_builds_upgrade_request_and_accepts_socket() {
    let transport =
        CloudflareRealtimeTransportLayer::new("wss://api.openai.com/v1/realtime?model=foo");
    let expected_sdk_header = format!("openai-agents-sdk.{VERSION}");
    let socket = transport
        .connect_with("ek_test", |request| {
            assert_eq!(request.url, "https://api.openai.com/v1/realtime?model=foo");
            assert_eq!(
                request.headers.get("Authorization").map(String::as_str),
                Some("Bearer ek_test")
            );
            assert_eq!(
                request
                    .headers
                    .get("X-OpenAI-Agents-SDK")
                    .map(String::as_str),
                Some(expected_sdk_header.as_str())
            );
            Ok(FakeCloudflareSocket::default())
        })
        .expect("cloudflare upgrade should succeed");

    assert!(socket.accepted);
}

#[test]
fn twilio_transport_normalizes_audio_and_tracks_output_marks() {
    let mut transport = TwilioRealtimeTransportLayer::new();
    let config = transport.normalize_session_config(None);
    assert_eq!(
        config.input_audio_format,
        Some(RealtimeAudioFormat::G711Ulaw)
    );
    assert_eq!(
        config.output_audio_format,
        Some(RealtimeAudioFormat::G711Ulaw)
    );

    let actions = transport
        .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-1"}}"#, true)
        .expect("start should parse");
    assert_eq!(
        actions,
        vec![TwilioRealtimeTransportAction::StreamStarted {
            stream_sid: "sid-1".to_owned()
        }]
    );

    let media = transport.audio_messages(Some("response"), &[0; 8]);
    assert!(matches!(
        media.first(),
        Some(TwilioOutboundMessage::Media { .. })
    ));
    assert_eq!(
        media.last().map(|message| message.to_json_value()),
        Some(serde_json::json!({
            "event":"mark",
            "streamSid":"sid-1",
            "mark":{"name":"response:1"}
        }))
    );
}

#[test]
fn twilio_transport_resets_stream_state_on_new_start() {
    let mut transport = TwilioRealtimeTransportLayer::new();

    transport
        .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-1"}}"#, true)
        .expect("initial start should parse");
    transport
        .handle_incoming_message(r#"{"event":"mark","mark":{"name":"u:4"}}"#, true)
        .expect("mark should parse");

    let first = transport.audio_messages(Some("response"), &[0; 16]);
    assert_eq!(
        first.last().map(|message| message.to_json_value()),
        Some(serde_json::json!({
            "event":"mark",
            "streamSid":"sid-1",
            "mark":{"name":"response:2"}
        }))
    );

    transport
        .handle_incoming_message(r#"{"event":"start","start":{"streamSid":"sid-2"}}"#, true)
        .expect("next start should parse");
    let second = transport.audio_messages(Some("response"), &[0; 8]);
    assert_eq!(
        second.last().map(|message| message.to_json_value()),
        Some(serde_json::json!({
            "event":"mark",
            "streamSid":"sid-2",
            "mark":{"name":"response:1"}
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
