use futures::StreamExt;
use openai_agents::realtime::{
    OpenAIRealtimeSIPModel, OpenAIRealtimeWebSocketModel, RealtimeAgent, RealtimeAudioConfig,
    RealtimeAudioFormat, RealtimeAudioInputConfig, RealtimeAudioOutputConfig, RealtimeEvent,
    RealtimeModel, RealtimeModelConfig, RealtimeRunConfig, RealtimeRunner,
    RealtimeSessionModelSettings, TransportConfig,
};
use std::collections::BTreeMap;

#[tokio::test]
async fn realtime_session_streams_events_for_live_commands() {
    let runner = RealtimeRunner::new(RealtimeAgent::new("assistant"));
    let session = runner.run().await.expect("session should start");
    let collector = {
        let session = session.clone();
        tokio::spawn(async move { session.stream_events().collect::<Vec<_>>().await })
    };

    session
        .send_text("hello")
        .await
        .expect("text turn should succeed");
    session
        .send_audio(&[1, 2, 3, 4])
        .await
        .expect("audio turn should succeed");
    session
        .interrupt(Some("user_stop".to_owned()))
        .await
        .expect("interrupt should succeed");

    let mut specialist = RealtimeAgent::new("specialist");
    specialist.model_settings = Some(RealtimeSessionModelSettings {
        model_name: Some("gpt-realtime-specialist".to_owned()),
        ..RealtimeSessionModelSettings::default()
    });
    session
        .update_agent(specialist)
        .await
        .expect("agent update should succeed");
    session.close().await.expect("close should succeed");

    let events = collector.await.expect("collector should finish");

    assert_eq!(session.transcript().await, "hello");
    assert!(matches!(events.first(), Some(RealtimeEvent::AgentStart(_))));
    assert!(events.iter().any(
        |event| matches!(event, RealtimeEvent::TranscriptDelta(delta) if delta.text == "hello")
    ));
    assert!(events.iter().any(
        |event| matches!(event, RealtimeEvent::RawModelEvent(raw) if raw.event_type == "audio_done")
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RealtimeEvent::Interrupted(_)))
    );
    assert!(events.iter().any(
        |event| matches!(event, RealtimeEvent::AgentEnd(ended) if ended.info.agent_name.as_deref() == Some("assistant"))
    ));
    assert!(events.iter().any(
        |event| matches!(event, RealtimeEvent::AgentStart(started) if started.info.agent_name.as_deref() == Some("specialist"))
    ));
    assert!(events
        .iter()
        .any(|event| matches!(event, RealtimeEvent::SessionUpdated(updated) if updated.model.as_deref() == Some("gpt-realtime-specialist"))));
    assert_eq!(
        session
            .model_settings()
            .await
            .and_then(|settings| settings.model_name),
        Some("gpt-realtime-specialist".to_owned())
    );
    assert!(!session.playback_state().await.playing);
    assert!(matches!(
        events.last(),
        Some(RealtimeEvent::SessionClosed(_))
    ));
}

#[tokio::test]
async fn realtime_runner_applies_run_config_model_settings() {
    let runner =
        RealtimeRunner::new(RealtimeAgent::new("assistant")).with_config(RealtimeRunConfig {
            model_settings: Some(RealtimeSessionModelSettings {
                model_name: Some("gpt-realtime-configured".to_owned()),
                ..RealtimeSessionModelSettings::default()
            }),
            ..RealtimeRunConfig::default()
        });
    let session = runner.run().await.expect("session should start");

    assert_eq!(
        session
            .model_settings()
            .await
            .and_then(|settings| settings.model_name),
        Some("gpt-realtime-configured".to_owned())
    );
}

#[tokio::test]
async fn realtime_websocket_runtime_state_exposes_connected_transport_and_applied_output_settings()
{
    let mut query_params = BTreeMap::new();
    query_params.insert("model".to_owned(), "ignored-by-custom-query".to_owned());
    query_params.insert("foo".to_owned(), "bar".to_owned());

    let mut model = OpenAIRealtimeWebSocketModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: None,
            query_params,
        },
        ..OpenAIRealtimeWebSocketModel::default()
    };

    model.connect().await.expect("connect should succeed");
    model
        .update_session(&RealtimeSessionModelSettings {
            model_name: Some("gpt-realtime-2".to_owned()),
            audio: Some(RealtimeAudioConfig {
                input: Some(RealtimeAudioInputConfig {
                    format: Some(RealtimeAudioFormat::G711Ulaw),
                    ..RealtimeAudioInputConfig::default()
                }),
                output: Some(RealtimeAudioOutputConfig {
                    voice: Some("marin".to_owned()),
                    speed: Some(1.25),
                    ..RealtimeAudioOutputConfig::default()
                }),
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("session update should succeed");

    let runtime_state = model.runtime_state();
    assert_eq!(
        runtime_state.transport.connection_url.as_deref(),
        Some("wss://example.com/realtime?foo=bar&model=ignored-by-custom-query")
    );
    assert!(runtime_state.transport.api_key_present);
    assert_eq!(
        runtime_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice"))
            .and_then(serde_json::Value::as_str),
        Some("marin")
    );
    assert_eq!(
        runtime_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.25)
    );
}

#[tokio::test]
async fn realtime_sip_runtime_state_exposes_call_attachment_and_applied_output_settings() {
    let mut query_params = BTreeMap::new();
    query_params.insert("foo".to_owned(), "bar".to_owned());

    let mut model = OpenAIRealtimeSIPModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: Some("call_123".to_owned()),
            query_params,
        },
        ..OpenAIRealtimeSIPModel::default()
    };

    model.connect().await.expect("connect should succeed");
    model
        .update_session(&RealtimeSessionModelSettings {
            audio: Some(RealtimeAudioConfig {
                output: Some(RealtimeAudioOutputConfig {
                    voice: Some("verse".to_owned()),
                    speed: Some(1.5),
                    ..RealtimeAudioOutputConfig::default()
                }),
                ..RealtimeAudioConfig::default()
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("session update should succeed");

    let runtime_state = model.runtime_state();
    assert_eq!(
        runtime_state.transport.connection_url.as_deref(),
        Some("wss://example.com/realtime?call_id=call_123&foo=bar")
    );
    assert_eq!(runtime_state.transport.call_id.as_deref(), Some("call_123"));
    assert_eq!(
        runtime_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice"))
            .and_then(serde_json::Value::as_str),
        Some("verse")
    );
    assert_eq!(
        runtime_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.5)
    );
}

#[tokio::test]
async fn realtime_models_expose_connected_transport_state() {
    let mut websocket = OpenAIRealtimeWebSocketModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: None,
            query_params: BTreeMap::from([
                ("foo".to_owned(), "bar".to_owned()),
                ("model".to_owned(), "ignored-by-custom-query".to_owned()),
            ]),
        },
        ..OpenAIRealtimeWebSocketModel::default()
    };
    websocket.connect().await.expect("connect should succeed");
    websocket
        .update_session(&RealtimeSessionModelSettings {
            audio: Some(RealtimeAudioConfig {
                output: Some(RealtimeAudioOutputConfig {
                    voice: Some("marin".to_owned()),
                    speed: Some(1.25),
                    ..RealtimeAudioOutputConfig::default()
                }),
                ..RealtimeAudioConfig::default()
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("websocket session update should succeed");

    let websocket_state = websocket.runtime_state();
    assert_eq!(
        websocket_state.transport.connection_url.as_deref(),
        Some("wss://example.com/realtime?foo=bar&model=ignored-by-custom-query")
    );
    assert!(websocket_state.transport.api_key_present);
    assert_eq!(
        websocket_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice"))
            .and_then(serde_json::Value::as_str),
        Some("marin")
    );
    assert_eq!(
        websocket_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.25)
    );

    let mut sip = OpenAIRealtimeSIPModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: Some("call_123".to_owned()),
            query_params: BTreeMap::from([("foo".to_owned(), "bar".to_owned())]),
        },
        ..OpenAIRealtimeSIPModel::default()
    };
    sip.connect().await.expect("connect should succeed");
    sip.update_session(&RealtimeSessionModelSettings {
        audio: Some(RealtimeAudioConfig {
            output: Some(RealtimeAudioOutputConfig {
                voice: Some("verse".to_owned()),
                speed: Some(1.5),
                ..RealtimeAudioOutputConfig::default()
            }),
            ..RealtimeAudioConfig::default()
        }),
        ..RealtimeSessionModelSettings::default()
    })
    .await
    .expect("sip session update should succeed");

    let sip_state = sip.runtime_state();
    assert_eq!(
        sip_state.transport.connection_url.as_deref(),
        Some("wss://example.com/realtime?call_id=call_123&foo=bar")
    );
    assert_eq!(sip_state.transport.call_id.as_deref(), Some("call_123"));
    assert_eq!(
        sip_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice"))
            .and_then(serde_json::Value::as_str),
        Some("verse")
    );
    assert_eq!(
        sip_state
            .last_session_payload
            .as_ref()
            .and_then(|payload| payload.payload.get("audio"))
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.5)
    );
}

#[tokio::test]
async fn partial_session_updates_preserve_prior_settings() {
    let runner = RealtimeRunner::new(RealtimeAgent::new("assistant"));
    let session = runner.run().await.expect("session should start");

    session
        .update_agent({
            let mut configured = RealtimeAgent::new("assistant");
            configured.model_settings = Some(RealtimeSessionModelSettings {
                model_name: Some("gpt-realtime-1.5".to_owned()),
                audio: Some(RealtimeAudioConfig {
                    input: Some(RealtimeAudioInputConfig {
                        format: Some(RealtimeAudioFormat::G711Ulaw),
                        ..RealtimeAudioInputConfig::default()
                    }),
                    output: Some(RealtimeAudioOutputConfig {
                        voice: Some("alloy".to_owned()),
                        speed: Some(1.25),
                        ..RealtimeAudioOutputConfig::default()
                    }),
                }),
                ..RealtimeSessionModelSettings::default()
            });
            configured
        })
        .await
        .expect("initial update should succeed");

    let updated = session
        .update_agent({
            let mut partial = RealtimeAgent::new("assistant");
            partial.model_settings = Some(RealtimeSessionModelSettings {
                audio: Some(RealtimeAudioConfig {
                    output: Some(RealtimeAudioOutputConfig {
                        voice: Some("verse".to_owned()),
                        ..RealtimeAudioOutputConfig::default()
                    }),
                    ..RealtimeAudioConfig::default()
                }),
                ..RealtimeSessionModelSettings::default()
            });
            partial
        })
        .await
        .expect("partial update should succeed");

    let settings = session
        .model_settings()
        .await
        .expect("session should keep model settings");
    assert_eq!(settings.model_name.as_deref(), Some("gpt-realtime-1.5"));
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.input.as_ref())
            .and_then(|input| input.format.clone()),
        Some(RealtimeAudioFormat::G711Ulaw)
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.voice.as_deref()),
        Some("verse")
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.speed),
        Some(1.25)
    );
    assert!(matches!(
        updated,
        RealtimeEvent::SessionUpdated(event) if event.model.as_deref() == Some("gpt-realtime-1.5")
    ));
    assert_eq!(settings.voice.as_deref(), Some("verse"));
    assert_eq!(settings.speed, Some(1.25));
}

#[tokio::test]
async fn partial_session_updates_can_clear_and_normalize_state() {
    let mut model = OpenAIRealtimeWebSocketModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: None,
            query_params: BTreeMap::new(),
        },
        ..OpenAIRealtimeWebSocketModel::default()
    };
    model.connect().await.expect("connect should succeed");

    model
        .update_session(&RealtimeSessionModelSettings {
            model_name: Some("gpt-realtime-1.5".to_owned()),
            voice: Some("alloy".to_owned()),
            speed: Some(1.25),
            output_audio_format: Some(RealtimeAudioFormat::Pcm16),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("initial model update should succeed");
    model
        .update_session(&RealtimeSessionModelSettings {
            audio: Some(RealtimeAudioConfig {
                output: Some(RealtimeAudioOutputConfig {
                    voice: Some("verse".to_owned()),
                    ..RealtimeAudioOutputConfig::default()
                }),
                ..RealtimeAudioConfig::default()
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("mixed model update should succeed");

    let payload = model
        .runtime_state()
        .last_session_payload
        .expect("mixed update should record a payload")
        .payload;
    assert_eq!(
        payload
            .get("audio")
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice"))
            .and_then(serde_json::Value::as_str),
        Some("verse")
    );
    assert_eq!(
        payload
            .get("audio")
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.25)
    );
    assert_eq!(
        model
            .applied_settings
            .as_ref()
            .and_then(|settings| settings.voice.as_deref()),
        Some("verse")
    );
    assert_eq!(
        model
            .applied_settings
            .as_ref()
            .and_then(|settings| settings.speed),
        Some(1.25)
    );
    assert_eq!(
        model
            .applied_settings
            .as_ref()
            .and_then(|settings| settings.audio.as_ref())
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.voice.as_deref()),
        Some("verse")
    );

    model
        .update_session(&RealtimeSessionModelSettings {
            audio: Some(RealtimeAudioConfig {
                output: Some(RealtimeAudioOutputConfig::default().cleared_voice()),
                ..RealtimeAudioConfig::default()
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("clear update should succeed");

    let cleared_payload = model
        .runtime_state()
        .last_session_payload
        .expect("clear update should record a payload")
        .payload;
    assert_eq!(
        cleared_payload
            .get("audio")
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("voice")),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        cleared_payload
            .get("audio")
            .and_then(|audio| audio.get("output"))
            .and_then(|output| output.get("speed"))
            .and_then(serde_json::Value::as_f64),
        Some(1.25)
    );
    assert_eq!(
        model
            .applied_settings
            .as_ref()
            .and_then(|settings| settings.voice.as_deref()),
        None
    );
    assert_eq!(
        model
            .applied_settings
            .as_ref()
            .and_then(|settings| settings.speed),
        Some(1.25)
    );

    let runner = RealtimeRunner::new(RealtimeAgent::new("assistant"));
    let session = runner.run().await.expect("session should start");
    session
        .update_agent({
            let mut configured = RealtimeAgent::new("assistant");
            configured.model_settings = Some(RealtimeSessionModelSettings {
                model_name: Some("gpt-realtime-1.5".to_owned()),
                voice: Some("alloy".to_owned()),
                speed: Some(1.25),
                output_audio_format: Some(RealtimeAudioFormat::Pcm16),
                ..RealtimeSessionModelSettings::default()
            });
            configured
        })
        .await
        .expect("initial session update should succeed");
    session
        .update_agent({
            let mut mixed = RealtimeAgent::new("assistant");
            mixed.model_settings = Some(RealtimeSessionModelSettings {
                audio: Some(RealtimeAudioConfig {
                    output: Some(RealtimeAudioOutputConfig {
                        voice: Some("verse".to_owned()),
                        ..RealtimeAudioOutputConfig::default()
                    }),
                    ..RealtimeAudioConfig::default()
                }),
                ..RealtimeSessionModelSettings::default()
            });
            mixed
        })
        .await
        .expect("mixed session update should succeed");
    session
        .update_agent({
            let mut cleared = RealtimeAgent::new("assistant");
            cleared.model_settings = Some(RealtimeSessionModelSettings {
                audio: Some(RealtimeAudioConfig {
                    output: Some(RealtimeAudioOutputConfig::default().cleared_voice()),
                    ..RealtimeAudioConfig::default()
                }),
                ..RealtimeSessionModelSettings::default()
            });
            cleared
        })
        .await
        .expect("clear session update should succeed");

    let settings = session
        .model_settings()
        .await
        .expect("session should expose normalized settings");
    assert_eq!(settings.voice.as_deref(), None);
    assert_eq!(settings.speed, Some(1.25));
    assert_eq!(
        settings.output_audio_format,
        Some(RealtimeAudioFormat::Pcm16)
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.voice.as_deref()),
        None
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.speed),
        Some(1.25)
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.format.clone()),
        Some(RealtimeAudioFormat::Pcm16)
    );
}

#[tokio::test]
async fn model_flat_alias_updates_override_stale_normalized_audio_state() {
    let mut model = OpenAIRealtimeWebSocketModel {
        config: RealtimeModelConfig {
            model: Some("gpt-realtime-1.5".to_owned()),
        },
        transport: TransportConfig {
            api_key: Some("sk-test".to_owned()),
            websocket_url: Some("https://example.com/realtime".to_owned()),
            call_id: None,
            query_params: BTreeMap::new(),
        },
        ..OpenAIRealtimeWebSocketModel::default()
    };
    model.connect().await.expect("connect should succeed");

    model
        .update_session(&RealtimeSessionModelSettings {
            audio: Some(RealtimeAudioConfig {
                output: Some(RealtimeAudioOutputConfig {
                    format: Some(RealtimeAudioFormat::Pcm16),
                    voice: Some("alloy".to_owned()),
                    speed: Some(1.0),
                    ..RealtimeAudioOutputConfig::default()
                }),
                ..RealtimeAudioConfig::default()
            }),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("initial nested update should succeed");

    model
        .update_session(&RealtimeSessionModelSettings {
            voice: Some("verse".to_owned()),
            speed: Some(1.5),
            output_audio_format: Some(RealtimeAudioFormat::G711Ulaw),
            ..RealtimeSessionModelSettings::default()
        })
        .await
        .expect("flat alias update should succeed");

    let payload = model
        .runtime_state()
        .last_session_payload
        .expect("flat update should record a payload")
        .payload;
    let output = payload
        .get("audio")
        .and_then(|audio| audio.get("output"))
        .expect("payload should include normalized audio.output");
    assert_eq!(
        output.get("voice").and_then(serde_json::Value::as_str),
        Some("verse")
    );
    assert_eq!(
        output.get("speed").and_then(serde_json::Value::as_f64),
        Some(1.5)
    );
    assert_eq!(
        output
            .get("format")
            .and_then(|format| format.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("audio/pcmu")
    );

    let applied = model
        .applied_settings
        .as_ref()
        .expect("model should persist applied settings");
    assert_eq!(applied.voice.as_deref(), Some("verse"));
    assert_eq!(applied.speed, Some(1.5));
    assert_eq!(
        applied.output_audio_format,
        Some(RealtimeAudioFormat::G711Ulaw)
    );
    assert_eq!(
        applied
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.voice.as_deref()),
        Some("verse")
    );
    assert_eq!(
        applied
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.speed),
        Some(1.5)
    );
    assert_eq!(
        applied
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.format.clone()),
        Some(RealtimeAudioFormat::G711Ulaw)
    );
}

#[tokio::test]
async fn session_flat_alias_updates_override_stale_normalized_audio_state() {
    let runner = RealtimeRunner::new(RealtimeAgent::new("assistant"));
    let session = runner.run().await.expect("session should start");

    session
        .update_agent({
            let mut configured = RealtimeAgent::new("assistant");
            configured.model_settings = Some(RealtimeSessionModelSettings {
                audio: Some(RealtimeAudioConfig {
                    output: Some(RealtimeAudioOutputConfig {
                        format: Some(RealtimeAudioFormat::Pcm16),
                        voice: Some("alloy".to_owned()),
                        speed: Some(1.0),
                        ..RealtimeAudioOutputConfig::default()
                    }),
                    ..RealtimeAudioConfig::default()
                }),
                ..RealtimeSessionModelSettings::default()
            });
            configured
        })
        .await
        .expect("initial nested session update should succeed");

    session
        .update_agent({
            let mut updated = RealtimeAgent::new("assistant");
            updated.model_settings = Some(RealtimeSessionModelSettings {
                voice: Some("verse".to_owned()),
                speed: Some(1.5),
                output_audio_format: Some(RealtimeAudioFormat::G711Ulaw),
                ..RealtimeSessionModelSettings::default()
            });
            updated
        })
        .await
        .expect("flat alias session update should succeed");

    let settings = session
        .model_settings()
        .await
        .expect("session should expose normalized settings");
    assert_eq!(settings.voice.as_deref(), Some("verse"));
    assert_eq!(settings.speed, Some(1.5));
    assert_eq!(
        settings.output_audio_format,
        Some(RealtimeAudioFormat::G711Ulaw)
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.voice.as_deref()),
        Some("verse")
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.speed),
        Some(1.5)
    );
    assert_eq!(
        settings
            .audio
            .as_ref()
            .and_then(|audio| audio.output.as_ref())
            .and_then(|output| output.format.clone()),
        Some(RealtimeAudioFormat::G711Ulaw)
    );
}
