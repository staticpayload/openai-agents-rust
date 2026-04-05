use agents_core::Result;

use crate::agent::RealtimeAgent;
use crate::config::RealtimeRunConfig;
use crate::events::RealtimeEvent;
use crate::model::RealtimeModelConfig;
use crate::openai_realtime::{OpenAIRealtimeWebSocketModel, TransportConfig};
use crate::session::RealtimeSession;

#[derive(Clone, Debug, Default)]
pub struct RealtimeRunner {
    agent: RealtimeAgent,
    config: RealtimeRunConfig,
}

impl RealtimeRunner {
    pub fn new(agent: RealtimeAgent) -> Self {
        Self {
            agent,
            config: RealtimeRunConfig::default(),
        }
    }

    pub fn with_config(mut self, config: RealtimeRunConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn run(&self) -> Result<RealtimeSession> {
        let mut effective_agent = self.agent.clone();
        if self.config.model_settings.is_some() {
            effective_agent.model_settings = self.config.model_settings.clone();
        }
        let model_name = self
            .config
            .model_settings
            .as_ref()
            .and_then(|settings| settings.model_name.clone())
            .or_else(|| {
                self.agent
                    .model_settings
                    .as_ref()
                    .and_then(|settings| settings.model_name.clone())
            });
        let session = RealtimeSession::new(model_name.clone());
        session
            .attach_model_driver(Box::new(OpenAIRealtimeWebSocketModel {
                config: RealtimeModelConfig { model: model_name },
                transport: TransportConfig::default(),
                connected: false,
                last_session_payload: None,
            }))
            .await;
        session.connect(Some(effective_agent.clone())).await?;
        if effective_agent.model_settings.is_some() {
            session.update_agent(effective_agent).await?;
        }
        Ok(session)
    }

    pub async fn run_text_turn(
        &self,
        session: &RealtimeSession,
        text: &str,
    ) -> Result<RealtimeEvent> {
        let mut events = session.send_text(text).await?;
        Ok(events
            .iter()
            .find(|event| matches!(event, RealtimeEvent::TranscriptDelta(_)))
            .cloned()
            .or_else(|| events.pop())
            .unwrap_or_else(|| {
                RealtimeEvent::TranscriptDelta(crate::events::RealtimeTranscriptDeltaEvent {
                    text: text.to_owned(),
                })
            }))
    }
}
