use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub type TResponseInputItem = InputItem;

/// Input items passed into a run.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    Text { text: String },
    Json { value: Value },
}

impl PartialEq for InputItem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text { text: left, .. }, Self::Text { text: right, .. }) => left == right,
            (Self::Json { value: left, .. }, Self::Json { value: right, .. }) => left == right,
            _ => false,
        }
    }
}

impl InputItem {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            Self::Json { .. } => None,
        }
    }
}

impl From<&str> for InputItem {
    fn from(value: &str) -> Self {
        Self::Text {
            text: value.to_owned(),
        }
    }
}

impl From<String> for InputItem {
    fn from(value: String) -> Self {
        Self::Text { text: value }
    }
}

/// Output items emitted by a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Text {
        text: String,
    },
    Json {
        value: Value,
    },
    ToolCall {
        call_id: String,
        tool_name: String,
        arguments: Value,
        namespace: Option<String>,
    },
    Handoff {
        target_agent: String,
    },
    Reasoning {
        text: String,
    },
}

impl OutputItem {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            Self::Json { .. }
            | Self::ToolCall { .. }
            | Self::Handoff { .. }
            | Self::Reasoning { .. } => None,
        }
    }
}

/// Replayable run items produced while executing an agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunItem {
    MessageOutput {
        content: OutputItem,
    },
    ToolCall {
        tool_name: String,
        arguments: Value,
        call_id: Option<String>,
        namespace: Option<String>,
    },
    ToolCallOutput {
        tool_name: String,
        output: OutputItem,
        call_id: Option<String>,
        namespace: Option<String>,
    },
    HandoffCall {
        target_agent: String,
    },
    HandoffOutput {
        source_agent: String,
    },
    Reasoning {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompactionItem {
    pub raw_item: InputItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MessageOutputItem {
    pub raw_item: OutputItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningItem {
    pub raw_item: RunItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallItem {
    pub raw_item: RunItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallOutputItem {
    pub raw_item: RunItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffCallItem {
    pub raw_item: RunItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffOutputItem {
    pub raw_item: RunItem,
    pub source_agent: Option<String>,
    pub target_agent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MCPApprovalRequestItem {
    pub raw_item: InputItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MCPApprovalResponseItem {
    pub raw_item: InputItem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolApprovalItem {
    pub raw_item: InputItem,
}

pub struct ItemHelpers;

impl ItemHelpers {
    pub fn to_input_item(item: &RunItem) -> Option<InputItem> {
        item.to_input_item()
    }

    pub fn to_input_items(items: &[RunItem]) -> Vec<InputItem> {
        items.iter().filter_map(Self::to_input_item).collect()
    }

    pub fn extract_text(item: &OutputItem) -> Option<&str> {
        item.as_text()
    }

    pub fn is_tool_call(item: &RunItem) -> bool {
        matches!(item, RunItem::ToolCall { .. })
    }
}

impl RunItem {
    pub fn to_input_item(&self) -> Option<InputItem> {
        match self {
            Self::MessageOutput { content } => match content {
                OutputItem::Text { text } => Some(InputItem::Text { text: text.clone() }),
                OutputItem::Json { value } => Some(InputItem::Json {
                    value: value.clone(),
                }),
                OutputItem::Reasoning { text } => Some(InputItem::Json {
                    value: json!({
                        "type": "reasoning",
                        "text": text,
                    }),
                }),
                OutputItem::ToolCall {
                    call_id,
                    tool_name,
                    arguments,
                    namespace,
                } => Some(InputItem::Json {
                    value: json!({
                        "type": "tool_call",
                        "call_id": call_id,
                        "tool_name": tool_name,
                        "arguments": arguments,
                        "namespace": namespace,
                    }),
                }),
                OutputItem::Handoff { target_agent } => Some(InputItem::Json {
                    value: json!({
                        "type": "handoff_call",
                        "target_agent": target_agent,
                    }),
                }),
            },
            Self::ToolCall {
                tool_name,
                arguments,
                call_id,
                namespace,
            } => Some(InputItem::Json {
                value: json!({
                    "type": "tool_call",
                    "tool_name": tool_name,
                    "arguments": arguments,
                    "call_id": call_id,
                    "namespace": namespace,
                }),
            }),
            Self::ToolCallOutput {
                tool_name,
                output,
                call_id,
                namespace,
            } => Some(InputItem::Json {
                value: json!({
                    "type": "tool_call_output",
                    "tool_name": tool_name,
                    "output": serde_json::to_value(output).ok(),
                    "call_id": call_id,
                    "namespace": namespace,
                }),
            }),
            Self::HandoffCall { target_agent } => Some(InputItem::Json {
                value: json!({
                    "type": "handoff_call",
                    "target_agent": target_agent,
                }),
            }),
            Self::HandoffOutput { source_agent } => Some(InputItem::Json {
                value: json!({
                    "type": "handoff_output",
                    "source_agent": source_agent,
                }),
            }),
            Self::Reasoning { text } => Some(InputItem::Json {
                value: json!({
                    "type": "reasoning",
                    "text": text,
                }),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn converts_tool_call_output_to_structured_input() {
        let item = RunItem::ToolCallOutput {
            tool_name: "search".to_owned(),
            output: OutputItem::Text {
                text: "result".to_owned(),
            },
            call_id: Some("call-1".to_owned()),
            namespace: Some("knowledge".to_owned()),
        };

        let input = item.to_input_item().expect("tool output should convert");

        assert_eq!(
            input,
            InputItem::Json {
                value: json!({
                    "type": "tool_call_output",
                    "tool_name": "search",
                    "output": {
                        "type": "text",
                        "text": "result"
                    },
                    "call_id": "call-1",
                    "namespace": "knowledge"
                })
            }
        );
    }
}
