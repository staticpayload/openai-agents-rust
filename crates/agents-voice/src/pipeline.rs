use std::sync::Arc;

use agents_core::Result;
use futures::StreamExt;

use crate::events::VoiceStreamEvent;
use crate::input::{AudioInput, StreamedAudioInput};
use crate::model::{STTModelSettings, TTSModel, TTSModelSettings, VoiceModelProvider};
use crate::openai_model_provider::OpenAIVoiceModelProvider;
use crate::pipeline_config::VoicePipelineConfig;
use crate::result::StreamedAudioResult;
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

    pub async fn run<W: VoiceWorkflowBase>(
        &self,
        workflow: &W,
        input: AudioInput,
    ) -> Result<StreamedAudioResult> {
        let stt_model = self.model_provider.stt_model();
        let tts_model = self.model_provider.tts_model();
        let transcription = stt_model
            .transcribe(&input, &STTModelSettings::default())
            .await?;
        self.run_transcription(workflow, transcription, tts_model.as_ref())
            .await
    }

    pub async fn run_streamed_audio_input<W: VoiceWorkflowBase>(
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
        self.run_transcription(workflow, transcription, tts_model.as_ref())
            .await
    }

    async fn run_transcription<W: VoiceWorkflowBase>(
        &self,
        workflow: &W,
        transcription: String,
        tts_model: &dyn TTSModel,
    ) -> Result<StreamedAudioResult> {
        let mut transcript = Vec::new();
        let mut events = Vec::new();
        let mut audio_chunks = 0usize;

        let mut intro = Box::pin(workflow.on_start());
        while let Some(chunk) = intro.next().await {
            let text: String = chunk?;
            transcript.push(text.clone());
            let synthesized = tts_model
                .synthesize(&text, &TTSModelSettings::default())
                .await?;
            audio_chunks += synthesized
                .iter()
                .filter(|event| matches!(event, VoiceStreamEvent::Audio(_)))
                .count();
            events.extend(synthesized);
        }

        let mut text_stream = Box::pin(workflow.run(transcription));
        while let Some(chunk) = text_stream.next().await {
            let text: String = chunk?;
            transcript.push(text.clone());
            let synthesized = tts_model
                .synthesize(&text, &TTSModelSettings::default())
                .await?;
            audio_chunks += synthesized
                .iter()
                .filter(|event| matches!(event, VoiceStreamEvent::Audio(_)))
                .count();
            events.extend(synthesized);
        }

        if !self.config.stream_audio {
            audio_chunks = 0;
        }

        Ok(StreamedAudioResult {
            transcript,
            audio_chunks,
            events,
        })
    }
}
