use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::Result;
use crate::items::{InputItem, OutputItem};
use crate::tool::ToolDefinition;
use crate::usage::Usage;

/// Model request shared across providers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModelRequest {
    pub trace_id: Option<Uuid>,
    pub model: Option<String>,
    pub instructions: Option<String>,
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
    pub input: Vec<InputItem>,
    pub tools: Vec<ToolDefinition>,
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
}
