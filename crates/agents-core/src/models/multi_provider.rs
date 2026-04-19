use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::errors::{AgentsError, Result};
use crate::model_settings::ModelSettings;
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

    fn resolve_provider_for_model<'a>(
        &self,
        model: Option<&'a str>,
    ) -> Option<(Arc<dyn ModelProvider>, Option<&'a str>)> {
        let (prefix, stripped_name) = self.split_model_name(model);

        if let Some(prefix) = prefix {
            if let Some(provider) = self.provider_map.get_provider(prefix) {
                return Some((provider, stripped_name));
            }

            if prefix == "openai" {
                return Some(match self.openai_prefix_mode {
                    MultiProviderOpenAIPrefixMode::Alias => {
                        (self.default_provider.clone(), stripped_name)
                    }
                    MultiProviderOpenAIPrefixMode::ModelId => {
                        (self.default_provider.clone(), model)
                    }
                });
            }

            return match self.unknown_prefix_mode {
                MultiProviderUnknownPrefixMode::ModelId => {
                    Some((self.default_provider.clone(), model))
                }
                MultiProviderUnknownPrefixMode::Error => None,
            };
        }

        Some((self.default_provider.clone(), model))
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
        if let Some((provider, resolved_model)) = self.resolve_provider_for_model(model) {
            return provider.resolve(resolved_model);
        }

        let (prefix, _) = self.split_model_name(model);
        Arc::new(UnavailableModel {
            message: format!(
                "unknown model provider prefix `{}`",
                prefix.unwrap_or("unknown")
            ),
        })
    }

    fn resolve_trace_metadata(
        &self,
        model: Option<&str>,
        metadata: Option<&BTreeMap<String, Value>>,
    ) -> Option<BTreeMap<String, Value>> {
        if let Some((provider, resolved_model)) = self.resolve_provider_for_model(model) {
            return provider.resolve_trace_metadata(resolved_model, metadata);
        }

        metadata.cloned()
    }

    fn prepare_request(&self, mut request: ModelRequest) -> ModelRequest {
        if let Some((provider, resolved_model)) =
            self.resolve_provider_for_model(request.model.as_deref())
        {
            request.model = resolved_model.map(ToOwned::to_owned);
            return provider.prepare_request(request);
        }

        request
    }

    fn resolve_with_settings(
        &self,
        model: Option<&str>,
        settings: &ModelSettings,
    ) -> Arc<dyn Model> {
        if let Some((provider, resolved_model)) = self.resolve_provider_for_model(model) {
            return provider.resolve_with_settings(resolved_model, settings);
        }

        let (prefix, _) = self.split_model_name(model);
        Arc::new(UnavailableModel {
            message: format!(
                "unknown model provider prefix `{}`",
                prefix.unwrap_or("unknown")
            ),
        })
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

    #[tokio::test]
    async fn preserves_openai_prefix_as_literal_model_id_when_requested() {
        let capture = Arc::new(CaptureModel::default());
        let provider = MultiProvider::new(Arc::new(CaptureProvider::new(capture.clone())))
            .with_openai_prefix_mode(MultiProviderOpenAIPrefixMode::ModelId);
        let model = provider.resolve(Some("openai/gpt-5"));
        model
            .generate(ModelRequest::default())
            .await
            .expect("generation should succeed");

        let seen = capture.seen.lock().expect("seen lock");
        assert_eq!(seen.as_slice(), &[Some("openai/gpt-5".to_owned())]);
    }

    #[tokio::test]
    async fn preserves_unknown_prefix_as_literal_model_id_when_requested() {
        let capture = Arc::new(CaptureModel::default());
        let provider = MultiProvider::new(Arc::new(CaptureProvider::new(capture.clone())))
            .with_unknown_prefix_mode(MultiProviderUnknownPrefixMode::ModelId);
        let model = provider.resolve(Some("openrouter/openai/gpt-5"));
        model
            .generate(ModelRequest::default())
            .await
            .expect("generation should succeed");

        let seen = capture.seen.lock().expect("seen lock");
        assert_eq!(
            seen.as_slice(),
            &[Some("openrouter/openai/gpt-5".to_owned())]
        );
    }

    #[tokio::test]
    async fn explicit_provider_map_overrides_openai_prefix_mode() {
        let default_capture = Arc::new(CaptureModel::default());
        let custom_capture = Arc::new(CaptureModel::default());

        let mut map = MultiProviderMap::default();
        map.add_provider(
            "openai",
            Arc::new(CaptureProvider::new(custom_capture.clone())),
        );

        let provider = MultiProvider::new(Arc::new(CaptureProvider::new(default_capture)))
            .with_provider_map(map)
            .with_openai_prefix_mode(MultiProviderOpenAIPrefixMode::ModelId);
        let model = provider.resolve(Some("openai/gpt-5"));
        model
            .generate(ModelRequest::default())
            .await
            .expect("generation should succeed");

        let seen = custom_capture.seen.lock().expect("seen lock");
        assert_eq!(seen.as_slice(), &[Some("gpt-5".to_owned())]);
    }

    #[tokio::test]
    async fn multi_provider_routes_models_predictably() {
        let default_capture = Arc::new(CaptureModel::default());
        let openrouter_capture = Arc::new(CaptureModel::default());
        let explicit_openai_capture = Arc::new(CaptureModel::default());

        let mut provider_map = MultiProviderMap::default();
        provider_map.add_provider(
            "openrouter",
            Arc::new(CaptureProvider::new(openrouter_capture.clone())),
        );
        provider_map.add_provider(
            "openai",
            Arc::new(CaptureProvider::new(explicit_openai_capture.clone())),
        );

        let alias_provider =
            MultiProvider::new(Arc::new(CaptureProvider::new(default_capture.clone())));
        alias_provider
            .resolve(Some("openai/gpt-5"))
            .generate(ModelRequest::default())
            .await
            .expect("openai alias should resolve through the default provider");

        let literal_provider =
            MultiProvider::new(Arc::new(CaptureProvider::new(default_capture.clone())))
                .with_openai_prefix_mode(MultiProviderOpenAIPrefixMode::ModelId)
                .with_unknown_prefix_mode(MultiProviderUnknownPrefixMode::ModelId);
        literal_provider
            .resolve(Some("openai/gpt-5"))
            .generate(ModelRequest::default())
            .await
            .expect("model-id mode should preserve the openai prefix");
        literal_provider
            .resolve(Some("unknown/gpt-5"))
            .generate(ModelRequest::default())
            .await
            .expect("unknown prefixes should be passed through in model-id mode");

        let unknown_error =
            MultiProvider::new(Arc::new(CaptureProvider::new(default_capture.clone())))
                .resolve(Some("unknown/gpt-5"))
                .generate(ModelRequest::default())
                .await
                .expect_err("unknown prefixes should error by default");
        assert!(
            unknown_error
                .to_string()
                .contains("unknown model provider prefix `unknown`")
        );

        let mapped_provider =
            MultiProvider::new(Arc::new(CaptureProvider::new(default_capture.clone())))
                .with_provider_map(provider_map)
                .with_openai_prefix_mode(MultiProviderOpenAIPrefixMode::ModelId);
        mapped_provider
            .resolve(Some("openrouter/gpt-5"))
            .generate(ModelRequest::default())
            .await
            .expect("explicit provider-map entries should route by prefix");
        mapped_provider
            .resolve(Some("openai/gpt-5"))
            .generate(ModelRequest::default())
            .await
            .expect("provider-map openai entries should override built-in prefix handling");

        let default_seen = default_capture.seen.lock().expect("seen lock");
        assert_eq!(
            default_seen.as_slice(),
            &[
                Some("gpt-5".to_owned()),
                Some("openai/gpt-5".to_owned()),
                Some("unknown/gpt-5".to_owned()),
            ]
        );
        drop(default_seen);

        let openrouter_seen = openrouter_capture.seen.lock().expect("seen lock");
        assert_eq!(openrouter_seen.as_slice(), &[Some("gpt-5".to_owned())]);
        drop(openrouter_seen);

        let explicit_openai_seen = explicit_openai_capture.seen.lock().expect("seen lock");
        assert_eq!(explicit_openai_seen.as_slice(), &[Some("gpt-5".to_owned())]);
    }

    #[test]
    fn forwards_trace_metadata_hooks_through_routed_provider() {
        #[derive(Clone)]
        struct HookProvider;

        impl ModelProvider for HookProvider {
            fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
                Arc::new(UnavailableModel {
                    message: "unused".to_owned(),
                })
            }

            fn resolve_trace_metadata(
                &self,
                _model: Option<&str>,
                metadata: Option<&BTreeMap<String, Value>>,
            ) -> Option<BTreeMap<String, Value>> {
                let mut metadata = metadata.cloned().unwrap_or_default();
                metadata.insert(
                    "agent_harness_id".to_owned(),
                    Value::String("hooked".to_owned()),
                );
                Some(metadata)
            }

            fn prepare_request(&self, mut request: ModelRequest) -> ModelRequest {
                request.settings.metadata.insert(
                    "agent_harness_id".to_owned(),
                    Value::String("hooked".to_owned()),
                );
                request
            }
        }

        let provider = MultiProvider::new(Arc::new(HookProvider));
        let trace_metadata = provider
            .resolve_trace_metadata(Some("openai/gpt-5"), None)
            .expect("metadata should exist");
        assert_eq!(
            trace_metadata.get("agent_harness_id"),
            Some(&Value::String("hooked".to_owned()))
        );
        assert_eq!(
            provider
                .prepare_request(ModelRequest {
                    model: Some("openai/gpt-5".to_owned()),
                    ..Default::default()
                })
                .settings
                .metadata
                .get("agent_harness_id"),
            Some(&Value::String("hooked".to_owned()))
        );
    }
}
