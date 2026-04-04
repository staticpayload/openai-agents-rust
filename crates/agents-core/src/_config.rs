use std::sync::{OnceLock, RwLock};

/// Default OpenAI API selection used by top-level helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultOpenAIApi {
    ChatCompletions,
    #[default]
    Responses,
}

/// Default transport selection used by Responses helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultOpenAIResponsesTransport {
    #[default]
    Http,
    Websocket,
}

#[derive(Clone, Debug, Default)]
struct OpenAICompatibilityConfig {
    api_key: Option<String>,
    tracing_export_api_key: Option<String>,
    api: DefaultOpenAIApi,
    responses_transport: DefaultOpenAIResponsesTransport,
}

fn config() -> &'static RwLock<OpenAICompatibilityConfig> {
    static CONFIG: OnceLock<RwLock<OpenAICompatibilityConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| RwLock::new(OpenAICompatibilityConfig::default()))
}

pub fn set_default_openai_key(key: impl Into<String>, use_for_tracing: bool) {
    let key = key.into();
    let mut config = config().write().expect("openai compatibility config");
    config.api_key = Some(key.clone());
    if use_for_tracing {
        config.tracing_export_api_key = Some(key);
    }
}

pub fn default_openai_key() -> Option<String> {
    config()
        .read()
        .expect("openai compatibility config")
        .api_key
        .clone()
}

pub fn set_default_tracing_export_api_key(key: impl Into<String>) {
    config()
        .write()
        .expect("openai compatibility config")
        .tracing_export_api_key = Some(key.into());
}

pub fn default_tracing_export_api_key() -> Option<String> {
    config()
        .read()
        .expect("openai compatibility config")
        .tracing_export_api_key
        .clone()
}

pub fn set_default_openai_api(api: DefaultOpenAIApi) {
    config().write().expect("openai compatibility config").api = api;
}

pub fn default_openai_api() -> DefaultOpenAIApi {
    config().read().expect("openai compatibility config").api
}

pub fn set_default_openai_responses_transport(transport: DefaultOpenAIResponsesTransport) {
    config()
        .write()
        .expect("openai compatibility config")
        .responses_transport = transport;
}

pub fn default_openai_responses_transport() -> DefaultOpenAIResponsesTransport {
    config()
        .read()
        .expect("openai compatibility config")
        .responses_transport
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_default_openai_state() {
        set_default_openai_key("test-key", true);
        set_default_openai_api(DefaultOpenAIApi::ChatCompletions);
        set_default_openai_responses_transport(DefaultOpenAIResponsesTransport::Websocket);

        assert_eq!(default_openai_key().as_deref(), Some("test-key"));
        assert_eq!(
            default_tracing_export_api_key().as_deref(),
            Some("test-key")
        );
        assert_eq!(default_openai_api(), DefaultOpenAIApi::ChatCompletions);
        assert_eq!(
            default_openai_responses_transport(),
            DefaultOpenAIResponsesTransport::Websocket
        );
    }
}
