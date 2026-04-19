use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RealtimeAudioFormat;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeClientMessage {
    pub kind: String,
    #[serde(default)]
    pub other_data: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeInputAudioTranscriptionConfig {
    pub language: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
}

impl RealtimeInputAudioTranscriptionConfig {
    pub fn merge(&self, update: &Self) -> Self {
        Self {
            language: update.language.clone().or_else(|| self.language.clone()),
            model: update.model.clone().or_else(|| self.model.clone()),
            prompt: update.prompt.clone().or_else(|| self.prompt.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeInputAudioNoiseReductionConfig {
    pub kind: Option<String>,
}

impl RealtimeInputAudioNoiseReductionConfig {
    pub fn merge(&self, update: &Self) -> Self {
        Self {
            kind: update.kind.clone().or_else(|| self.kind.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeTurnDetectionConfig {
    pub kind: Option<String>,
    pub create_response: Option<bool>,
    pub eagerness: Option<String>,
    pub interrupt_response: Option<bool>,
    pub prefix_padding_ms: Option<u64>,
    pub silence_duration_ms: Option<u64>,
    pub threshold: Option<f32>,
    pub idle_timeout_ms: Option<u64>,
    pub model_version: Option<String>,
}

impl RealtimeTurnDetectionConfig {
    pub fn merge(&self, update: &Self) -> Self {
        Self {
            kind: update.kind.clone().or_else(|| self.kind.clone()),
            create_response: update.create_response.or(self.create_response),
            eagerness: update.eagerness.clone().or_else(|| self.eagerness.clone()),
            interrupt_response: update.interrupt_response.or(self.interrupt_response),
            prefix_padding_ms: update.prefix_padding_ms.or(self.prefix_padding_ms),
            silence_duration_ms: update.silence_duration_ms.or(self.silence_duration_ms),
            threshold: update.threshold.or(self.threshold),
            idle_timeout_ms: update.idle_timeout_ms.or(self.idle_timeout_ms),
            model_version: update
                .model_version
                .clone()
                .or_else(|| self.model_version.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeAudioInputConfig {
    pub format: Option<RealtimeAudioFormat>,
    pub noise_reduction: Option<RealtimeInputAudioNoiseReductionConfig>,
    pub transcription: Option<RealtimeInputAudioTranscriptionConfig>,
    pub turn_detection: Option<RealtimeTurnDetectionConfig>,
    #[serde(skip)]
    pub clear_format: bool,
    #[serde(skip)]
    pub clear_noise_reduction: bool,
    #[serde(skip)]
    pub clear_transcription: bool,
    #[serde(skip)]
    pub clear_turn_detection: bool,
}

impl RealtimeAudioInputConfig {
    pub fn cleared_format(mut self) -> Self {
        self.format = None;
        self.clear_format = true;
        self
    }

    pub fn cleared_noise_reduction(mut self) -> Self {
        self.noise_reduction = None;
        self.clear_noise_reduction = true;
        self
    }

    pub fn cleared_transcription(mut self) -> Self {
        self.transcription = None;
        self.clear_transcription = true;
        self
    }

    pub fn cleared_turn_detection(mut self) -> Self {
        self.turn_detection = None;
        self.clear_turn_detection = true;
        self
    }

    pub fn merge(&self, update: &Self) -> Self {
        Self {
            format: if update.clear_format {
                None
            } else {
                update.format.clone().or_else(|| self.format.clone())
            },
            noise_reduction: match (&self.noise_reduction, &update.noise_reduction) {
                (_, _) if update.clear_noise_reduction => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            transcription: match (&self.transcription, &update.transcription) {
                (_, _) if update.clear_transcription => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            turn_detection: match (&self.turn_detection, &update.turn_detection) {
                (_, _) if update.clear_turn_detection => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            clear_format: update.clear_format,
            clear_noise_reduction: update.clear_noise_reduction,
            clear_transcription: update.clear_transcription,
            clear_turn_detection: update.clear_turn_detection,
        }
    }

    pub(crate) fn has_values_or_clears(&self) -> bool {
        self.clear_format
            || self.clear_noise_reduction
            || self.clear_transcription
            || self.clear_turn_detection
            || self.format.is_some()
            || self.noise_reduction.is_some()
            || self.transcription.is_some()
            || self.turn_detection.is_some()
    }

    pub(crate) fn without_clear_markers(&self) -> Option<Self> {
        let mut cleaned = self.clone();
        cleaned.clear_format = false;
        cleaned.clear_noise_reduction = false;
        cleaned.clear_transcription = false;
        cleaned.clear_turn_detection = false;
        (cleaned.format.is_some()
            || cleaned.noise_reduction.is_some()
            || cleaned.transcription.is_some()
            || cleaned.turn_detection.is_some())
        .then_some(cleaned)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeAudioOutputConfig {
    pub format: Option<RealtimeAudioFormat>,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    #[serde(skip)]
    pub clear_format: bool,
    #[serde(skip)]
    pub clear_voice: bool,
    #[serde(skip)]
    pub clear_speed: bool,
}

impl RealtimeAudioOutputConfig {
    pub fn cleared_format(mut self) -> Self {
        self.format = None;
        self.clear_format = true;
        self
    }

    pub fn cleared_voice(mut self) -> Self {
        self.voice = None;
        self.clear_voice = true;
        self
    }

    pub fn cleared_speed(mut self) -> Self {
        self.speed = None;
        self.clear_speed = true;
        self
    }

    pub fn merge(&self, update: &Self) -> Self {
        Self {
            format: if update.clear_format {
                None
            } else {
                update.format.clone().or_else(|| self.format.clone())
            },
            voice: if update.clear_voice {
                None
            } else {
                update.voice.clone().or_else(|| self.voice.clone())
            },
            speed: if update.clear_speed {
                None
            } else {
                update.speed.or(self.speed)
            },
            clear_format: update.clear_format,
            clear_voice: update.clear_voice,
            clear_speed: update.clear_speed,
        }
    }

    pub(crate) fn has_values_or_clears(&self) -> bool {
        self.clear_format
            || self.clear_voice
            || self.clear_speed
            || self.format.is_some()
            || self.voice.is_some()
            || self.speed.is_some()
    }

    pub(crate) fn without_clear_markers(&self) -> Option<Self> {
        let mut cleaned = self.clone();
        cleaned.clear_format = false;
        cleaned.clear_voice = false;
        cleaned.clear_speed = false;
        (cleaned.format.is_some() || cleaned.voice.is_some() || cleaned.speed.is_some())
            .then_some(cleaned)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeAudioConfig {
    pub input: Option<RealtimeAudioInputConfig>,
    pub output: Option<RealtimeAudioOutputConfig>,
}

impl RealtimeAudioConfig {
    pub fn merge(&self, update: &Self) -> Self {
        Self {
            input: match (&self.input, &update.input) {
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            output: match (&self.output, &update.output) {
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
        }
    }

    pub(crate) fn without_clear_markers(&self) -> Option<Self> {
        let input = self
            .input
            .as_ref()
            .and_then(RealtimeAudioInputConfig::without_clear_markers);
        let output = self
            .output
            .as_ref()
            .and_then(RealtimeAudioOutputConfig::without_clear_markers);
        (input.is_some() || output.is_some()).then_some(Self { input, output })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeModelTracingConfig {
    pub workflow_name: Option<String>,
    pub group_id: Option<String>,
    pub metadata: Option<serde_json::Map<String, Value>>,
}

impl RealtimeModelTracingConfig {
    pub fn merge(&self, update: &Self) -> Self {
        Self {
            workflow_name: update
                .workflow_name
                .clone()
                .or_else(|| self.workflow_name.clone()),
            group_id: update.group_id.clone().or_else(|| self.group_id.clone()),
            metadata: update.metadata.clone().or_else(|| self.metadata.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeSessionModelSettings {
    pub model_name: Option<String>,
    pub instructions: Option<String>,
    pub modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub audio: Option<RealtimeAudioConfig>,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub input_audio_format: Option<RealtimeAudioFormat>,
    pub output_audio_format: Option<RealtimeAudioFormat>,
    pub input_audio_transcription: Option<RealtimeInputAudioTranscriptionConfig>,
    pub input_audio_noise_reduction: Option<RealtimeInputAudioNoiseReductionConfig>,
    pub turn_detection: Option<RealtimeTurnDetectionConfig>,
    pub tool_choice: Option<String>,
    pub tracing: Option<RealtimeModelTracingConfig>,
    #[serde(skip)]
    pub clear_voice: bool,
    #[serde(skip)]
    pub clear_speed: bool,
    #[serde(skip)]
    pub clear_input_audio_format: bool,
    #[serde(skip)]
    pub clear_output_audio_format: bool,
    #[serde(skip)]
    pub clear_input_audio_transcription: bool,
    #[serde(skip)]
    pub clear_input_audio_noise_reduction: bool,
    #[serde(skip)]
    pub clear_turn_detection: bool,
}

impl RealtimeSessionModelSettings {
    pub fn cleared_voice(mut self) -> Self {
        self.voice = None;
        self.clear_voice = true;
        self
    }

    pub fn cleared_speed(mut self) -> Self {
        self.speed = None;
        self.clear_speed = true;
        self
    }

    pub fn cleared_input_audio_format(mut self) -> Self {
        self.input_audio_format = None;
        self.clear_input_audio_format = true;
        self
    }

    pub fn cleared_output_audio_format(mut self) -> Self {
        self.output_audio_format = None;
        self.clear_output_audio_format = true;
        self
    }

    pub fn cleared_input_audio_transcription(mut self) -> Self {
        self.input_audio_transcription = None;
        self.clear_input_audio_transcription = true;
        self
    }

    pub fn cleared_input_audio_noise_reduction(mut self) -> Self {
        self.input_audio_noise_reduction = None;
        self.clear_input_audio_noise_reduction = true;
        self
    }

    pub fn cleared_turn_detection(mut self) -> Self {
        self.turn_detection = None;
        self.clear_turn_detection = true;
        self
    }

    pub fn merge(&self, update: &Self) -> Self {
        let mut merged = Self {
            model_name: update
                .model_name
                .clone()
                .or_else(|| self.model_name.clone()),
            instructions: update
                .instructions
                .clone()
                .or_else(|| self.instructions.clone()),
            modalities: update
                .modalities
                .clone()
                .or_else(|| self.modalities.clone()),
            output_modalities: update
                .output_modalities
                .clone()
                .or_else(|| self.output_modalities.clone()),
            audio: match (&self.audio, &update.audio) {
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            voice: if update.clear_voice {
                None
            } else {
                update.voice.clone().or_else(|| self.voice.clone())
            },
            speed: if update.clear_speed {
                None
            } else {
                update.speed.or(self.speed)
            },
            input_audio_format: if update.clear_input_audio_format {
                None
            } else {
                update
                    .input_audio_format
                    .clone()
                    .or_else(|| self.input_audio_format.clone())
            },
            output_audio_format: if update.clear_output_audio_format {
                None
            } else {
                update
                    .output_audio_format
                    .clone()
                    .or_else(|| self.output_audio_format.clone())
            },
            input_audio_transcription: match (
                &self.input_audio_transcription,
                &update.input_audio_transcription,
            ) {
                (_, _) if update.clear_input_audio_transcription => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            input_audio_noise_reduction: match (
                &self.input_audio_noise_reduction,
                &update.input_audio_noise_reduction,
            ) {
                (_, _) if update.clear_input_audio_noise_reduction => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            turn_detection: match (&self.turn_detection, &update.turn_detection) {
                (_, _) if update.clear_turn_detection => None,
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            tool_choice: update
                .tool_choice
                .clone()
                .or_else(|| self.tool_choice.clone()),
            tracing: match (&self.tracing, &update.tracing) {
                (Some(current), Some(next)) => Some(current.merge(next)),
                (None, Some(next)) => Some(next.clone()),
                (Some(current), None) => Some(current.clone()),
                (None, None) => None,
            },
            clear_voice: update.clear_voice,
            clear_speed: update.clear_speed,
            clear_input_audio_format: update.clear_input_audio_format,
            clear_output_audio_format: update.clear_output_audio_format,
            clear_input_audio_transcription: update.clear_input_audio_transcription,
            clear_input_audio_noise_reduction: update.clear_input_audio_noise_reduction,
            clear_turn_detection: update.clear_turn_detection,
        };

        if update.clear_output_audio_format
            || update.output_audio_format.is_some()
            || update.clear_voice
            || update.voice.is_some()
            || update.clear_speed
            || update.speed.is_some()
        {
            let audio = merged
                .audio
                .get_or_insert_with(RealtimeAudioConfig::default);
            let output = audio
                .output
                .get_or_insert_with(RealtimeAudioOutputConfig::default);

            if update.clear_output_audio_format {
                output.format = None;
                output.clear_format = true;
            } else if let Some(format) = update.output_audio_format.clone() {
                output.format = Some(format);
                output.clear_format = false;
            }

            if update.clear_voice {
                output.voice = None;
                output.clear_voice = true;
            } else if let Some(voice) = update.voice.clone() {
                output.voice = Some(voice);
                output.clear_voice = false;
            }

            if update.clear_speed {
                output.speed = None;
                output.clear_speed = true;
            } else if let Some(speed) = update.speed {
                output.speed = Some(speed);
                output.clear_speed = false;
            }
        }

        merged
    }

    pub fn normalize_effective(&self) -> Self {
        let mut normalized = self.clone();
        let mut audio = normalized.audio.clone().unwrap_or_default();
        let mut input = audio.input.clone().unwrap_or_default();
        let mut output = audio.output.clone().unwrap_or_default();

        if normalized.clear_input_audio_format || input.clear_format {
            normalized.input_audio_format = None;
            input.format = None;
            normalized.clear_input_audio_format = true;
            input.clear_format = true;
        } else {
            let effective = input
                .format
                .clone()
                .or_else(|| normalized.input_audio_format.clone());
            input.format = effective.clone();
            normalized.input_audio_format = effective;
        }

        if normalized.clear_input_audio_transcription || input.clear_transcription {
            normalized.input_audio_transcription = None;
            input.transcription = None;
            normalized.clear_input_audio_transcription = true;
            input.clear_transcription = true;
        } else {
            let effective = input
                .transcription
                .clone()
                .or_else(|| normalized.input_audio_transcription.clone());
            input.transcription = effective.clone();
            normalized.input_audio_transcription = effective;
        }

        if normalized.clear_input_audio_noise_reduction || input.clear_noise_reduction {
            normalized.input_audio_noise_reduction = None;
            input.noise_reduction = None;
            normalized.clear_input_audio_noise_reduction = true;
            input.clear_noise_reduction = true;
        } else {
            let effective = input
                .noise_reduction
                .clone()
                .or_else(|| normalized.input_audio_noise_reduction.clone());
            input.noise_reduction = effective.clone();
            normalized.input_audio_noise_reduction = effective;
        }

        if normalized.clear_turn_detection || input.clear_turn_detection {
            normalized.turn_detection = None;
            input.turn_detection = None;
            normalized.clear_turn_detection = true;
            input.clear_turn_detection = true;
        } else {
            let effective = input
                .turn_detection
                .clone()
                .or_else(|| normalized.turn_detection.clone());
            input.turn_detection = effective.clone();
            normalized.turn_detection = effective;
        }

        if normalized.clear_output_audio_format || output.clear_format {
            normalized.output_audio_format = None;
            output.format = None;
            normalized.clear_output_audio_format = true;
            output.clear_format = true;
        } else {
            let effective = output
                .format
                .clone()
                .or_else(|| normalized.output_audio_format.clone());
            output.format = effective.clone();
            normalized.output_audio_format = effective;
        }

        if normalized.clear_voice || output.clear_voice {
            normalized.voice = None;
            output.voice = None;
            normalized.clear_voice = true;
            output.clear_voice = true;
        } else {
            let effective = output.voice.clone().or_else(|| normalized.voice.clone());
            output.voice = effective.clone();
            normalized.voice = effective;
        }

        if normalized.clear_speed || output.clear_speed {
            normalized.speed = None;
            output.speed = None;
            normalized.clear_speed = true;
            output.clear_speed = true;
        } else {
            let effective = output.speed.or(normalized.speed);
            output.speed = effective;
            normalized.speed = effective;
        }

        audio.input = input.has_values_or_clears().then_some(input);
        audio.output = output.has_values_or_clears().then_some(output);
        normalized.audio = (audio.input.is_some() || audio.output.is_some()).then_some(audio);

        normalized
    }

    pub fn without_clear_markers(&self) -> Self {
        let mut cleaned = self.clone();
        cleaned.clear_voice = false;
        cleaned.clear_speed = false;
        cleaned.clear_input_audio_format = false;
        cleaned.clear_output_audio_format = false;
        cleaned.clear_input_audio_transcription = false;
        cleaned.clear_input_audio_noise_reduction = false;
        cleaned.clear_turn_detection = false;
        cleaned.audio = cleaned
            .audio
            .as_ref()
            .and_then(RealtimeAudioConfig::without_clear_markers);
        cleaned
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeGuardrailsSettings {
    pub debounce_text_length: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeRunConfig {
    pub model_settings: Option<RealtimeSessionModelSettings>,
    pub guardrails_settings: Option<RealtimeGuardrailsSettings>,
    pub tracing_disabled: Option<bool>,
    pub async_tool_calls: Option<bool>,
}
