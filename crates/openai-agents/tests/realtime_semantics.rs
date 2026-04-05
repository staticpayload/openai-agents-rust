use futures::StreamExt;
use openai_agents::realtime::{
    RealtimeAgent, RealtimeEvent, RealtimeRunConfig, RealtimeRunner, RealtimeSessionModelSettings,
};

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
