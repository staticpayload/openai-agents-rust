use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::errors::Result;
use crate::items::InputItem;
use crate::memory::session_settings::{SessionSettings, resolve_session_limit};
use crate::memory::util::apply_session_limit;

/// Arguments for compaction-aware OpenAI responses sessions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpenAIResponsesCompactionArgs {
    pub response_id: Option<String>,
    pub compaction_mode: Option<String>,
    pub store: Option<bool>,
    pub force: Option<bool>,
}

#[async_trait]
pub trait Session: Send + Sync {
    fn session_id(&self) -> &str;

    fn session_settings(&self) -> Option<&SessionSettings> {
        None
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>>;

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()>;

    async fn pop_item(&self) -> Result<Option<InputItem>>;

    async fn clear_session(&self) -> Result<()>;

    async fn get_items(&self) -> Result<Vec<InputItem>> {
        self.get_items_with_limit(None).await
    }

    async fn clear(&self) -> Result<()> {
        self.clear_session().await
    }

    fn compaction_session(&self) -> Option<&dyn OpenAIResponsesCompactionAwareSession> {
        None
    }
}

#[async_trait]
pub trait OpenAIResponsesCompactionAwareSession: Session {
    async fn run_compaction(&self, args: Option<OpenAIResponsesCompactionArgs>) -> Result<()>;
}

pub fn is_openai_responses_compaction_aware_session(session: &dyn Session) -> bool {
    session.compaction_session().is_some()
}

/// In-memory session used by tests and local workflows.
#[derive(Clone, Debug)]
pub struct MemorySession {
    session_id: String,
    session_settings: Option<SessionSettings>,
    items: Arc<Mutex<Vec<InputItem>>>,
}

impl MemorySession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            session_settings: Some(SessionSettings::default()),
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_settings(mut self, settings: SessionSettings) -> Self {
        self.session_settings = Some(settings);
        self
    }
}

#[async_trait]
impl Session for MemorySession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn session_settings(&self) -> Option<&SessionSettings> {
        self.session_settings.as_ref()
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>> {
        let items = self.items.lock().await.clone();
        let resolved_limit = resolve_session_limit(limit, self.session_settings());
        Ok(apply_session_limit(&items, resolved_limit))
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        self.items.lock().await.extend(items);
        Ok(())
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        Ok(self.items.lock().await.pop())
    }

    async fn clear_session(&self) -> Result<()> {
        self.items.lock().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_session_respects_default_limit_setting() {
        let session =
            MemorySession::new("session").with_settings(SessionSettings { limit: Some(2) });
        session
            .add_items(vec![
                InputItem::from("a"),
                InputItem::from("b"),
                InputItem::from("c"),
            ])
            .await
            .expect("items should be added");

        let items = session.get_items().await.expect("items should load");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_text(), Some("b"));
        assert_eq!(items[1].as_text(), Some("c"));
    }
}
