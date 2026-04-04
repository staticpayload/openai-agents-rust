use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpanData {
    Agent(AgentSpanData),
    Function(FunctionSpanData),
    Generation(GenerationSpanData),
    Response(ResponseSpanData),
    Handoff(HandoffSpanData),
    Custom(CustomSpanData),
    Guardrail(GuardrailSpanData),
    MpcListTools(MCPListToolsSpanData),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentSpanData {
    pub name: String,
    pub handoffs: Option<Vec<String>>,
    pub tools: Option<Vec<String>>,
    pub output_type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FunctionSpanData {
    pub name: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub mcp_data: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GenerationSpanData {
    pub input: Option<Vec<Value>>,
    pub output: Option<Vec<Value>>,
    pub model: Option<String>,
    pub model_config: Option<BTreeMap<String, Value>>,
    pub usage: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResponseSpanData {
    pub response_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffSpanData {
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CustomSpanData {
    pub name: String,
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GuardrailSpanData {
    pub name: String,
    pub triggered: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MCPListToolsSpanData {
    pub server: String,
    #[serde(default)]
    pub tools: Vec<String>,
}
