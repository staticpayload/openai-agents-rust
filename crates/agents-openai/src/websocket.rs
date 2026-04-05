use agents_core::{AgentsError, ModelRequest, Result};
use reqwest::header::HeaderMap;
use serde_json::Value;
use url::Url;

use crate::models::{OpenAIClientOptions, OpenAIResponsesModel};

/// Shared helper for constructing Responses websocket requests.
#[derive(Clone, Debug, Default)]
pub struct ResponsesWebSocketSession {
    pub model: Option<String>,
    pub response_id: Option<String>,
    pub client_options: OpenAIClientOptions,
}

pub fn responses_websocket_session(
    model: impl Into<String>,
    client_options: OpenAIClientOptions,
) -> ResponsesWebSocketSession {
    ResponsesWebSocketSession::new(Some(model.into()), client_options)
}

impl ResponsesWebSocketSession {
    pub fn new(model: Option<String>, client_options: OpenAIClientOptions) -> Self {
        Self {
            model,
            response_id: None,
            client_options,
        }
    }

    pub fn with_response_id(mut self, response_id: impl Into<String>) -> Self {
        self.response_id = Some(response_id.into());
        self
    }

    pub fn websocket_url(&self) -> Result<String> {
        self.websocket_url_with_query(std::iter::empty::<(String, String)>())
    }

    pub fn websocket_url_with_query<I, K, V>(&self, query: I) -> Result<String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let base = self.client_options.websocket_base_url.trim_end_matches('/');
        let normalized = if base.starts_with("ws://") || base.starts_with("wss://") {
            base.to_owned()
        } else if let Some(stripped) = base.strip_prefix("https://") {
            format!("wss://{stripped}")
        } else if let Some(stripped) = base.strip_prefix("http://") {
            format!("ws://{stripped}")
        } else {
            return Err(AgentsError::message(format!(
                "unsupported websocket base url `{}`",
                self.client_options.websocket_base_url
            )));
        };

        let mut url =
            Url::parse(&normalized).map_err(|error| AgentsError::message(error.to_string()))?;
        let path = format!("{}/responses", url.path().trim_end_matches('/'));
        url.set_path(&path);
        let query = query
            .into_iter()
            .map(|(key, value)| (key.as_ref().to_owned(), value.as_ref().to_owned()))
            .collect::<Vec<_>>();
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key.as_ref(), value.as_ref());
            }
        }

        Ok(url.to_string())
    }

    pub fn headers(&self) -> Result<HeaderMap> {
        self.client_options.auth_headers()
    }

    pub fn request_payload(&self, request: &ModelRequest) -> Value {
        let model = OpenAIResponsesModel::new(
            self.model.clone().unwrap_or_else(|| "gpt-5".to_owned()),
            self.client_options.clone(),
        );
        let mut payload = model.build_payload(request);
        if let (Some(response_id), Value::Object(payload_object)) =
            (&self.response_id, &mut payload)
        {
            payload_object.insert(
                "previous_response_id".to_owned(),
                Value::String(response_id.clone()),
            );
        }
        payload
    }

    pub fn request_frame(&self, request: &ModelRequest) -> Value {
        let mut payload = self.request_payload(request);
        if let Value::Object(ref mut payload_object) = payload {
            payload_object.insert(
                "type".to_owned(),
                Value::String("response.create".to_owned()),
            );
            payload_object.insert("stream".to_owned(), Value::Bool(true));
        }
        payload
    }
}

#[cfg(test)]
mod tests {
    use agents_core::{InputItem, ModelRequest};
    use serde_json::json;

    use super::*;

    #[test]
    fn builds_websocket_url_from_https_base() {
        let session = ResponsesWebSocketSession::new(
            Some("gpt-5".to_owned()),
            OpenAIClientOptions {
                api_key: Some("sk-test".to_owned()),
                base_url: "https://api.openai.com/v1".to_owned(),
                websocket_base_url: "https://api.openai.com/v1".to_owned(),
                organization: None,
                project: None,
            },
        );

        assert_eq!(
            session.websocket_url().expect("url should build"),
            "wss://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn builds_request_payload() {
        let session = ResponsesWebSocketSession::new(
            Some("gpt-5".to_owned()),
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        )
        .with_response_id("resp_123");
        let payload = session.request_frame(&ModelRequest {
            trace_id: None,
            model: Some("gpt-5".to_owned()),
            instructions: Some("Be precise".to_owned()),
            previous_response_id: None,
            conversation_id: None,
            settings: Default::default(),
            input: vec![
                InputItem::from("hello"),
                InputItem::Json {
                    value: json!({"type":"tool_call_output"}),
                },
            ],
            tools: Vec::new(),
        });

        assert_eq!(payload["model"], "gpt-5");
        assert_eq!(payload["type"], "response.create");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["previous_response_id"], "resp_123");
        assert_eq!(
            payload["input"].as_array().map(|items| items.len()),
            Some(2)
        );
    }

    #[test]
    fn appends_websocket_query_parameters() {
        let session = ResponsesWebSocketSession::new(
            Some("gpt-5".to_owned()),
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        );

        let url = session
            .websocket_url_with_query([("foo", "bar"), ("baz", "1")])
            .expect("url should build");

        assert!(url.contains("foo=bar"));
        assert!(url.contains("baz=1"));
    }
}
