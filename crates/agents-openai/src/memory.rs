use std::sync::Arc;

use agents_core::{InputItem, MemorySession, Result, Session};
use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::OpenAIClientOptions;

#[derive(Clone, Debug)]
pub struct OpenAIConversationsSession {
    inner: MemorySession,
    pub client_options: OpenAIClientOptions,
    conversation_id: Arc<Mutex<String>>,
    last_response_id: Arc<Mutex<Option<String>>>,
}

impl OpenAIConversationsSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            inner: MemorySession::new(session_id.clone()),
            client_options: OpenAIClientOptions::default(),
            conversation_id: Arc::new(Mutex::new(session_id)),
            last_response_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_client_options(mut self, client_options: OpenAIClientOptions) -> Self {
        self.client_options = client_options;
        self
    }

    pub async fn set_conversation_id(&self, conversation_id: impl Into<String>) {
        *self.conversation_id.lock().await = conversation_id.into();
    }

    pub async fn conversation_id(&self) -> String {
        self.conversation_id.lock().await.clone()
    }

    pub async fn last_response_id(&self) -> Option<String> {
        self.last_response_id.lock().await.clone()
    }

    pub async fn set_last_response_id(&self, response_id: impl Into<String>) {
        *self.last_response_id.lock().await = Some(response_id.into());
    }
}

#[async_trait]
impl Session for OpenAIConversationsSession {
    fn session_id(&self) -> &str {
        self.inner.session_id()
    }

    async fn get_items(&self) -> Result<Vec<InputItem>> {
        self.inner.get_items().await
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        self.inner.add_items(items).await
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        self.inner.pop_item().await
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear().await?;
        *self.conversation_id.lock().await = format!("conv_{}", Uuid::new_v4());
        *self.last_response_id.lock().await = None;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenAIResponsesCompactionMode {
    PreviousResponseId,
    Input,
    #[default]
    Auto,
}

#[derive(Clone, Debug)]
pub struct OpenAIResponsesCompactionSession {
    inner: MemorySession,
    pub mode: OpenAIResponsesCompactionMode,
    pub client_options: OpenAIClientOptions,
    response_id: Arc<Mutex<Option<String>>>,
    deferred_response_id: Arc<Mutex<Option<String>>>,
    last_unstored_response_id: Arc<Mutex<Option<String>>>,
    compaction_threshold: usize,
}

impl OpenAIResponsesCompactionSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            inner: MemorySession::new(session_id),
            mode: OpenAIResponsesCompactionMode::Auto,
            client_options: OpenAIClientOptions::default(),
            response_id: Arc::new(Mutex::new(None)),
            deferred_response_id: Arc::new(Mutex::new(None)),
            last_unstored_response_id: Arc::new(Mutex::new(None)),
            compaction_threshold: 10,
        }
    }

    pub fn with_mode(mut self, mode: OpenAIResponsesCompactionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_client_options(mut self, client_options: OpenAIClientOptions) -> Self {
        self.client_options = client_options;
        self
    }

    pub fn with_compaction_threshold(mut self, compaction_threshold: usize) -> Self {
        self.compaction_threshold = compaction_threshold;
        self
    }

    pub async fn response_id(&self) -> Option<String> {
        self.response_id.lock().await.clone()
    }

    pub async fn set_response_id(&self, response_id: impl Into<String>) {
        *self.response_id.lock().await = Some(response_id.into());
    }

    pub async fn defer_response_id(&self, response_id: impl Into<String>) {
        *self.deferred_response_id.lock().await = Some(response_id.into());
    }

    pub async fn take_deferred_response_id(&self) -> Option<String> {
        self.deferred_response_id.lock().await.take()
    }

    pub async fn mark_response_unstored(&self, response_id: impl Into<String>) {
        *self.last_unstored_response_id.lock().await = Some(response_id.into());
    }

    pub async fn last_unstored_response_id(&self) -> Option<String> {
        self.last_unstored_response_id.lock().await.clone()
    }

    pub async fn compaction_candidate_items(&self) -> Result<Vec<InputItem>> {
        Ok(self
            .inner
            .get_items()
            .await?
            .into_iter()
            .filter(|item| !is_user_like_item(item) && !is_compaction_marker(item))
            .collect())
    }

    pub async fn compaction_candidate_count(&self) -> Result<usize> {
        Ok(self.compaction_candidate_items().await?.len())
    }

    pub async fn should_compact(&self) -> Result<bool> {
        Ok(self.compaction_candidate_count().await? >= self.compaction_threshold)
    }

    pub async fn compact(&self) -> Result<()> {
        let items = self.inner.get_items().await?;
        let candidate_indices = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (!is_user_like_item(item) && !is_compaction_marker(item)).then_some(index)
            })
            .collect::<Vec<_>>();
        if candidate_indices.len() <= self.compaction_threshold {
            return Ok(());
        }

        let keep_candidate_count = (self.compaction_threshold.max(2) / 2).max(1);
        let kept_candidate_indices = candidate_indices
            .iter()
            .rev()
            .take(keep_candidate_count)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let compacted = items
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if is_user_like_item(&item)
                    || is_compaction_marker(&item)
                    || kept_candidate_indices.contains(&index)
                {
                    Some(item)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.inner.clear().await?;
        self.inner.add_items(compacted).await
    }
}

#[async_trait]
impl Session for OpenAIResponsesCompactionSession {
    fn session_id(&self) -> &str {
        self.inner.session_id()
    }

    async fn get_items(&self) -> Result<Vec<InputItem>> {
        self.inner.get_items().await
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        self.inner.add_items(items).await?;
        if self.should_compact().await?
            && matches!(
                self.mode,
                OpenAIResponsesCompactionMode::Input | OpenAIResponsesCompactionMode::Auto
            )
        {
            self.compact().await?;
        }
        Ok(())
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        self.inner.pop_item().await
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear().await?;
        *self.response_id.lock().await = None;
        *self.deferred_response_id.lock().await = None;
        *self.last_unstored_response_id.lock().await = None;
        Ok(())
    }
}

fn is_user_like_item(item: &InputItem) -> bool {
    match item {
        InputItem::Text { .. } => true,
        InputItem::Json { value } => value
            .get("role")
            .and_then(serde_json::Value::as_str)
            .map(|role| role == "user")
            .unwrap_or(false),
    }
}

fn is_compaction_marker(item: &InputItem) -> bool {
    match item {
        InputItem::Text { .. } => false,
        InputItem::Json { value } => value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(|kind| kind == "compaction")
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compaction_session_trims_history_when_threshold_exceeded() {
        let session = OpenAIResponsesCompactionSession::new("session")
            .with_compaction_threshold(4)
            .with_mode(OpenAIResponsesCompactionMode::Input);

        session
            .add_items(vec![
                InputItem::from("1"),
                InputItem::Json {
                    value: serde_json::json!({"type": "tool_call_output", "call_id": "call-1"}),
                },
                InputItem::Json {
                    value: serde_json::json!({"type": "tool_call_output", "call_id": "call-2"}),
                },
                InputItem::Json {
                    value: serde_json::json!({"type": "tool_call_output", "call_id": "call-3"}),
                },
                InputItem::Json {
                    value: serde_json::json!({"type": "tool_call_output", "call_id": "call-4"}),
                },
                InputItem::Json {
                    value: serde_json::json!({"type": "tool_call_output", "call_id": "call-5"}),
                },
            ])
            .await
            .expect("items should be stored");

        let items = session.get_items().await.expect("items should load");
        assert_eq!(items[0], InputItem::from("1"));
        assert!(items.len() <= 4);
    }

    #[tokio::test]
    async fn conversations_session_rekeys_on_clear() {
        let session = OpenAIConversationsSession::new("conv-1");
        let before = session.conversation_id().await;
        session.clear().await.expect("clear should succeed");
        let after = session.conversation_id().await;

        assert_ne!(before, after);
    }

    #[tokio::test]
    async fn compaction_tracks_response_metadata() {
        let session = OpenAIResponsesCompactionSession::new("session");
        session.set_response_id("resp-1").await;
        session.defer_response_id("resp-2").await;
        session.mark_response_unstored("resp-3").await;

        assert_eq!(session.response_id().await.as_deref(), Some("resp-1"));
        assert_eq!(
            session.take_deferred_response_id().await.as_deref(),
            Some("resp-2")
        );
        assert_eq!(
            session.last_unstored_response_id().await.as_deref(),
            Some("resp-3")
        );
    }
}
