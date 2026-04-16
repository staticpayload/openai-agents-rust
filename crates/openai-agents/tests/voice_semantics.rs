use futures::StreamExt;
use openai_agents::Agent;
use openai_agents::voice::{
    AudioInput, SingleAgentVoiceWorkflow, StreamedAudioInput, VoicePipeline, VoicePipelineConfig,
    VoiceStreamEvent,
};

fn first_audio_text(events: &[VoiceStreamEvent]) -> String {
    let samples = events
        .iter()
        .find_map(|event| match event {
            VoiceStreamEvent::Audio(audio) => audio.data.clone(),
            _ => None,
        })
        .expect("voice pipeline should emit audio");

    samples
        .into_iter()
        .map(|sample| sample as u8)
        .map(char::from)
        .collect()
}

#[tokio::test]
async fn voice_pipeline_returns_live_streamed_audio_result() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: true,
        ..VoicePipelineConfig::default()
    });

    let result = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1, 2, 3],
            },
        )
        .await
        .expect("pipeline should start");

    let events = result.stream_events().collect::<Vec<_>>().await;
    let completed = result
        .wait_for_completion()
        .await
        .expect("pipeline should complete");

    assert_eq!(
        completed.transcript,
        vec!["transcribed:audio/wav:3".to_owned()]
    );
    assert!(completed.audio_chunks >= 1);
    assert!(events.iter().any(
        |event| matches!(event, VoiceStreamEvent::Lifecycle(data) if data.event == "session_started")
    ));
    assert!(events.iter().any(
        |event| matches!(event, VoiceStreamEvent::Transcript(delta) if delta.text == "transcribed:audio/wav:3")
    ));
    assert!(events.iter().any(
        |event| matches!(event, VoiceStreamEvent::Lifecycle(data) if data.event == "session_ended")
    ));
}

#[tokio::test]
async fn voice_pipeline_supports_streamed_audio_input() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: false,
        ..VoicePipelineConfig::default()
    });

    let result = pipeline
        .run_streamed_audio_input(
            &workflow,
            StreamedAudioInput {
                mime_type: "audio/wav".to_owned(),
                chunks: vec![vec![1, 2], vec![3]],
            },
        )
        .await
        .expect("streamed pipeline should start");
    let completed = result
        .wait_for_completion()
        .await
        .expect("streamed pipeline should complete");

    assert_eq!(completed.transcript, vec!["[2][1]".to_owned()]);
    assert_eq!(completed.audio_chunks, 0);
    assert!(completed.events.iter().any(
        |event| matches!(event, VoiceStreamEvent::Transcript(delta) if delta.text == "[2][1]")
    ));
    assert!(completed.events.iter().any(
        |event| matches!(event, VoiceStreamEvent::Lifecycle(data) if data.event == "session_ended")
    ));
}

#[tokio::test]
async fn voice_pipeline_suppresses_audio_for_buffered_input_when_streaming_disabled() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: false,
        ..VoicePipelineConfig::default()
    });

    let result = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![9, 8, 7],
            },
        )
        .await
        .expect("buffered pipeline should start");
    let completed = result
        .wait_for_completion()
        .await
        .expect("buffered pipeline should complete");

    assert_eq!(
        completed.transcript,
        vec!["transcribed:audio/wav:3".to_owned()]
    );
    assert_eq!(completed.audio_chunks, 0);
    assert!(
        completed
            .events
            .iter()
            .all(|event| !matches!(event, VoiceStreamEvent::Audio(_)))
    );
}

#[tokio::test]
async fn single_agent_voice_workflow_retains_state_across_pipeline_turns() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: false,
        ..VoicePipelineConfig::default()
    });

    let first = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1],
            },
        )
        .await
        .expect("first turn should start")
        .wait_for_completion()
        .await
        .expect("first turn should complete");
    let second = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1, 2],
            },
        )
        .await
        .expect("second turn should start")
        .wait_for_completion()
        .await
        .expect("second turn should complete");

    assert_eq!(first.transcript, vec!["transcribed:audio/wav:1".to_owned()]);
    assert_eq!(
        second.transcript,
        vec!["transcribed:audio/wav:2".to_owned()]
    );
}

#[tokio::test]
async fn voice_pipeline_forwards_configured_stt_settings_to_runtime_models() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: false,
        stt_settings: openai_agents::voice::STTModelSettings {
            model: Some("whisper-1".to_owned()),
            language: Some("en".to_owned()),
            prompt: Some("be precise".to_owned()),
        },
        ..VoicePipelineConfig::default()
    });

    let completed = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1, 2, 3],
            },
        )
        .await
        .expect("pipeline should start")
        .wait_for_completion()
        .await
        .expect("pipeline should complete");

    assert_eq!(
        completed.transcript,
        vec!["transcribed:audio/wav:3".to_owned()]
    );
    assert!(!completed.transcript[0].contains("whisper-1"));
    assert!(!completed.transcript[0].contains("be precise"));
}

#[tokio::test]
async fn voice_pipeline_forwards_configured_tts_settings_to_runtime_models() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: true,
        tts_settings: openai_agents::voice::TTSModelSettings {
            model: Some("gpt-4o-mini-tts".to_owned()),
            voice: Some("fable".to_owned()),
            speed: Some(1.25),
        },
        ..VoicePipelineConfig::default()
    });

    let completed = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1, 2, 3],
            },
        )
        .await
        .expect("pipeline should start")
        .wait_for_completion()
        .await
        .expect("pipeline should complete");

    assert_eq!(completed.audio_chunks, 1);
    assert_eq!(
        first_audio_text(&completed.events),
        "transcribed:audio/wav:3|model=gpt-4o-mini-tts|voice=fable|speed=1.25"
    );
}

#[tokio::test]
async fn voice_pipeline_forwards_configured_stt_and_tts_settings() {
    let workflow = SingleAgentVoiceWorkflow::new(Agent::builder("assistant").build());
    let pipeline = VoicePipeline::new(VoicePipelineConfig {
        stream_audio: true,
        stt_settings: openai_agents::voice::STTModelSettings {
            model: Some("whisper-1".to_owned()),
            language: Some("en".to_owned()),
            prompt: Some("be precise".to_owned()),
        },
        tts_settings: openai_agents::voice::TTSModelSettings {
            model: Some("gpt-4o-mini-tts".to_owned()),
            voice: Some("fable".to_owned()),
            speed: Some(1.25),
        },
        ..VoicePipelineConfig::default()
    });

    let completed = pipeline
        .run(
            &workflow,
            AudioInput {
                mime_type: "audio/wav".to_owned(),
                bytes: vec![1, 2, 3],
            },
        )
        .await
        .expect("pipeline should start")
        .wait_for_completion()
        .await
        .expect("pipeline should complete");

    assert_eq!(
        completed.transcript,
        vec!["transcribed:audio/wav:3".to_owned()]
    );
    assert!(!completed.transcript[0].contains("whisper-1"));
    assert!(!completed.transcript[0].contains("be precise"));
    assert_eq!(completed.audio_chunks, 1);
    assert_eq!(
        first_audio_text(&completed.events),
        "transcribed:audio/wav:3|model=gpt-4o-mini-tts|voice=fable|speed=1.25"
    );
}
