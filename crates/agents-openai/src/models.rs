use agents_core::{
    AgentsError, InputItem, Model, ModelRequest, ModelResponse, OutputItem, Result, ToolDefinition,
    Usage,
};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::defaults::{OPENAI_DEFAULT_BASE_URL, OPENAI_DEFAULT_WEBSOCKET_BASE_URL};
use crate::websocket::ResponsesWebSocketSession;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);

#[derive(Debug)]
struct OpenAIHttpResponse {
    payload: Value,
    request_id: Option<String>,
}

/// Client options shared across OpenAI-backed transports.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAIClientOptions {
    pub api_key: Option<String>,
    pub base_url: String,
    pub websocket_base_url: String,
    pub organization: Option<String>,
    pub project: Option<String>,
}

impl OpenAIClientOptions {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            base_url: OPENAI_DEFAULT_BASE_URL.to_owned(),
            websocket_base_url: OPENAI_DEFAULT_WEBSOCKET_BASE_URL.to_owned(),
            organization: None,
            project: None,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_websocket_base_url(mut self, websocket_base_url: impl Into<String>) -> Self {
        self.websocket_base_url = websocket_base_url.into();
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

    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub fn auth_headers(&self) -> Result<HeaderMap> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or(AgentsError::ModelProviderNotConfigured)?;
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|error| AgentsError::message(error.to_string()))?,
        );
        if let Some(organization) = &self.organization {
            headers.insert(
                "OpenAI-Organization",
                HeaderValue::from_str(organization)
                    .map_err(|error| AgentsError::message(error.to_string()))?,
            );
        }
        if let Some(project) = &self.project {
            headers.insert(
                "OpenAI-Project",
                HeaderValue::from_str(project)
                    .map_err(|error| AgentsError::message(error.to_string()))?,
            );
        }
        Ok(headers)
    }
}

#[derive(Clone, Debug)]
pub struct OpenAIResponsesModel {
    model: String,
    options: OpenAIClientOptions,
}

impl OpenAIResponsesModel {
    pub fn new(model: impl Into<String>, options: OpenAIClientOptions) -> Self {
        Self {
            model: model.into(),
            options,
        }
    }

    pub fn build_payload(&self, request: &ModelRequest) -> Value {
        let mut payload = serde_json::Map::new();
        payload.insert("model".to_owned(), Value::String(self.model.clone()));
        if let Some(instructions) = &request.instructions {
            payload.insert(
                "instructions".to_owned(),
                Value::String(instructions.clone()),
            );
        }
        if let Some(previous_response_id) = &request.previous_response_id {
            payload.insert(
                "previous_response_id".to_owned(),
                Value::String(previous_response_id.clone()),
            );
        }
        if let Some(conversation_id) = &request.conversation_id {
            payload.insert(
                "conversation".to_owned(),
                Value::String(conversation_id.clone()),
            );
        }
        payload.insert(
            "input".to_owned(),
            Value::Array(
                request
                    .input
                    .iter()
                    .map(openai_response_input_item)
                    .collect(),
            ),
        );
        let tools = openai_tools_payload(&request.tools);
        if !tools.is_empty() {
            payload.insert("tools".to_owned(), Value::Array(tools));
        }
        apply_responses_model_settings(&mut payload, &request.settings);
        Value::Object(payload)
    }
}

#[async_trait]
impl Model for OpenAIResponsesModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let payload = self.build_payload(&request);
        let response = post_json(
            &self.options.api_url("/responses"),
            &self.options,
            payload,
            &request.settings,
        )
        .await?;

        Ok(parse_responses_response(
            &self.model,
            &response.payload,
            response.request_id,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct OpenAIChatCompletionsModel {
    model: String,
    options: OpenAIClientOptions,
}

impl OpenAIChatCompletionsModel {
    pub fn new(model: impl Into<String>, options: OpenAIClientOptions) -> Self {
        Self {
            model: model.into(),
            options,
        }
    }

    pub fn build_payload(&self, request: &ModelRequest) -> Value {
        let mut payload = serde_json::Map::new();
        payload.insert("model".to_owned(), Value::String(self.model.clone()));
        let mut messages = Vec::new();
        if let Some(instructions) = &request.instructions {
            messages.push(json!({
                "role": "system",
                "content": instructions,
            }));
        }
        messages.extend(request.input.iter().flat_map(openai_chat_messages));
        payload.insert("messages".to_owned(), Value::Array(messages));
        let tools = openai_tools_payload(&request.tools);
        let has_tools = !tools.is_empty();
        if !tools.is_empty() {
            payload.insert("tools".to_owned(), Value::Array(tools));
        }
        apply_chat_model_settings(&mut payload, &request.settings, has_tools);
        Value::Object(payload)
    }
}

#[async_trait]
impl Model for OpenAIChatCompletionsModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let payload = self.build_payload(&request);
        let response = post_json(
            &self.options.api_url("/chat/completions"),
            &self.options,
            payload,
            &request.settings,
        )
        .await?;

        Ok(parse_chat_completions_response(
            &self.model,
            &response.payload,
            response.request_id,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct OpenAIResponsesWsModel {
    inner: OpenAIResponsesModel,
}

impl OpenAIResponsesWsModel {
    pub fn new(model: impl Into<String>, options: OpenAIClientOptions) -> Self {
        Self {
            inner: OpenAIResponsesModel::new(model, options),
        }
    }

    pub fn websocket_session(&self) -> ResponsesWebSocketSession {
        ResponsesWebSocketSession::new(Some(self.inner.model.clone()), self.inner.options.clone())
    }
}

#[async_trait]
impl Model for OpenAIResponsesWsModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        self.inner.generate(request).await
    }
}

async fn post_json(
    url: &str,
    options: &OpenAIClientOptions,
    payload: Value,
    settings: &agents_core::ModelSettings,
) -> Result<OpenAIHttpResponse> {
    let mut headers = options.auth_headers()?;
    for (name, value) in &settings.extra_headers {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| AgentsError::message(error.to_string()))?,
            HeaderValue::from_str(&json_value_to_string(value))
                .map_err(|error| AgentsError::message(error.to_string()))?,
        );
    }
    let mut request = HTTP_CLIENT.post(url).headers(headers);
    if !settings.extra_query.is_empty() {
        let query = settings
            .extra_query
            .iter()
            .map(|(key, value)| (key.clone(), json_value_to_string(value)))
            .collect::<Vec<_>>();
        request = request.query(&query);
    }
    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

    let request_id = response
        .headers()
        .get("x-request-id")
        .or_else(|| response.headers().get("request-id"))
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;
    if !status.is_success() {
        return Err(AgentsError::message(format!(
            "openai request failed with status {}: {}",
            status, text
        )));
    }

    let payload =
        serde_json::from_str(&text).map_err(|error| AgentsError::message(error.to_string()))?;
    Ok(OpenAIHttpResponse {
        payload,
        request_id,
    })
}

fn apply_responses_model_settings(
    payload: &mut serde_json::Map<String, Value>,
    settings: &agents_core::ModelSettings,
) {
    if let Some(value) = settings.temperature {
        payload.insert("temperature".to_owned(), json!(value));
    }
    if let Some(value) = settings.top_p {
        payload.insert("top_p".to_owned(), json!(value));
    }
    if let Some(value) = settings.max_output_tokens {
        payload.insert("max_output_tokens".to_owned(), json!(value));
    }
    if let Some(value) = settings.parallel_tool_calls {
        payload.insert("parallel_tool_calls".to_owned(), json!(value));
    }
    if let Some(value) = &settings.tool_choice {
        payload.insert("tool_choice".to_owned(), Value::String(value.clone()));
    }
    if let Some(value) = &settings.truncation {
        payload.insert("truncation".to_owned(), Value::String(value.clone()));
    }
    if let Some(value) = settings.store {
        payload.insert("store".to_owned(), json!(value));
    }
    if !settings.response_include.is_empty() {
        payload.insert(
            "include".to_owned(),
            Value::Array(
                settings
                    .response_include
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if !settings.metadata.is_empty() {
        payload.insert("metadata".to_owned(), json!(settings.metadata));
    }
    if let Some(reasoning) = &settings.reasoning {
        payload.insert(
            "reasoning".to_owned(),
            json!({
                "effort": reasoning.effort,
                "summary": reasoning.summary,
            }),
        );
    }
    if let Some(verbosity) = &settings.verbosity {
        payload.insert(
            "text".to_owned(),
            json!({
                "verbosity": verbosity,
            }),
        );
    }
    for (key, value) in &settings.extra_body {
        payload.insert(key.clone(), value.clone());
    }
}

fn apply_chat_model_settings(
    payload: &mut serde_json::Map<String, Value>,
    settings: &agents_core::ModelSettings,
    has_tools: bool,
) {
    if let Some(value) = settings.temperature {
        payload.insert("temperature".to_owned(), json!(value));
    }
    if let Some(value) = settings.top_p {
        payload.insert("top_p".to_owned(), json!(value));
    }
    if let Some(value) = settings.max_output_tokens {
        payload.insert("max_tokens".to_owned(), json!(value));
    }
    if let Some(value) = settings.frequency_penalty {
        payload.insert("frequency_penalty".to_owned(), json!(value));
    }
    if let Some(value) = settings.presence_penalty {
        payload.insert("presence_penalty".to_owned(), json!(value));
    }
    if let Some(value) = settings.parallel_tool_calls {
        payload.insert("parallel_tool_calls".to_owned(), json!(value));
    }
    if let Some(value) = &settings.tool_choice {
        payload.insert("tool_choice".to_owned(), Value::String(value.clone()));
    } else if has_tools {
        payload.insert("tool_choice".to_owned(), Value::String("auto".to_owned()));
    }
    if let Some(value) = settings.top_logprobs {
        payload.insert("logprobs".to_owned(), Value::Bool(true));
        payload.insert("top_logprobs".to_owned(), json!(value));
    }
    for (key, value) in &settings.extra_body {
        payload.insert(key.clone(), value.clone());
    }
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

fn openai_response_input_item(item: &InputItem) -> Value {
    match item {
        InputItem::Text { text } => json!({
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": text,
                }
            ]
        }),
        InputItem::Json { value } => openai_response_json_item(value),
    }
}

fn openai_response_json_item(value: &Value) -> Value {
    if let Some(role) = value.get("role").and_then(Value::as_str) {
        return json!({
            "type": "message",
            "role": role,
            "content": value.get("content").cloned().unwrap_or_else(|| {
                Value::Array(vec![json!({
                    "type": "input_text",
                    "text": value.to_string(),
                })])
            }),
        });
    }

    match value.get("type").and_then(Value::as_str) {
        Some("tool_call_output") => json!({
            "type": "function_call_output",
            "call_id": first_non_empty_string(value, &["call_id", "id"]).unwrap_or_default(),
            "output": tool_output_to_responses_output(value.get("output")),
        }),
        Some("tool_call") => json!({
            "type": "function_call",
            "call_id": first_non_empty_string(value, &["call_id", "id"]).unwrap_or_default(),
            "name": first_non_empty_string(value, &["tool_name", "name"]).unwrap_or_default(),
            "arguments": serialize_json_argument(value.get("arguments").unwrap_or(&Value::Null)),
        }),
        Some("reasoning") => json!({
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "input_text",
                    "text": value
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("reasoning"),
                }
            ],
        }),
        Some("handoff_call") => json!({
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "input_text",
                    "text": format!(
                        "[handoff:{}]",
                        value.get("target_agent").and_then(Value::as_str).unwrap_or_default()
                    ),
                }
            ],
        }),
        Some("handoff_output") => json!({
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "input_text",
                    "text": format!(
                        "[handoff_complete:{}]",
                        value.get("source_agent").and_then(Value::as_str).unwrap_or_default()
                    ),
                }
            ],
        }),
        _ => value.clone(),
    }
}

fn openai_chat_messages(item: &InputItem) -> Vec<Value> {
    match item {
        InputItem::Text { text } => vec![json!({
            "role": "user",
            "content": text,
        })],
        InputItem::Json { value } => openai_chat_json_messages(value),
    }
}

fn openai_chat_json_messages(value: &Value) -> Vec<Value> {
    if let Some(role) = value.get("role").and_then(Value::as_str) {
        return vec![json!({
            "role": role,
            "content": value.get("content").cloned().unwrap_or_else(|| json!(value.to_string())),
        })];
    }

    match value.get("type").and_then(Value::as_str) {
        Some("tool_call_output") => vec![json!({
            "role": "tool",
            "tool_call_id": first_non_empty_string(value, &["call_id", "id"]),
            "content": tool_output_to_chat_content(value.get("output")),
        })],
        Some("tool_call") => vec![json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [
                {
                    "id": first_non_empty_string(value, &["call_id", "id"]).unwrap_or_default(),
                    "type": "function",
                    "function": {
                        "name": first_non_empty_string(value, &["tool_name", "name"]).unwrap_or_default(),
                        "arguments": serialize_json_argument(value.get("arguments").unwrap_or(&Value::Null)),
                    }
                }
            ],
        })],
        Some("reasoning") => vec![json!({
            "role": "assistant",
            "content": value.get("text").and_then(Value::as_str).unwrap_or("reasoning"),
        })],
        Some("handoff_call") => vec![json!({
            "role": "assistant",
            "content": format!(
                "[handoff:{}]",
                value.get("target_agent").and_then(Value::as_str).unwrap_or_default()
            ),
        })],
        Some("handoff_output") => vec![json!({
            "role": "tool",
            "content": format!(
                "[handoff_complete:{}]",
                value.get("source_agent").and_then(Value::as_str).unwrap_or_default()
            ),
        })],
        _ => vec![json!({
            "role": "user",
            "content": value.to_string(),
        })],
    }
}

fn openai_tools_payload(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(openai_tool_payload).collect()
}

fn openai_tool_payload(tool: &ToolDefinition) -> Value {
    if tool.input_json_schema.is_none() {
        return json!({
            "type": tool.name,
        });
    }

    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_json_schema.clone().unwrap_or(Value::Null),
            "strict": tool.strict_json_schema,
        }
    })
}

fn parse_responses_response(
    model: &str,
    payload: &Value,
    request_id: Option<String>,
) -> ModelResponse {
    let mut output = Vec::new();
    if let Some(items) = payload.get("output").and_then(Value::as_array) {
        for item in items {
            let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                output.push(OutputItem::Json {
                    value: item.clone(),
                });
                continue;
            };
            match item_type {
                "message" => {
                    let before = output.len();
                    if let Some(content) = item.get("content").and_then(Value::as_array) {
                        for content_item in content {
                            if let Some(text) = content_item.get("text").and_then(Value::as_str) {
                                output.push(OutputItem::Text {
                                    text: text.to_owned(),
                                });
                            }
                        }
                    }
                    if output.len() == before {
                        output.push(OutputItem::Json {
                            value: item.clone(),
                        });
                    }
                }
                "function_call" => {
                    let tool_name = first_non_empty_string(item, &["name", "tool_name"]);
                    if let Some(tool_name) = tool_name {
                        output.push(OutputItem::ToolCall {
                            call_id: first_non_empty_string(item, &["call_id", "id"])
                                .unwrap_or_default(),
                            tool_name,
                            arguments: parse_json_maybe_string(item.get("arguments")),
                            namespace: optional_string_field(item, "namespace"),
                        });
                    } else {
                        output.push(OutputItem::Json {
                            value: item.clone(),
                        });
                    }
                }
                "reasoning" => {
                    let reasoning_text = item
                        .get("summary")
                        .and_then(Value::as_array)
                        .and_then(|summary| summary.first())
                        .and_then(|entry| entry.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or("reasoning");
                    output.push(OutputItem::Reasoning {
                        text: reasoning_text.to_owned(),
                    });
                }
                _ => output.push(OutputItem::Json {
                    value: item.clone(),
                }),
            }
        }
    }
    if output.is_empty() {
        if let Some(text) = payload.get("output_text").and_then(Value::as_str) {
            output.push(OutputItem::Text {
                text: text.to_owned(),
            });
        }
    }

    ModelResponse {
        model: Some(model.to_owned()),
        output,
        usage: Usage {
            input_tokens: payload
                .get("usage")
                .and_then(|usage| usage.get("input_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
            output_tokens: payload
                .get("usage")
                .and_then(|usage| usage.get("output_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
        },
        response_id: optional_string_field(payload, "id"),
        request_id,
    }
}

fn parse_chat_completions_response(
    model: &str,
    payload: &Value,
    request_id: Option<String>,
) -> ModelResponse {
    let mut output = Vec::new();
    if let Some(message) = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
    {
        if let Some(content) = parse_chat_message_content(message.get("content")) {
            output.push(OutputItem::Text { text: content });
        }
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let function = tool_call.get("function").unwrap_or(&Value::Null);
                output.push(OutputItem::ToolCall {
                    call_id: first_non_empty_string(tool_call, &["id"]).unwrap_or_default(),
                    tool_name: first_non_empty_string(function, &["name"]).unwrap_or_default(),
                    arguments: parse_json_maybe_string(function.get("arguments")),
                    namespace: None,
                });
            }
        }
        if output.is_empty() {
            output.push(OutputItem::Json {
                value: message.clone(),
            });
        }
    }

    ModelResponse {
        model: Some(model.to_owned()),
        output,
        usage: Usage {
            input_tokens: payload
                .get("usage")
                .and_then(|usage| usage.get("prompt_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
            output_tokens: payload
                .get("usage")
                .and_then(|usage| usage.get("completion_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
        },
        response_id: None,
        request_id,
    }
}

fn first_non_empty_string(value: &Value, fields: &[&str]) -> Option<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str))
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn parse_json_maybe_string(value: Option<&Value>) -> Value {
    match value {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_else(|_| json!(text)),
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

fn serialize_json_argument(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_owned()),
    }
}

fn parse_chat_message_content(content: Option<&Value>) -> Option<String> {
    match content {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(items)) => {
            let text = items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() { None } else { Some(text) }
        }
        Some(Value::Null) | None => None,
        Some(other) => Some(other.to_string()),
    }
}

fn tool_output_to_responses_output(value: Option<&Value>) -> Value {
    match value {
        Some(Value::Object(map)) => match map.get("type").and_then(Value::as_str) {
            Some("text") => map
                .get("text")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
            Some("json") => map.get("value").cloned().unwrap_or(Value::Null),
            _ => Value::String(Value::Object(map.clone()).to_string()),
        },
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

fn tool_output_to_chat_content(value: Option<&Value>) -> String {
    match tool_output_to_responses_output(value) {
        Value::String(text) => text,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn responses_payload_includes_tools_and_input() {
        let model = OpenAIResponsesModel::new(
            "gpt-5",
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        );
        let payload = model.build_payload(&ModelRequest {
            model: Some("gpt-5".to_owned()),
            instructions: Some("Be precise".to_owned()),
            previous_response_id: None,
            conversation_id: None,
            settings: agents_core::ModelSettings {
                temperature: Some(0.3),
                max_output_tokens: Some(256),
                store: Some(true),
                tool_choice: Some("required".to_owned()),
                response_include: vec!["reasoning".to_owned()],
                extra_body: std::collections::BTreeMap::from([(
                    "service_tier".to_owned(),
                    json!("priority"),
                )]),
                ..Default::default()
            },
            input: vec![InputItem::from("hello")],
            tools: vec![
                ToolDefinition::new("search", "Search").with_input_json_schema(json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                })),
            ],
            trace_id: None,
        });

        assert_eq!(payload["model"], "gpt-5");
        assert_eq!(payload["input"][0]["role"], "user");
        assert_eq!(payload["tools"][0]["type"], "function");
        assert!((payload["temperature"].as_f64().unwrap_or_default() - 0.3).abs() < 0.000_1);
        assert_eq!(payload["max_output_tokens"], 256);
        assert_eq!(payload["store"], true);
        assert_eq!(payload["tool_choice"], "required");
        assert_eq!(payload["include"][0], "reasoning");
        assert_eq!(payload["service_tier"], "priority");
    }

    #[test]
    fn chat_payload_includes_tool_choice() {
        let model = OpenAIChatCompletionsModel::new(
            "gpt-4.1",
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        );
        let payload = model.build_payload(&ModelRequest {
            model: Some("gpt-4.1".to_owned()),
            instructions: Some("Be brief".to_owned()),
            previous_response_id: None,
            conversation_id: None,
            settings: agents_core::ModelSettings {
                frequency_penalty: Some(0.4),
                presence_penalty: Some(0.2),
                parallel_tool_calls: Some(true),
                top_logprobs: Some(3),
                ..Default::default()
            },
            input: vec![
                InputItem::from("hello"),
                InputItem::Json {
                    value: json!({
                        "type": "tool_call",
                        "tool_name": "search",
                        "call_id": "call-1",
                        "arguments": {"query": "rust"},
                    }),
                },
            ],
            tools: vec![
                ToolDefinition::new("search", "Search").with_input_json_schema(json!({
                    "type": "object"
                })),
            ],
            trace_id: None,
        });

        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][1]["content"], "hello");
        assert_eq!(
            payload["messages"][2]["tool_calls"][0]["function"]["name"],
            "search"
        );
        assert!((payload["frequency_penalty"].as_f64().unwrap_or_default() - 0.4).abs() < 0.000_1);
        assert!((payload["presence_penalty"].as_f64().unwrap_or_default() - 0.2).abs() < 0.000_1);
        assert_eq!(payload["parallel_tool_calls"], true);
        assert_eq!(payload["top_logprobs"], 3);
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[test]
    fn responses_payload_converts_tool_outputs() {
        let model = OpenAIResponsesModel::new(
            "gpt-5",
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        );
        let payload = model.build_payload(&ModelRequest {
            model: Some("gpt-5".to_owned()),
            instructions: None,
            previous_response_id: None,
            conversation_id: None,
            settings: Default::default(),
            input: vec![InputItem::Json {
                value: json!({
                    "type": "tool_call_output",
                    "call_id": "call-1",
                    "output": {
                        "type": "text",
                        "text": "found it"
                    }
                }),
            }],
            tools: Vec::new(),
            trace_id: None,
        });

        assert_eq!(payload["input"][0]["type"], "function_call_output");
        assert_eq!(payload["input"][0]["output"], "found it");
    }

    #[test]
    fn responses_payload_carries_conversation_tracking_fields() {
        let model = OpenAIResponsesModel::new(
            "gpt-5",
            OpenAIClientOptions::new(Some("sk-test".to_owned())),
        );
        let payload = model.build_payload(&ModelRequest {
            model: Some("gpt-5".to_owned()),
            instructions: None,
            previous_response_id: Some("resp_123".to_owned()),
            conversation_id: Some("conv_123".to_owned()),
            settings: Default::default(),
            input: vec![InputItem::from("hello")],
            tools: Vec::new(),
            trace_id: None,
        });

        assert_eq!(payload["previous_response_id"], "resp_123");
        assert_eq!(payload["conversation"], "conv_123");
    }

    #[test]
    fn parses_responses_tool_call_and_text() {
        let parsed = parse_responses_response(
            "gpt-5",
            &json!({
                "output": [
                    {
                        "type": "reasoning",
                        "summary": [{"text": "thinking"}]
                    },
                    {
                        "type": "function_call",
                        "call_id": "call-1",
                        "name": "search",
                        "arguments": "{\"query\":\"rust\"}"
                    },
                    {
                        "type": "message",
                        "content": [{"type": "output_text", "text": "done"}]
                    }
                ],
                "usage": {"input_tokens": 3, "output_tokens": 4},
                "id": "resp_123"
            }),
            Some("req_123".to_owned()),
        );

        assert_eq!(parsed.usage.input_tokens, 3);
        assert_eq!(parsed.usage.output_tokens, 4);
        assert_eq!(parsed.response_id.as_deref(), Some("resp_123"));
        assert_eq!(parsed.request_id.as_deref(), Some("req_123"));
        assert!(matches!(parsed.output[0], OutputItem::Reasoning { .. }));
        assert!(matches!(parsed.output[1], OutputItem::ToolCall { .. }));
        assert!(matches!(parsed.output[2], OutputItem::Text { .. }));
    }

    #[test]
    fn parses_chat_completions_tool_calls() {
        let parsed = parse_chat_completions_response(
            "gpt-4.1",
            &json!({
                "choices": [{
                    "message": {
                        "content": "done",
                        "tool_calls": [{
                            "id": "call-1",
                            "function": {
                                "name": "search",
                                "arguments": "{\"query\":\"rust\"}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 5, "completion_tokens": 6}
            }),
            Some("req_chat_123".to_owned()),
        );

        assert_eq!(parsed.usage.input_tokens, 5);
        assert_eq!(parsed.usage.output_tokens, 6);
        assert_eq!(parsed.response_id, None);
        assert_eq!(parsed.request_id.as_deref(), Some("req_chat_123"));
        assert!(matches!(parsed.output[0], OutputItem::Text { .. }));
        assert!(matches!(parsed.output[1], OutputItem::ToolCall { .. }));
    }

    #[test]
    fn preserves_unknown_responses_output_items_as_json() {
        let parsed = parse_responses_response(
            "gpt-5",
            &json!({
                "output": [
                    {
                        "type": "web_search_call",
                        "id": "ws_123"
                    }
                ]
            }),
            None,
        );

        assert!(matches!(parsed.output[0], OutputItem::Json { .. }));
    }
}
