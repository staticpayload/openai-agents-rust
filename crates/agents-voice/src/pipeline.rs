use std::sync::Arc;

use agents_core::Result;
use futures::StreamExt;

use crate::events::{VoiceStreamEvent, VoiceStreamEventError, VoiceStreamEventLifecycle};
use crate::input::{AudioInput, StreamedAudioInput};
use crate::model::{STTModelSettings, TTSModel, TTSModelSettings, VoiceModelProvider};
use crate::openai_model_provider::OpenAIVoiceModelProvider;
use crate::pipeline_config::VoicePipelineConfig;
use crate::result::{StreamedAudioResult, VoiceStreamRecorder};
use crate::workflow::VoiceWorkflowBase;

#[derive(Clone)]
pub struct VoicePipeline {
    config: VoicePipelineConfig,
    model_provider: Arc<dyn VoiceModelProvider>,
}

impl Default for VoicePipeline {
    fn default() -> Self {
        Self::new(VoicePipelineConfig::default())
    }
}

impl VoicePipeline {
    pub fn new(config: VoicePipelineConfig) -> Self {
        Self {
            config,
            model_provider: Arc::new(OpenAIVoiceModelProvider::default()),
        }
    }

    pub fn with_model_provider(mut self, model_provider: Arc<dyn VoiceModelProvider>) -> Self {
        self.model_provider = model_provider;
        self
    }

    pub async fn run<W: VoiceWorkflowBase + Clone + 'static>(
        &self,
        workflow: &W,
        input: AudioInput,
    ) -> Result<StreamedAudioResult> {
        let stt_model = self.model_provider.stt_model();
        let tts_model = self.model_provider.tts_model();
        let transcription = stt_model
            .transcribe(&input, &STTModelSettings::default())
            .await?;
        self.run_transcription(workflow, transcription, tts_model)
            .await
    }

    pub async fn run_streamed_audio_input<W: VoiceWorkflowBase + Clone + 'static>(
        &self,
        workflow: &W,
        input: StreamedAudioInput,
    ) -> Result<StreamedAudioResult> {
        let stt_model = self.model_provider.stt_model();
        let tts_model = self.model_provider.tts_model();
        let mut session = stt_model
            .start_session(&STTModelSettings::default())
            .await?;
        for chunk in input.chunks {
            session.push_audio(&chunk).await?;
        }
        let transcription = session.finish().await?;
        self.run_transcription(workflow, transcription, tts_model)
            .await
    }

    async fn run_transcription<W: VoiceWorkflowBase + Clone + 'static>(
        &self,
        workflow: &W,
        transcription: String,
        tts_model: Box<dyn TTSModel>,
    ) -> Result<StreamedAudioResult> {
        let recorder = VoiceStreamRecorder::new(self.config.stream_audio);
        let result = recorder.result();
        let workflow = workflow.clone();

        tokio::spawn(async move {
            recorder
                .push_events(vec![VoiceStreamEvent::Lifecycle(
                    VoiceStreamEventLifecycle {
                        event: "started".to_owned(),
                    },
                )])
                .await;

            let completion = async {
                let mut intro = Box::pin(workflow.on_start());
                while let Some(chunk) = intro.next().await {
                    synthesize_chunk(&recorder, tts_model.as_ref(), chunk?).await?;
                }

                let mut text_stream = Box::pin(workflow.run(transcription));
                while let Some(chunk) = text_stream.next().await {
                    synthesize_chunk(&recorder, tts_model.as_ref(), chunk?).await?;
                }

                Result::<()>::Ok(())
            }
            .await;

            match completion {
                Ok(()) => {
                    recorder
                        .push_events(vec![VoiceStreamEvent::Lifecycle(
                            VoiceStreamEventLifecycle {
                                event: "completed".to_owned(),
                            },
                        )])
                        .await;
                    recorder.complete().await;
                }
                Err(error) => {
                    recorder
                        .push_events(vec![VoiceStreamEvent::Error(VoiceStreamEventError {
                            error: error.to_string(),
                        })])
                        .await;
                    recorder.fail(error).await;
                }
            }
        });

        Ok(result)
    }
}

async fn synthesize_chunk(
    recorder: &VoiceStreamRecorder,
    tts_model: &dyn TTSModel,
    text: String,
) -> Result<()> {
    recorder.push_transcript(text.clone()).await;
    let synthesized = tts_model
        .synthesize(&text, &TTSModelSettings::default())
        .await?;
    recorder.push_events(synthesized).await;
    Ok(())
}
