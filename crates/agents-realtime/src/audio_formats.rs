use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeAudioFormat {
    Pcm16,
    G711Ulaw,
    G711Alaw,
    Custom(String),
}

impl Default for RealtimeAudioFormat {
    fn default() -> Self {
        Self::Pcm16
    }
}

pub fn to_realtime_audio_format(value: impl AsRef<str>) -> RealtimeAudioFormat {
    match value.as_ref() {
        "pcm16" | "pcm" | "audio/pcm" => RealtimeAudioFormat::Pcm16,
        "g711_ulaw" | "pcmu" | "audio/pcmu" => RealtimeAudioFormat::G711Ulaw,
        "g711_alaw" | "pcma" | "audio/pcma" => RealtimeAudioFormat::G711Alaw,
        other => RealtimeAudioFormat::Custom(other.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_known_and_custom_formats() {
        assert_eq!(
            to_realtime_audio_format("pcm16"),
            RealtimeAudioFormat::Pcm16
        );
        assert_eq!(
            to_realtime_audio_format("g711_ulaw"),
            RealtimeAudioFormat::G711Ulaw
        );
        assert_eq!(
            to_realtime_audio_format("g711_alaw"),
            RealtimeAudioFormat::G711Alaw
        );
        assert_eq!(
            to_realtime_audio_format("opus"),
            RealtimeAudioFormat::Custom("opus".to_owned())
        );
    }

    #[test]
    fn converts_legacy_and_mime_aliases() {
        assert_eq!(to_realtime_audio_format("pcm"), RealtimeAudioFormat::Pcm16);
        assert_eq!(
            to_realtime_audio_format("audio/pcm"),
            RealtimeAudioFormat::Pcm16
        );
        assert_eq!(
            to_realtime_audio_format("pcmu"),
            RealtimeAudioFormat::G711Ulaw
        );
        assert_eq!(
            to_realtime_audio_format("audio/pcmu"),
            RealtimeAudioFormat::G711Ulaw
        );
        assert_eq!(
            to_realtime_audio_format("pcma"),
            RealtimeAudioFormat::G711Alaw
        );
        assert_eq!(
            to_realtime_audio_format("audio/pcma"),
            RealtimeAudioFormat::G711Alaw
        );
    }
}
