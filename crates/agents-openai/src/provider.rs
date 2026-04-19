use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use agents_core::{Model, ModelProvider, ModelRequest, get_default_model};
use serde_json::Value;

use crate::defaults::{OpenAIApi, default_openai_api, default_openai_key};
use crate::models::{
    OpenAIChatCompletionsModel, OpenAIClientOptions, OpenAIResponsesModel, OpenAIResponsesWsModel,
};
use crate::openai_agent_registration::{
    OpenAIAgentRegistrationConfig, ResolvedOpenAIAgentRegistrationConfig,
    merge_openai_harness_id_into_metadata, resolve_openai_agent_registration_config,
};
use crate::{
    get_default_openai_websocket_base_url, get_openai_base_url, get_use_responses_by_default,
    get_use_responses_websocket_by_default,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenAIResponsesTransport {
    Http,
    WebSocket,
}

#[derive(Clone, Debug)]
pub struct OpenAIProvider {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub websocket_base_url: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub api: Option<OpenAIApi>,
    pub use_responses: Option<bool>,
    pub use_responses_websocket: bool,
    pub agent_registration: Option<ResolvedOpenAIAgentRegistrationConfig>,
    websocket_model_cache:
        Arc<Mutex<HashMap<(String, OpenAIClientOptions), Arc<OpenAIResponsesWsModel>>>>,
}

impl Default for OpenAIProvider {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            websocket_base_url: None,
            organization: None,
            project: None,
            api: None,
            use_responses: None,
            use_responses_websocket: get_use_responses_websocket_by_default() == Some(true),
            agent_registration: None,
            websocket_model_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl OpenAIProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn with_websocket_base_url(mut self, websocket_base_url: impl Into<String>) -> Self {
        self.websocket_base_url = Some(websocket_base_url.into());
        self
    }

    pub fn with_organization(mut self, organization: impl Into<String>) -> Self {
        self.organization = Some(organization.into());
        self
    }

    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn with_api(mut self, api: OpenAIApi) -> Self {
        self.api = Some(api);
        self
    }

    pub fn with_use_responses(mut self, use_responses: bool) -> Self {
        self.use_responses = Some(use_responses);
        self
    }

    pub fn with_use_responses_websocket(mut self, use_responses_websocket: bool) -> Self {
        self.use_responses_websocket = use_responses_websocket;
        self
    }

    pub fn with_agent_registration(
        mut self,
        agent_registration: OpenAIAgentRegistrationConfig,
    ) -> Self {
        self.agent_registration =
            resolve_openai_agent_registration_config(Some(&agent_registration));
        self
    }

    pub fn client_options(&self) -> OpenAIClientOptions {
        OpenAIClientOptions {
            api_key: self.api_key.clone().or_else(default_openai_key),
            base_url: self.base_url.clone().unwrap_or_else(get_openai_base_url),
            websocket_base_url: self
                .websocket_base_url
                .clone()
                .unwrap_or_else(get_default_openai_websocket_base_url),
            organization: self.organization.clone(),
            project: self.project.clone(),
        }
    }

    pub fn resolved_api(&self) -> OpenAIApi {
        if let Some(use_responses) = self.use_responses {
            if use_responses {
                OpenAIApi::Responses
            } else {
                OpenAIApi::ChatCompletions
            }
        } else if let Some(api) = self.api {
            api
        } else if let Some(use_responses_by_default) = get_use_responses_by_default() {
            if use_responses_by_default {
                OpenAIApi::Responses
            } else {
                OpenAIApi::ChatCompletions
            }
        } else {
            default_openai_api()
        }
    }

    pub fn responses_transport(&self) -> OpenAIResponsesTransport {
        if self.use_responses_websocket {
            OpenAIResponsesTransport::WebSocket
        } else {
            OpenAIResponsesTransport::Http
        }
    }

    fn resolve_responses_ws_model(&self, model_name: &str) -> Arc<dyn Model> {
        let client_options = self.client_options();
        let mut cache = self
            .websocket_model_cache
            .lock()
            .expect("openai provider websocket cache");
        let entry = cache
            .entry((model_name.to_owned(), client_options.clone()))
            .or_insert_with(|| {
                Arc::new(OpenAIResponsesWsModel::new(
                    model_name.to_owned(),
                    client_options,
                ))
            })
            .clone();
        entry
    }

    pub fn effective_harness_id(&self) -> Option<String> {
        self.agent_registration
            .as_ref()
            .map(|registration| registration.harness_id.clone())
            .or_else(|| {
                resolve_openai_agent_registration_config(None)
                    .map(|registration| registration.harness_id)
            })
    }
}

impl ModelProvider for OpenAIProvider {
    fn resolve_trace_metadata(
        &self,
        _model: Option<&str>,
        metadata: Option<&BTreeMap<String, Value>>,
    ) -> Option<BTreeMap<String, Value>> {
        merge_openai_harness_id_into_metadata(metadata, self.effective_harness_id().as_deref())
    }

    fn prepare_request(&self, mut request: ModelRequest) -> ModelRequest {
        request.settings.metadata = merge_openai_harness_id_into_metadata(
            Some(&request.settings.metadata),
            self.effective_harness_id().as_deref(),
        )
        .unwrap_or_default();
        request
    }

    fn resolve(&self, model: Option<&str>) -> Arc<dyn Model> {
        let resolved_default_model = get_default_model();
        let model_name = model.unwrap_or(resolved_default_model.as_str());
        let options = self.client_options();

        match (self.resolved_api(), self.responses_transport()) {
            (OpenAIApi::ChatCompletions, _) => {
                Arc::new(OpenAIChatCompletionsModel::new(model_name, options))
            }
            (OpenAIApi::Responses, OpenAIResponsesTransport::Http) => {
                Arc::new(OpenAIResponsesModel::new(model_name, options))
            }
            (OpenAIApi::Responses, OpenAIResponsesTransport::WebSocket) => {
                self.resolve_responses_ws_model(model_name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::{default_openai_base_url, default_openai_websocket_base_url};
    use crate::openai_agent_registration::{
        OPENAI_AGENT_HARNESS_ID_ENV_VAR, OPENAI_HARNESS_ID_TRACE_METADATA_KEY,
        OpenAIAgentRegistrationConfig, get_default_openai_agent_registration,
        set_default_openai_agent_registration, set_default_openai_harness,
    };
    use crate::{
        get_default_openai_websocket_base_url, get_openai_base_url, get_use_responses_by_default,
        get_use_responses_websocket_by_default, set_default_openai_websocket_base_url,
        set_openai_base_url, set_use_responses_by_default, set_use_responses_websocket_by_default,
    };
    use serde_json::json;
    use std::env;

    fn agent_registration_test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    struct DefaultHarnessReset(Option<OpenAIAgentRegistrationConfig>);

    impl Drop for DefaultHarnessReset {
        fn drop(&mut self) {
            set_default_openai_agent_registration(self.0.clone());
        }
    }

    struct EnvHarnessReset(Option<String>);

    impl Drop for EnvHarnessReset {
        fn drop(&mut self) {
            match &self.0 {
                Some(value) => unsafe { env::set_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR, value) },
                None => unsafe { env::remove_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR) },
            }
        }
    }

    #[test]
    fn resolves_defaults_into_client_options() {
        let provider = OpenAIProvider::new().with_api_key("sk-test");
        let options = provider.client_options();

        assert_eq!(options.api_key.as_deref(), Some("sk-test"));
        assert_eq!(options.base_url, default_openai_base_url());
        assert_eq!(
            options.websocket_base_url,
            default_openai_websocket_base_url()
        );
    }

    #[test]
    fn prefers_websocket_for_responses_when_requested() {
        let provider = OpenAIProvider::new()
            .with_use_responses(true)
            .with_use_responses_websocket(true);

        assert_eq!(provider.resolved_api(), OpenAIApi::Responses);
        assert_eq!(
            provider.responses_transport(),
            OpenAIResponsesTransport::WebSocket
        );
    }

    #[test]
    fn honors_shared_default_response_api_preference() {
        set_use_responses_by_default(false);

        let provider = OpenAIProvider::new();

        assert_eq!(get_use_responses_by_default(), Some(false));
        assert_eq!(provider.resolved_api(), OpenAIApi::ChatCompletions);

        set_use_responses_by_default(true);
    }

    #[test]
    fn explicit_api_override_wins_over_shared_default() {
        set_use_responses_by_default(true);

        let chat_provider = OpenAIProvider::new().with_api(OpenAIApi::ChatCompletions);
        assert_eq!(chat_provider.resolved_api(), OpenAIApi::ChatCompletions);

        set_use_responses_by_default(false);

        let responses_provider = OpenAIProvider::new().with_api(OpenAIApi::Responses);
        assert_eq!(responses_provider.resolved_api(), OpenAIApi::Responses);

        set_use_responses_by_default(true);
    }

    #[test]
    fn honors_shared_default_websocket_transport_preference() {
        set_use_responses_websocket_by_default(true);

        let provider = OpenAIProvider::new().with_use_responses(true);

        assert_eq!(get_use_responses_websocket_by_default(), Some(true));
        assert_eq!(
            provider.responses_transport(),
            OpenAIResponsesTransport::WebSocket
        );

        set_use_responses_websocket_by_default(false);
    }

    #[test]
    fn explicit_transport_override_wins_over_shared_default() {
        set_use_responses_websocket_by_default(true);

        let http_provider = OpenAIProvider::new()
            .with_use_responses(true)
            .with_use_responses_websocket(false);
        assert_eq!(
            http_provider.responses_transport(),
            OpenAIResponsesTransport::Http
        );

        set_use_responses_websocket_by_default(false);

        let websocket_provider = OpenAIProvider::new()
            .with_use_responses(true)
            .with_use_responses_websocket(true);
        assert_eq!(
            websocket_provider.responses_transport(),
            OpenAIResponsesTransport::WebSocket
        );
    }

    #[test]
    fn public_websocket_flag_remains_bool_and_preserves_explicit_false_override() {
        set_use_responses_websocket_by_default(true);

        let mut provider = OpenAIProvider::new().with_use_responses(true);
        provider.use_responses_websocket = false;

        assert!(!provider.use_responses_websocket);
        assert_eq!(
            provider.responses_transport(),
            OpenAIResponsesTransport::Http
        );
    }

    #[test]
    fn client_options_honor_shared_base_url_overrides() {
        set_openai_base_url("https://shared-base.example/v1");
        set_default_openai_websocket_base_url("wss://shared-ws.example/v1");

        let provider = OpenAIProvider::new().with_api_key("sk-test");
        let options = provider.client_options();

        assert_eq!(get_openai_base_url(), "https://shared-base.example/v1");
        assert_eq!(
            get_default_openai_websocket_base_url(),
            "wss://shared-ws.example/v1"
        );
        assert_eq!(options.base_url, "https://shared-base.example/v1");
        assert_eq!(options.websocket_base_url, "wss://shared-ws.example/v1");

        set_openai_base_url(crate::defaults::OPENAI_DEFAULT_BASE_URL);
        set_default_openai_websocket_base_url(crate::defaults::OPENAI_DEFAULT_WEBSOCKET_BASE_URL);
    }

    #[test]
    fn reuses_websocket_models_for_the_same_model_name() {
        let provider = OpenAIProvider::new()
            .with_api_key("sk-test")
            .with_use_responses(true)
            .with_use_responses_websocket(true);

        let first = provider.resolve(Some("gpt-5"));
        let second = provider.resolve(Some("gpt-5"));
        let third = provider.resolve(Some("gpt-5-mini"));

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &third));
    }

    #[test]
    fn does_not_reuse_websocket_models_across_different_client_options() {
        let provider = OpenAIProvider::new()
            .with_api_key("sk-test-a")
            .with_base_url("https://api-a.example/v1")
            .with_use_responses(true)
            .with_use_responses_websocket(true);
        let first = provider.resolve(Some("gpt-5"));

        let mut provider_with_new_credentials = provider.clone();
        provider_with_new_credentials.api_key = Some("sk-test-b".to_owned());
        provider_with_new_credentials.base_url = Some("https://api-b.example/v1".to_owned());
        let second = provider_with_new_credentials.resolve(Some("gpt-5"));

        assert!(
            !Arc::ptr_eq(&first, &second),
            "websocket models should not be reused across distinct client identities"
        );
    }

    #[test]
    fn uses_shared_default_model_resolution() {
        let provider = OpenAIProvider::new()
            .with_api_key("sk-test")
            .with_use_responses(true)
            .with_use_responses_websocket(true);
        let default_model = get_default_model();
        let model = provider.resolve(None);
        let explicit = provider.resolve(Some(default_model.as_str()));

        assert!(Arc::ptr_eq(&model, &explicit));
    }

    #[test]
    fn harness_registration_metadata_follows_override_rules() {
        let _guard = agent_registration_test_lock().lock().expect("test lock");
        let _default_reset = DefaultHarnessReset(get_default_openai_agent_registration());
        let _env_reset = EnvHarnessReset(env::var(OPENAI_AGENT_HARNESS_ID_ENV_VAR).ok());

        unsafe { env::set_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR, "env-harness") };
        set_default_openai_harness(Some("default-harness"));

        let default_provider = OpenAIProvider::new();
        assert_eq!(
            default_provider.effective_harness_id().as_deref(),
            Some("default-harness")
        );
        assert_eq!(
            default_provider
                .resolve_trace_metadata(None, None)
                .and_then(|metadata| metadata.get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY).cloned()),
            Some(json!("default-harness"))
        );
        assert_eq!(
            default_provider
                .prepare_request(ModelRequest::default())
                .settings
                .metadata
                .get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY),
            Some(&json!("default-harness"))
        );

        set_default_openai_agent_registration(None);
        let env_provider = OpenAIProvider::new();
        assert_eq!(
            env_provider.effective_harness_id().as_deref(),
            Some("env-harness")
        );

        let provider =
            OpenAIProvider::new().with_agent_registration(OpenAIAgentRegistrationConfig {
                harness_id: Some("provider-harness".to_owned()),
            });
        assert_eq!(
            provider.effective_harness_id().as_deref(),
            Some("provider-harness")
        );
        assert_eq!(
            provider
                .resolve_trace_metadata(
                    None,
                    Some(&BTreeMap::from([
                        (
                            OPENAI_HARNESS_ID_TRACE_METADATA_KEY.to_owned(),
                            json!("explicit-harness"),
                        ),
                        ("source".to_owned(), json!("test")),
                    ])),
                )
                .and_then(|metadata| metadata.get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY).cloned()),
            Some(json!("explicit-harness"))
        );
        assert_eq!(
            provider
                .prepare_request(ModelRequest {
                    settings: agents_core::ModelSettings {
                        metadata: BTreeMap::from([(
                            OPENAI_HARNESS_ID_TRACE_METADATA_KEY.to_owned(),
                            json!("explicit-harness"),
                        )]),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .settings
                .metadata
                .get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY),
            Some(&json!("explicit-harness"))
        );
    }
}
