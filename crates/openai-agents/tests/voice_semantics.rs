use futures::StreamExt;
use openai_agents::Agent;
use openai_agents::voice::{
    AudioInput, SingleAgentVoiceWorkflow, StreamedAudioInput, VoicePipeline, VoicePipelineConfig,
    VoiceStreamEvent,
};

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
