use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::Result;
use crate::items::{InputItem, OutputItem};
use crate::model_settings::ModelSettings;
use crate::tool::ToolDefinition;
use crate::usage::Usage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelTracing {
    Disabled,
    #[default]
    Enabled,
    EnabledWithoutData,
}

impl ModelTracing {
    pub fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn include_data(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Model request shared across providers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModelRequest {
    pub trace_id: Option<Uuid>,
    pub model: Option<String>,
    pub instructions: Option<String>,
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
    pub settings: ModelSettings,
    pub input: Vec<InputItem>,
    pub tools: Vec<ToolDefinition>,
}

impl ModelRequest {
    pub fn effective_model_name(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

/// Model response returned by providers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModelResponse {
    pub model: Option<String>,
    pub output: Vec<OutputItem>,
    pub usage: Usage,
    pub response_id: Option<String>,
    pub request_id: Option<String>,
}

impl ModelResponse {
    pub fn to_input_items(&self) -> Vec<InputItem> {
        self.output
            .iter()
            .map(|item| match item {
                OutputItem::Text { text } => InputItem::Text { text: text.clone() },
                OutputItem::Json { value } => InputItem::Json {
                    value: value.clone(),
                },
                OutputItem::ToolCall {
                    call_id,
                    tool_name,
                    arguments,
                    namespace,
                } => InputItem::Json {
                    value: serde_json::json!({
                        "type": "tool_call",
                        "call_id": call_id,
                        "tool_name": tool_name,
                        "arguments": arguments,
                        "namespace": namespace,
                    }),
                },
                OutputItem::Handoff { target_agent } => InputItem::Json {
                    value: serde_json::json!({
                        "type": "handoff_call",
                        "target_agent": target_agent,
                    }),
                },
                OutputItem::Reasoning { text } => InputItem::Json {
                    value: serde_json::json!({
                        "type": "reasoning",
                        "text": text,
                    }),
                },
            })
            .collect()
    }
}

#[async_trait]
pub trait Model: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse>;
}

/// Resolves models by name.
pub trait ModelProvider: Send + Sync {
    fn resolve(&self, model: Option<&str>) -> Arc<dyn Model>;

    fn resolve_with_settings(
        &self,
        model: Option<&str>,
        _settings: &ModelSettings,
    ) -> Arc<dyn Model> {
        self.resolve(model)
    }
}
