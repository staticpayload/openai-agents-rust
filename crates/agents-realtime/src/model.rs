use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use agents_core::Result;

use crate::config::RealtimeSessionModelSettings;
use crate::model_events::RealtimeModelEvent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimePlaybackState {
    pub playing: bool,
    pub buffered_audio_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimePlaybackTracker {
    pub state: RealtimePlaybackState,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeModelConfig {
    pub model: Option<String>,
}

#[async_trait]
pub trait RealtimeModelListener: Send + Sync {
    async fn on_event(&self, _event: &RealtimeModelEvent) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
pub trait RealtimeModel: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send_text(&mut self, text: &str) -> Result<Vec<RealtimeModelEvent>>;

    async fn send_audio(&mut self, _bytes: &[u8]) -> Result<Vec<RealtimeModelEvent>> {
        Ok(Vec::new())
    }

    async fn interrupt(&mut self) -> Result<Vec<RealtimeModelEvent>> {
        Ok(Vec::new())
    }

    async fn update_session(
        &mut self,
        _settings: &RealtimeSessionModelSettings,
    ) -> Result<Vec<RealtimeModelEvent>> {
        Ok(Vec::new())
    }
}
