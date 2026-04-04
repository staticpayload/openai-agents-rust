use std::sync::RwLock;

use once_cell::sync::Lazy;

pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const OPENAI_DEFAULT_WEBSOCKET_BASE_URL: &str = "wss://api.openai.com/v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenAIApi {
    ChatCompletions,
    #[default]
    Responses,
}

#[derive(Clone, Debug, Default)]
struct OpenAISettings {
    api_key: Option<String>,
    tracing_export_api_key: Option<String>,
    api: OpenAIApi,
}

static SETTINGS: Lazy<RwLock<OpenAISettings>> =
    Lazy::new(|| RwLock::new(OpenAISettings::default()));

pub fn set_default_openai_key(key: impl Into<String>) {
    SETTINGS.write().expect("openai defaults lock").api_key = Some(key.into());
}

pub fn default_openai_key() -> Option<String> {
    SETTINGS
        .read()
        .expect("openai defaults lock")
        .api_key
        .clone()
}

pub fn set_tracing_export_api_key(key: impl Into<String>) {
    SETTINGS
        .write()
        .expect("openai defaults lock")
        .tracing_export_api_key = Some(key.into());
}

pub fn tracing_export_api_key() -> Option<String> {
    SETTINGS
        .read()
        .expect("openai defaults lock")
        .tracing_export_api_key
        .clone()
}

pub fn set_default_openai_api(api: OpenAIApi) {
    SETTINGS.write().expect("openai defaults lock").api = api;
}

pub fn default_openai_api() -> OpenAIApi {
    SETTINGS.read().expect("openai defaults lock").api
}

pub fn default_openai_base_url() -> &'static str {
    OPENAI_DEFAULT_BASE_URL
}

pub fn default_openai_websocket_base_url() -> &'static str {
    OPENAI_DEFAULT_WEBSOCKET_BASE_URL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_default_endpoints() {
        assert_eq!(default_openai_base_url(), OPENAI_DEFAULT_BASE_URL);
        assert_eq!(
            default_openai_websocket_base_url(),
            OPENAI_DEFAULT_WEBSOCKET_BASE_URL
        );
    }
}
