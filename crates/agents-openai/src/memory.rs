use std::sync::Arc;

use agents_core::{
    InputItem, MemorySession, OpenAIConversationAwareSession, OpenAIConversationSessionState,
    OpenAIResponsesCompactionArgs, OpenAIResponsesCompactionAwareSession, Result, Session,
    SessionSettings,
};
use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::OpenAIClientOptions;
use crate::{get_default_openai_key, get_openai_base_url};

const DEFAULT_COMPACTION_THRESHOLD: usize = 10;
const TOOL_CALL_SESSION_DESCRIPTION_KEY: &str = "tool_call_session_description";
const TOOL_CALL_SESSION_TITLE_KEY: &str = "tool_call_session_title";

pub async fn start_openai_conversations_session(
    client_options: Option<OpenAIClientOptions>,
) -> Result<String> {
    let client_options = client_options.unwrap_or_else(|| {
        OpenAIClientOptions::new(get_default_openai_key()).with_base_url(get_openai_base_url())
    });
    let api_key = client_options
        .api_key
        .clone()
        .ok_or(agents_core::AgentsError::ModelProviderNotConfigured)?;
    let response = reqwest::Client::new()
        .post(client_options.api_url("/conversations"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth(api_key)
        .json(&serde_json::json!({ "items": [] }))
        .send()
        .await
        .map_err(|error| agents_core::AgentsError::message(error.to_string()))?;
    let response = response
        .error_for_status()
        .map_err(|error| agents_core::AgentsError::message(error.to_string()))?;
    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| agents_core::AgentsError::message(error.to_string()))?;
    payload
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| agents_core::AgentsError::message("conversation create response missing id"))
}

pub fn select_compaction_candidate_items(items: &[InputItem]) -> Vec<InputItem> {
    items
        .iter()
        .filter(|item| !is_user_like_item(item) && !is_compaction_marker(item))
        .cloned()
        .collect()
}

pub fn default_should_trigger_compaction(compaction_candidate_items: &[InputItem]) -> bool {
    compaction_candidate_items.len() >= DEFAULT_COMPACTION_THRESHOLD
}

pub fn is_openai_model_name(model: &str) -> bool {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return false;
    }

    let without_ft_prefix = trimmed.strip_prefix("ft:").unwrap_or(trimmed);
    let root = without_ft_prefix.split(':').next().unwrap_or_default();

    root.starts_with("gpt-")
        || (root.starts_with('o') && root.chars().nth(1).is_some_and(|ch| ch.is_ascii_digit()))
}

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

    fn session_settings(&self) -> Option<&SessionSettings> {
        self.inner.session_settings()
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>> {
        self.inner.get_items_with_limit(limit).await
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        self.inner.add_items(items).await
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        self.inner.pop_item().await
    }

    async fn clear_session(&self) -> Result<()> {
        self.inner.clear_session().await?;
        *self.conversation_id.lock().await = format!("conv_{}", Uuid::new_v4());
        *self.last_response_id.lock().await = None;
        Ok(())
    }

    fn conversation_session(&self) -> Option<&dyn OpenAIConversationAwareSession> {
        Some(self)
    }
}

#[async_trait]
impl OpenAIConversationAwareSession for OpenAIConversationsSession {
    async fn load_openai_conversation_state(&self) -> Result<OpenAIConversationSessionState> {
        Ok(OpenAIConversationSessionState {
            conversation_id: Some(self.conversation_id().await),
            previous_response_id: self.last_response_id().await,
            auto_previous_response_id: true,
        })
    }

    async fn save_openai_conversation_state(
        &self,
        state: OpenAIConversationSessionState,
    ) -> Result<()> {
        if let Some(conversation_id) = state.conversation_id {
            *self.conversation_id.lock().await = conversation_id;
        }
        *self.last_response_id.lock().await = state.previous_response_id;
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
    pub model: String,
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
            model: "gpt-4.1".to_owned(),
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

    pub fn with_model(mut self, model: impl Into<String>) -> Result<Self> {
        let model = model.into();
        if !is_openai_model_name(&model) {
            return Err(agents_core::AgentsError::message(format!(
                "unsupported model for OpenAI responses compaction: {model}"
            )));
        }
        self.model = model;
        Ok(self)
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
        Ok(select_compaction_candidate_items(
            &self.inner.get_items().await?,
        ))
    }

    pub async fn compaction_candidate_count(&self) -> Result<usize> {
        Ok(self.compaction_candidate_items().await?.len())
    }

    pub async fn should_compact(&self) -> Result<bool> {
        Ok(self.compaction_candidate_count().await? >= self.compaction_threshold)
    }

    pub async fn compact(&self) -> Result<()> {
        self.compact_with_force(false).await
    }

    async fn compact_with_force(&self, force: bool) -> Result<()> {
        let items = sanitize_compaction_items(&self.inner.get_items().await?);
        let candidate_indices = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (!is_user_like_item(item) && !is_compaction_marker(item)).then_some(index)
            })
            .collect::<Vec<_>>();
        if candidate_indices.len() <= self.compaction_threshold {
            if force {
                self.inner.clear().await?;
                return self.inner.add_items(items).await;
            }
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

    async fn compact_to_previous_response_id(
        &self,
        previous_response_id: String,
        force: bool,
    ) -> Result<()> {
        let items = sanitize_compaction_items(&self.inner.get_items().await?);
        let candidates = select_compaction_candidate_items(&items);
        if !force && candidates.len() < self.compaction_threshold {
            return Ok(());
        }

        let mut compacted = items
            .into_iter()
            .filter(|item| is_user_like_item(item) || is_compaction_marker(item))
            .collect::<Vec<_>>();
        compacted.push(InputItem::Json {
            value: serde_json::json!({
                "type": "compaction",
                "mode": "previous_response_id",
                "model": self.model,
                "previous_response_id": previous_response_id,
                "summary": format!("Compacted {} candidate item(s)", candidates.len()),
            }),
        });

        self.inner.clear().await?;
        self.inner.add_items(compacted).await?;
        *self.response_id.lock().await = Some(previous_response_id.clone());
        *self.deferred_response_id.lock().await = None;
        *self.last_unstored_response_id.lock().await = None;
        Ok(())
    }

    async fn resolve_compaction_mode(
        &self,
        requested_mode: OpenAIResponsesCompactionMode,
        response_id: Option<&str>,
        store: Option<bool>,
    ) -> OpenAIResponsesCompactionMode {
        if !matches!(requested_mode, OpenAIResponsesCompactionMode::Auto) {
            return requested_mode;
        }
        if matches!(store, Some(false)) {
            return OpenAIResponsesCompactionMode::Input;
        }
        if response_id.is_none() {
            return OpenAIResponsesCompactionMode::Input;
        }
        let last_unstored_response_id = self.last_unstored_response_id.lock().await.clone();
        if response_id == last_unstored_response_id.as_deref() {
            return OpenAIResponsesCompactionMode::Input;
        }
        OpenAIResponsesCompactionMode::PreviousResponseId
    }
}

#[async_trait]
impl Session for OpenAIResponsesCompactionSession {
    fn session_id(&self) -> &str {
        self.inner.session_id()
    }

    fn session_settings(&self) -> Option<&SessionSettings> {
        self.inner.session_settings()
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>> {
        self.inner.get_items_with_limit(limit).await
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

    async fn clear_session(&self) -> Result<()> {
        self.inner.clear_session().await?;
        *self.response_id.lock().await = None;
        *self.deferred_response_id.lock().await = None;
        *self.last_unstored_response_id.lock().await = None;
        Ok(())
    }

    fn conversation_session(&self) -> Option<&dyn OpenAIConversationAwareSession> {
        Some(self)
    }

    fn compaction_session(&self) -> Option<&dyn OpenAIResponsesCompactionAwareSession> {
        Some(self)
    }
}

#[async_trait]
impl OpenAIConversationAwareSession for OpenAIResponsesCompactionSession {
    async fn load_openai_conversation_state(&self) -> Result<OpenAIConversationSessionState> {
        let deferred_response_id = self.deferred_response_id.lock().await.clone();
        let response_id = self.response_id.lock().await.clone();
        let last_unstored_response_id = self.last_unstored_response_id.lock().await.clone();
        let previous_response_id = match self.mode {
            OpenAIResponsesCompactionMode::Input => None,
            OpenAIResponsesCompactionMode::PreviousResponseId
            | OpenAIResponsesCompactionMode::Auto => {
                let candidate = deferred_response_id.or(response_id);
                (candidate.as_deref() != last_unstored_response_id.as_deref()).then_some(candidate)
            }
            .flatten(),
        };
        let auto_previous_response_id = !matches!(self.mode, OpenAIResponsesCompactionMode::Input);

        Ok(OpenAIConversationSessionState {
            conversation_id: None,
            previous_response_id,
            auto_previous_response_id,
        })
    }

    async fn save_openai_conversation_state(
        &self,
        state: OpenAIConversationSessionState,
    ) -> Result<()> {
        match self.mode {
            OpenAIResponsesCompactionMode::Input => {
                *self.response_id.lock().await = None;
                *self.deferred_response_id.lock().await = None;
                *self.last_unstored_response_id.lock().await = None;
            }
            OpenAIResponsesCompactionMode::PreviousResponseId
            | OpenAIResponsesCompactionMode::Auto => {
                *self.response_id.lock().await = state.previous_response_id.clone();
                *self.deferred_response_id.lock().await = None;
                *self.last_unstored_response_id.lock().await = None;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl OpenAIResponsesCompactionAwareSession for OpenAIResponsesCompactionSession {
    async fn run_compaction(&self, args: Option<OpenAIResponsesCompactionArgs>) -> Result<()> {
        let args = args.unwrap_or_default();
        let requested_mode = match args.compaction_mode.as_deref() {
            Some("previous_response_id") => OpenAIResponsesCompactionMode::PreviousResponseId,
            Some("input") => OpenAIResponsesCompactionMode::Input,
            Some("auto") | None => self.mode,
            Some(other) => {
                return Err(agents_core::AgentsError::message(format!(
                    "unsupported compaction mode `{other}`"
                )));
            }
        };
        let force = args.force.unwrap_or(false);
        if let Some(response_id) = args.response_id.clone() {
            *self.response_id.lock().await = Some(response_id.clone());
            if matches!(args.store, Some(false)) {
                *self.last_unstored_response_id.lock().await = Some(response_id);
            } else if matches!(args.store, Some(true)) {
                *self.last_unstored_response_id.lock().await = None;
            }
        }
        let deferred_response_id = self.deferred_response_id.lock().await.clone();
        let current_response_id = self.response_id.lock().await.clone();
        let response_id = args
            .response_id
            .or(deferred_response_id)
            .or(current_response_id);
        let mode = self
            .resolve_compaction_mode(requested_mode, response_id.as_deref(), args.store)
            .await;

        match mode {
            OpenAIResponsesCompactionMode::Input => self.compact_with_force(force).await,
            OpenAIResponsesCompactionMode::PreviousResponseId => {
                let response_id = response_id.ok_or_else(|| {
                    agents_core::AgentsError::message(
                        "previous_response_id compaction requires a response id",
                    )
                })?;
                self.compact_to_previous_response_id(response_id, force)
                    .await
            }
            OpenAIResponsesCompactionMode::Auto => {
                if let Some(response_id) = response_id {
                    self.compact_to_previous_response_id(response_id, force)
                        .await
                } else {
                    self.compact_with_force(force).await
                }
            }
        }
    }
}

fn sanitize_compaction_items(items: &[InputItem]) -> Vec<InputItem> {
    items.iter().map(sanitize_compaction_item).collect()
}

fn sanitize_compaction_item(item: &InputItem) -> InputItem {
    match item {
        InputItem::Text { .. } => item.clone(),
        InputItem::Json { value } => {
            let Some(object) = value.as_object() else {
                return item.clone();
            };
            let mut sanitized = object.clone();
            sanitized.remove(TOOL_CALL_SESSION_DESCRIPTION_KEY);
            sanitized.remove(TOOL_CALL_SESSION_TITLE_KEY);
            InputItem::Json {
                value: Value::Object(sanitized),
            }
        }
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    async fn serve_single_json_response(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener address");
        let status = status.to_owned();
        let body = body.to_owned();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request should connect");
            let mut buffer = vec![0_u8; 4096];
            let _ = stream.read(&mut buffer).await.expect("request should read");
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len(),
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
        });

        format!("http://{address}/v1")
    }

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

    #[tokio::test]
    async fn conversations_session_exposes_conversation_aware_state() {
        let session = OpenAIConversationsSession::new("conv-1");
        session.set_last_response_id("resp-1").await;

        let state = session
            .load_openai_conversation_state()
            .await
            .expect("conversation state should load");

        assert_eq!(state.conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(state.previous_response_id.as_deref(), Some("resp-1"));
        assert!(state.auto_previous_response_id);
    }

    #[tokio::test]
    async fn compaction_input_mode_disables_previous_response_replay() {
        let session = OpenAIResponsesCompactionSession::new("session")
            .with_mode(OpenAIResponsesCompactionMode::Input);
        session.set_response_id("resp-1").await;

        let state = session
            .load_openai_conversation_state()
            .await
            .expect("conversation state should load");

        assert_eq!(state.previous_response_id, None);
        assert!(!state.auto_previous_response_id);
    }

    #[tokio::test]
    async fn compaction_validates_override_model_names() {
        let error = OpenAIResponsesCompactionSession::new("session")
            .with_model("claude-3")
            .expect_err("non-openai model should fail");

        assert!(error.to_string().contains("unsupported model"));
    }

    #[tokio::test]
    async fn previous_response_id_compaction_requires_response_id() {
        let session = OpenAIResponsesCompactionSession::new("session")
            .with_mode(OpenAIResponsesCompactionMode::PreviousResponseId);

        let error = session
            .run_compaction(Some(OpenAIResponsesCompactionArgs {
                force: Some(true),
                ..OpenAIResponsesCompactionArgs::default()
            }))
            .await
            .expect_err("previous_response_id compaction should require a response id");

        assert!(
            error
                .to_string()
                .contains("previous_response_id compaction requires a response id")
        );
    }

    #[tokio::test]
    async fn auto_compaction_prefers_previous_response_id_when_available() {
        let session = OpenAIResponsesCompactionSession::new("session");
        session
            .add_items(vec![
                InputItem::from("hello"),
                InputItem::Json {
                    value: serde_json::json!({"type":"tool_call_output","call_id":"call-1"}),
                },
                InputItem::Json {
                    value: serde_json::json!({"type":"reasoning","text":"thinking"}),
                },
            ])
            .await
            .expect("items should be stored");
        session.set_response_id("resp-prev").await;

        session
            .run_compaction(Some(OpenAIResponsesCompactionArgs {
                force: Some(true),
                ..OpenAIResponsesCompactionArgs::default()
            }))
            .await
            .expect("compaction should succeed");

        let items = session.get_items().await.expect("items should load");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_text(), Some("hello"));
        assert_eq!(
            items[1],
            InputItem::Json {
                value: serde_json::json!({
                    "type": "compaction",
                    "mode": "previous_response_id",
                    "model": "gpt-4.1",
                    "previous_response_id": "resp-prev",
                    "summary": "Compacted 2 candidate item(s)",
                }),
            }
        );
    }

    #[tokio::test]
    async fn input_compaction_strips_internal_tool_metadata() {
        let session = OpenAIResponsesCompactionSession::new("session")
            .with_compaction_threshold(1)
            .with_mode(OpenAIResponsesCompactionMode::Input);
        session
            .add_items(vec![InputItem::Json {
                value: serde_json::json!({
                    "type":"tool_call",
                    "tool_name":"lookup_account",
                    "call_id":"call_123",
                    "arguments":{},
                    TOOL_CALL_SESSION_DESCRIPTION_KEY: "Lookup customer records.",
                    TOOL_CALL_SESSION_TITLE_KEY: "Lookup Account",
                }),
            }])
            .await
            .expect("items should be stored");

        session
            .run_compaction(Some(OpenAIResponsesCompactionArgs {
                force: Some(true),
                compaction_mode: Some("input".to_owned()),
                ..OpenAIResponsesCompactionArgs::default()
            }))
            .await
            .expect("compaction should succeed");

        let items = session.get_items().await.expect("items should load");
        let json = match &items[0] {
            InputItem::Json { value } => value,
            InputItem::Text { .. } => panic!("expected json item"),
        };
        assert!(json.get(TOOL_CALL_SESSION_DESCRIPTION_KEY).is_none());
        assert!(json.get(TOOL_CALL_SESSION_TITLE_KEY).is_none());
    }

    #[tokio::test]
    async fn previous_response_id_compaction_respects_custom_threshold() {
        let session = OpenAIResponsesCompactionSession::new("session")
            .with_compaction_threshold(1)
            .with_mode(OpenAIResponsesCompactionMode::PreviousResponseId);
        session
            .add_items(vec![
                InputItem::from("hello"),
                InputItem::Json {
                    value: serde_json::json!({"type":"tool_call_output","call_id":"call-1"}),
                },
            ])
            .await
            .expect("items should be stored");

        session
            .run_compaction(Some(OpenAIResponsesCompactionArgs {
                response_id: Some("resp-next".to_owned()),
                ..OpenAIResponsesCompactionArgs::default()
            }))
            .await
            .expect("compaction should succeed");

        let items = session.get_items().await.expect("items should load");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_text(), Some("hello"));
        match &items[1] {
            InputItem::Json { value } => {
                assert_eq!(value["type"], "compaction");
                assert_eq!(value["previous_response_id"], "resp-next");
            }
            InputItem::Text { .. } => panic!("expected compaction item"),
        }
    }

    #[tokio::test]
    async fn auto_compaction_uses_input_mode_after_unstored_response() {
        let session = OpenAIResponsesCompactionSession::new("session");
        session
            .add_items(vec![
                InputItem::from("hello"),
                InputItem::Json {
                    value: serde_json::json!({"type":"tool_call_output","call_id":"call-1"}),
                },
            ])
            .await
            .expect("items should be stored");

        session
            .run_compaction(Some(OpenAIResponsesCompactionArgs {
                response_id: Some("resp-unstored".to_owned()),
                store: Some(false),
                force: Some(true),
                ..OpenAIResponsesCompactionArgs::default()
            }))
            .await
            .expect("compaction should succeed");

        let state = session
            .load_openai_conversation_state()
            .await
            .expect("conversation state should load");

        assert_eq!(state.previous_response_id, None);
        assert_eq!(
            session.last_unstored_response_id().await.as_deref(),
            Some("resp-unstored")
        );
    }

    #[tokio::test]
    async fn start_openai_conversations_session_reads_remote_conversation_id() {
        let base_url = serve_single_json_response("201 Created", r#"{"id":"conv_remote"}"#).await;
        let conversation_id = start_openai_conversations_session(Some(
            OpenAIClientOptions::new(Some("sk-test".to_owned())).with_base_url(base_url),
        ))
        .await
        .expect("conversation should start");

        assert_eq!(conversation_id, "conv_remote");
    }

    #[tokio::test]
    async fn start_openai_conversations_session_rejects_non_success_statuses() {
        let base_url = serve_single_json_response("400 Bad Request", r#"{"error":"bad"}"#).await;
        let error = start_openai_conversations_session(Some(
            OpenAIClientOptions::new(Some("sk-test".to_owned())).with_base_url(base_url),
        ))
        .await
        .expect_err("non-success status should fail");

        assert!(error.to_string().contains("400 Bad Request"));
    }

    #[test]
    fn validates_openai_model_names() {
        assert!(is_openai_model_name("gpt-4o"));
        assert!(is_openai_model_name("gpt-5"));
        assert!(is_openai_model_name("o3"));
        assert!(is_openai_model_name("ft:gpt-4.1:org:proj:suffix"));
        assert!(!is_openai_model_name(""));
        assert!(!is_openai_model_name("not-openai"));
    }

    #[test]
    fn selects_compaction_candidates_by_skipping_user_and_compaction_items() {
        let items = vec![
            InputItem::from("hello"),
            InputItem::Json {
                value: serde_json::json!({"type":"tool_call_output","call_id":"1"}),
            },
            InputItem::Json {
                value: serde_json::json!({"type":"compaction","summary":"done"}),
            },
            InputItem::Json {
                value: serde_json::json!({"type":"reasoning","text":"thinking"}),
            },
        ];

        let selected = select_compaction_candidate_items(&items);

        assert_eq!(selected.len(), 2);
        assert!(
            selected
                .iter()
                .all(|item| !matches!(item, InputItem::Text { .. }))
        );
        assert!(default_should_trigger_compaction(&vec![InputItem::Json {
            value: serde_json::json!({"type":"tool_call_output"})
        }; DEFAULT_COMPACTION_THRESHOLD]));
    }
}
