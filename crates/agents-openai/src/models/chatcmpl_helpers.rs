use agents_core::{InputItem, ToolDefinition};
use serde_json::{Value, json};

pub struct ChatCmplHelpers;

impl ChatCmplHelpers {
    pub fn input_to_messages(items: &[InputItem]) -> Vec<Value> {
        items
            .iter()
            .flat_map(|item| match item {
                InputItem::Text { text } => vec![json!({
                    "role": "user",
                    "content": text,
                })],
                InputItem::Json { value } => input_json_to_messages(value),
            })
            .collect()
    }

    pub fn tools_to_payload(tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_json_schema.clone().unwrap_or_else(|| json!({
                            "type": "object",
                            "properties": {}
                        })),
                    }
                })
            })
            .collect()
    }
}

fn input_json_to_messages(value: &Value) -> Vec<Value> {
    if let Some(role) = value.get("role").and_then(Value::as_str) {
        return vec![json!({
            "role": role,
            "content": value.get("content").cloned().unwrap_or_else(|| json!(value.to_string())),
        })];
    }

    match value.get("type").and_then(Value::as_str) {
        Some("tool_call_output") => vec![json!({
            "role": "tool",
            "tool_call_id": value.get("call_id"),
            "content": value.get("output").cloned().unwrap_or(Value::Null),
        })],
        Some("tool_call") => vec![json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": value.get("call_id").cloned().unwrap_or_else(|| json!("")),
                "type": "function",
                "function": {
                    "name": value.get("tool_name").cloned().unwrap_or_else(|| json!("")),
                    "arguments": value.get("arguments").cloned().unwrap_or_else(|| json!({})).to_string(),
                }
            }]
        })],
        Some("reasoning") => vec![json!({
            "role": "assistant",
            "content": value.get("text").cloned().unwrap_or_else(|| json!("")),
        })],
        _ => vec![value.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_messages_and_tools() {
        let messages = ChatCmplHelpers::input_to_messages(&[
            InputItem::from("hello"),
            InputItem::Json {
                value: json!({"type":"tool_call","tool_name":"search","call_id":"call-1","arguments":{"q":"rust"}}),
            },
        ]);
        let tools = ChatCmplHelpers::tools_to_payload(&[ToolDefinition::new("search", "Search")]);

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(tools[0]["type"], "function");
    }
}
