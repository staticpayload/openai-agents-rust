use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceStreamEventAudio {
    pub data: Option<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceStreamEventLifecycle {
    pub event: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceStreamEventTranscript {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceStreamEventError {
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceStreamEvent {
    Audio(VoiceStreamEventAudio),
    Transcript(VoiceStreamEventTranscript),
    Lifecycle(VoiceStreamEventLifecycle),
    Error(VoiceStreamEventError),
}
