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
}

#[async_trait]
pub trait Model: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse>;
}

/// Resolves models by name.
pub trait ModelProvider: Send + Sync {
    fn resolve(&self, model: Option<&str>) -> Arc<dyn Model>;
}
