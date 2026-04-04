//! Optional integrations and experimental APIs.

pub mod experimental {
    pub mod codex {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Default, Serialize, Deserialize)]
        pub struct CodexTool {
            pub name: String,
        }
    }
}

pub mod memory {
    use agents_core::{InputItem, MemorySession, Result, Session, SessionSettings};
    use async_trait::async_trait;

    #[derive(Clone, Debug)]
    pub struct AdvancedSqliteSession {
        pub path: String,
        inner: MemorySession,
    }

    impl AdvancedSqliteSession {
        pub fn new(session_id: impl Into<String>, path: impl Into<String>) -> Self {
            Self {
                path: path.into(),
                inner: MemorySession::new(session_id),
            }
        }
    }

    #[async_trait]
    impl Session for AdvancedSqliteSession {
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
            self.inner.clear_session().await
        }
    }

    #[derive(Clone, Debug)]
    pub struct DatabaseSession {
        pub connection_string: String,
        inner: MemorySession,
    }

    impl DatabaseSession {
        pub fn new(session_id: impl Into<String>, connection_string: impl Into<String>) -> Self {
            Self {
                connection_string: connection_string.into(),
                inner: MemorySession::new(session_id),
            }
        }
    }

    #[async_trait]
    impl Session for DatabaseSession {
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
            self.inner.clear_session().await
        }
    }

    #[derive(Clone, Debug)]
    pub struct RedisSession {
        pub url: String,
        inner: MemorySession,
    }

    impl RedisSession {
        pub fn new(session_id: impl Into<String>, url: impl Into<String>) -> Self {
            Self {
                url: url.into(),
                inner: MemorySession::new(session_id),
            }
        }
    }

    #[async_trait]
    impl Session for RedisSession {
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
            self.inner.clear_session().await
        }
    }

    #[derive(Clone, Debug)]
    pub struct DaprSession {
        pub address: String,
        inner: MemorySession,
    }

    impl DaprSession {
        pub fn new(session_id: impl Into<String>, address: impl Into<String>) -> Self {
            Self {
                address: address.into(),
                inner: MemorySession::new(session_id),
            }
        }
    }

    #[async_trait]
    impl Session for DaprSession {
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
            self.inner.clear_session().await
        }
    }

    #[derive(Clone, Debug)]
    pub struct EncryptedSession<S> {
        pub key_id: String,
        pub inner: S,
    }

    impl<S> EncryptedSession<S> {
        pub fn new(inner: S, key_id: impl Into<String>) -> Self {
            Self {
                key_id: key_id.into(),
                inner,
            }
        }
    }
}

pub mod providers {
    use std::sync::Arc;

    use agents_core::{Model, ModelProvider};
    use agents_openai::OpenAIProvider;

    #[derive(Clone, Debug, Default)]
    pub struct LiteLlmProvider {
        inner: OpenAIProvider,
    }

    impl LiteLlmProvider {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl ModelProvider for LiteLlmProvider {
        fn resolve(&self, model: Option<&str>) -> Arc<dyn Model> {
            self.inner.resolve(model)
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct AnyLlmProvider {
        inner: OpenAIProvider,
    }

    impl AnyLlmProvider {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl ModelProvider for AnyLlmProvider {
        fn resolve(&self, model: Option<&str>) -> Arc<dyn Model> {
            self.inner.resolve(model)
        }
    }
}

pub mod tool_output_trimmer {
    #[derive(Clone, Debug, Default)]
    pub struct ToolOutputTrimmer {
        pub max_chars: usize,
    }
}

pub mod visualization {
    #[derive(Clone, Debug, Default)]
    pub struct VisualizationGraph {
        pub nodes: usize,
        pub edges: usize,
    }
}
