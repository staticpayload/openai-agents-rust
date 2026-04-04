use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::{AgentsError, Result};
use crate::models::interface::{Model, ModelProvider, ModelRequest, ModelResponse};

#[derive(Clone, Default)]
pub struct MultiProviderMap {
    mapping: HashMap<String, Arc<dyn ModelProvider>>,
}

impl fmt::Debug for MultiProviderMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefixes = self.mapping.keys().cloned().collect::<Vec<_>>();
        f.debug_struct("MultiProviderMap")
            .field("prefixes", &prefixes)
            .finish()
    }
}

impl MultiProviderMap {
    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.mapping.contains_key(prefix)
    }

    pub fn get_mapping(&self) -> HashMap<String, Arc<dyn ModelProvider>> {
        self.mapping.clone()
    }

    pub fn set_mapping(&mut self, mapping: HashMap<String, Arc<dyn ModelProvider>>) {
        self.mapping = mapping;
    }

    pub fn get_provider(&self, prefix: &str) -> Option<Arc<dyn ModelProvider>> {
        self.mapping.get(prefix).cloned()
    }

    pub fn add_provider(&mut self, prefix: impl Into<String>, provider: Arc<dyn ModelProvider>) {
        self.mapping.insert(prefix.into(), provider);
    }

    pub fn remove_provider(&mut self, prefix: &str) -> Option<Arc<dyn ModelProvider>> {
        self.mapping.remove(prefix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiProviderOpenAIPrefixMode {
    Alias,
    ModelId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiProviderUnknownPrefixMode {
    Error,
    ModelId,
}

#[derive(Clone)]
pub struct MultiProvider {
    default_provider: Arc<dyn ModelProvider>,
    provider_map: MultiProviderMap,
    openai_prefix_mode: MultiProviderOpenAIPrefixMode,
    unknown_prefix_mode: MultiProviderUnknownPrefixMode,
}

impl fmt::Debug for MultiProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiProvider")
            .field("provider_map", &self.provider_map)
            .field("openai_prefix_mode", &self.openai_prefix_mode)
            .field("unknown_prefix_mode", &self.unknown_prefix_mode)
            .finish()
    }
}

impl MultiProvider {
    pub fn new(default_provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            default_provider,
            provider_map: MultiProviderMap::default(),
            openai_prefix_mode: MultiProviderOpenAIPrefixMode::Alias,
            unknown_prefix_mode: MultiProviderUnknownPrefixMode::Error,
        }
    }

    pub fn with_provider_map(mut self, provider_map: MultiProviderMap) -> Self {
        self.provider_map = provider_map;
        self
    }

    pub fn with_openai_prefix_mode(mut self, mode: MultiProviderOpenAIPrefixMode) -> Self {
        self.openai_prefix_mode = mode;
        self
    }

    pub fn with_unknown_prefix_mode(mut self, mode: MultiProviderUnknownPrefixMode) -> Self {
        self.unknown_prefix_mode = mode;
        self
    }

    pub fn provider_map(&self) -> &MultiProviderMap {
        &self.provider_map
    }

    fn split_model_name<'a>(
        &self,
        model_name: Option<&'a str>,
    ) -> (Option<&'a str>, Option<&'a str>) {
        let Some(model_name) = model_name else {
            return (None, None);
        };

        if let Some((prefix, remainder)) = model_name.split_once('/') {
            (Some(prefix), Some(remainder))
        } else {
            (None, Some(model_name))
        }
    }
}

struct UnavailableModel {
    message: String,
}

#[async_trait]
impl Model for UnavailableModel {
    async fn generate(&self, _request: ModelRequest) -> Result<ModelResponse> {
        Err(AgentsError::message(self.message.clone()))
    }
}

impl ModelProvider for MultiProvider {
    fn resolve(&self, model: Option<&str>) -> Arc<dyn Model> {
        let (prefix, stripped_name) = self.split_model_name(model);

        if let Some(prefix) = prefix {
            if let Some(provider) = self.provider_map.get_provider(prefix) {
                return provider.resolve(stripped_name);
            }

            if prefix == "openai" {
                return match self.openai_prefix_mode {
                    MultiProviderOpenAIPrefixMode::Alias => {
                        self.default_provider.resolve(stripped_name)
                    }
                    MultiProviderOpenAIPrefixMode::ModelId => self.default_provider.resolve(model),
                };
            }

            return match self.unknown_prefix_mode {
                MultiProviderUnknownPrefixMode::ModelId => self.default_provider.resolve(model),
                MultiProviderUnknownPrefixMode::Error => Arc::new(UnavailableModel {
                    message: format!("unknown model provider prefix `{prefix}`"),
                }),
            };
        }

        self.default_provider.resolve(model)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct CaptureModel {
        seen: Mutex<Vec<Option<String>>>,
    }

    #[async_trait]
    impl Model for CaptureModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            self.seen.lock().expect("seen lock").push(request.model);
            Ok(ModelResponse::default())
        }
    }

    #[derive(Clone)]
    struct CaptureProvider {
        model: Arc<CaptureModel>,
    }

    impl CaptureProvider {
        fn new(model: Arc<CaptureModel>) -> Self {
            Self { model }
        }
    }

    impl ModelProvider for CaptureProvider {
        fn resolve(&self, model: Option<&str>) -> Arc<dyn Model> {
            let model_name = model.map(ToOwned::to_owned);
            Arc::new(CapturedModel {
                inner: self.model.clone(),
                resolved_model: model_name,
            })
        }
    }

    struct CapturedModel {
        inner: Arc<CaptureModel>,
        resolved_model: Option<String>,
    }

    #[async_trait]
    impl Model for CapturedModel {
        async fn generate(&self, mut request: ModelRequest) -> Result<ModelResponse> {
            request.model = self.resolved_model.clone();
            self.inner.generate(request).await
        }
    }

    #[tokio::test]
    async fn routes_openai_alias_to_default_provider() {
        let capture = Arc::new(CaptureModel::default());
        let provider = MultiProvider::new(Arc::new(CaptureProvider::new(capture.clone())));
        let model = provider.resolve(Some("openai/gpt-5"));
        model
            .generate(ModelRequest {
                model: Some("ignored".to_owned()),
                ..ModelRequest::default()
            })
            .await
            .expect("generation should succeed");

        let seen = capture.seen.lock().expect("seen lock");
        assert_eq!(seen.as_slice(), &[Some("gpt-5".to_owned())]);
    }

    #[tokio::test]
    async fn routes_explicit_prefixes_via_provider_map() {
        let default_capture = Arc::new(CaptureModel::default());
        let custom_capture = Arc::new(CaptureModel::default());

        let mut map = MultiProviderMap::default();
        map.add_provider(
            "custom",
            Arc::new(CaptureProvider::new(custom_capture.clone())),
        );

        let provider = MultiProvider::new(Arc::new(CaptureProvider::new(default_capture)))
            .with_provider_map(map);
        let model = provider.resolve(Some("custom/router-model"));
        model
            .generate(ModelRequest::default())
            .await
            .expect("generation should succeed");

        let seen = custom_capture.seen.lock().expect("seen lock");
        assert_eq!(seen.as_slice(), &[Some("router-model".to_owned())]);
    }
}
