use std::collections::BTreeSet;

use agents_core::{CallModelData, InputItem, ModelInputData, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Trims bulky tool outputs from older model turns while preserving recent turns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutputTrimmer {
    pub recent_turns: usize,
    pub max_output_chars: usize,
    pub preview_chars: usize,
    pub trimmable_tools: Option<BTreeSet<String>>,
}

impl Default for ToolOutputTrimmer {
    fn default() -> Self {
        Self {
            recent_turns: 2,
            max_output_chars: 500,
            preview_chars: 200,
            trimmable_tools: None,
        }
    }
}

impl ToolOutputTrimmer {
    pub fn validate(&self) -> Result<()> {
        if self.recent_turns < 1 {
            return Err(agents_core::AgentsError::message(format!(
                "recent_turns must be >= 1, got {}",
                self.recent_turns
            )));
        }
        if self.max_output_chars < 1 {
            return Err(agents_core::AgentsError::message(format!(
                "max_output_chars must be >= 1, got {}",
                self.max_output_chars
            )));
        }
        Ok(())
    }

    pub fn apply<TContext: Clone>(&self, data: &CallModelData<TContext>) -> Result<ModelInputData> {
        self.validate()?;
        let items = &data.model_data.input;
        if items.is_empty() {
            return Ok(data.model_data.clone());
        }

        let boundary = self.find_recent_boundary(items);
        if boundary == 0 {
            return Ok(data.model_data.clone());
        }

        let call_id_to_names = self.build_call_id_to_names(items);
        let mut new_items = Vec::with_capacity(items.len());

        for (index, item) in items.iter().enumerate() {
            if index < boundary {
                if let Some(trimmed) = self.trim_item(item, &call_id_to_names) {
                    new_items.push(trimmed);
                    continue;
                }
            }
            new_items.push(item.clone());
        }

        Ok(ModelInputData {
            input: new_items,
            instructions: data.model_data.instructions.clone(),
        })
    }

    fn find_recent_boundary(&self, items: &[InputItem]) -> usize {
        let mut user_message_count = 0;
        for index in (0..items.len()).rev() {
            let item = &items[index];
            let is_user_message = match item {
                InputItem::Text { .. } => true,
                InputItem::Json { value } => {
                    value.get("role").and_then(Value::as_str) == Some("user")
                }
            };

            if is_user_message {
                user_message_count += 1;
                if user_message_count >= self.recent_turns {
                    return index;
                }
            }
        }
        0
    }

    fn build_call_id_to_names(
        &self,
        items: &[InputItem],
    ) -> std::collections::BTreeMap<String, Vec<String>> {
        let mut mapping = std::collections::BTreeMap::new();
        for item in items {
            let InputItem::Json { value } = item else {
                continue;
            };
            match value.get("type").and_then(Value::as_str) {
                Some("function_call") | Some("tool_call") => {
                    let call_id = value
                        .get("call_id")
                        .or_else(|| value.get("id"))
                        .and_then(Value::as_str);
                    let Some(call_id) = call_id else {
                        continue;
                    };

                    let names = extract_tool_names(value);
                    if !names.is_empty() {
                        mapping.insert(call_id.to_owned(), names);
                    }
                }
                Some("tool_search_call") => {
                    if let Some(call_id) = value
                        .get("call_id")
                        .or_else(|| value.get("id"))
                        .and_then(Value::as_str)
                    {
                        mapping.insert(call_id.to_owned(), vec!["tool_search".to_owned()]);
                    }
                }
                _ => {}
            }
        }
        mapping
    }

    fn trim_item(
        &self,
        item: &InputItem,
        call_id_to_names: &std::collections::BTreeMap<String, Vec<String>>,
    ) -> Option<InputItem> {
        let InputItem::Json { value } = item else {
            return None;
        };

        let item_type = value.get("type").and_then(Value::as_str)?;
        let call_id = value
            .get("call_id")
            .or_else(|| value.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let tool_names = call_id_to_names.get(call_id).cloned().unwrap_or_else(|| {
            if item_type == "tool_search_output" {
                vec!["tool_search".to_owned()]
            } else {
                Vec::new()
            }
        });

        if let Some(allowlist) = &self.trimmable_tools {
            if !tool_names.iter().any(|name| allowlist.contains(name)) {
                return None;
            }
        }

        match item_type {
            "function_call_output" | "tool_call_output" => {
                self.trim_function_call_output(value, &tool_names)
            }
            "tool_search_output" => self.trim_tool_search_output(value),
            _ => None,
        }
    }

    fn trim_function_call_output(&self, item: &Value, tool_names: &[String]) -> Option<InputItem> {
        let output = item.get("output").cloned().unwrap_or(Value::Null);
        let output_str = self.serialize_json_like(&output);
        if output_str.len() <= self.max_output_chars {
            return None;
        }

        let tool_name = tool_names
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown_tool".to_owned());
        let preview = output_str
            .chars()
            .take(self.preview_chars)
            .collect::<String>();
        let summary = format!(
            "[Trimmed: {tool_name} output - {} chars -> {} char preview]\n{preview}...",
            output_str.len(),
            self.preview_chars
        );
        if summary.len() >= output_str.len() {
            return None;
        }

        let mut trimmed = item.as_object()?.clone();
        trimmed.insert("output".to_owned(), Value::String(summary));
        Some(InputItem::Json {
            value: Value::Object(trimmed),
        })
    }

    fn trim_tool_search_output(&self, item: &Value) -> Option<InputItem> {
        let mut trimmed = item.as_object()?.clone();
        if let Some(results) = item.get("results") {
            let serialized = self.serialize_json_like(results);
            if serialized.len() <= self.max_output_chars {
                return None;
            }
            let preview = serialized
                .chars()
                .take(self.preview_chars)
                .collect::<String>();
            trimmed.insert("results".to_owned(), json!([{ "text": preview }]));
            return Some(InputItem::Json {
                value: Value::Object(trimmed),
            });
        }

        let tools = item.get("tools")?.as_array()?;
        let original = self.serialize_json_like(&Value::Array(tools.clone()));
        if original.len() <= self.max_output_chars {
            return None;
        }

        let trimmed_tools = tools
            .iter()
            .map(|tool| self.trim_tool_search_tool(tool))
            .collect::<Vec<_>>();
        trimmed.insert("tools".to_owned(), Value::Array(trimmed_tools));
        Some(InputItem::Json {
            value: Value::Object(trimmed),
        })
    }

    fn trim_tool_search_tool(&self, tool: &Value) -> Value {
        let Some(object) = tool.as_object() else {
            return tool.clone();
        };
        let mut trimmed = object.clone();

        if let Some(description) = object.get("description").and_then(Value::as_str) {
            let mut shortened = description
                .chars()
                .take(self.preview_chars)
                .collect::<String>();
            if description.len() > self.preview_chars {
                shortened.push_str("...");
            }
            trimmed.insert("description".to_owned(), Value::String(shortened));
        }

        match object.get("type").and_then(Value::as_str) {
            Some("function") => {
                if let Some(parameters) = object.get("parameters").and_then(Value::as_object) {
                    trimmed.insert(
                        "parameters".to_owned(),
                        Value::Object(self.trim_json_schema(parameters)),
                    );
                }
            }
            Some("namespace") => {
                if let Some(tools) = object.get("tools").and_then(Value::as_array) {
                    trimmed.insert(
                        "tools".to_owned(),
                        Value::Array(
                            tools
                                .iter()
                                .map(|value| self.trim_tool_search_tool(value))
                                .collect(),
                        ),
                    );
                }
            }
            _ => {}
        }

        Value::Object(trimmed)
    }

    fn trim_json_schema(
        &self,
        schema: &serde_json::Map<String, Value>,
    ) -> serde_json::Map<String, Value> {
        let mut trimmed = serde_json::Map::new();
        for (key, value) in schema {
            if matches!(
                key.as_str(),
                "description" | "title" | "$comment" | "examples"
            ) {
                continue;
            }
            let new_value = match value {
                Value::Object(object) => Value::Object(self.trim_json_schema(object)),
                Value::Array(values) => Value::Array(
                    values
                        .iter()
                        .map(|value| match value {
                            Value::Object(object) => Value::Object(self.trim_json_schema(object)),
                            _ => value.clone(),
                        })
                        .collect(),
                ),
                _ => value.clone(),
            };
            trimmed.insert(key.clone(), new_value);
        }
        trimmed
    }

    fn serialize_json_like(&self, value: &Value) -> String {
        serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
    }
}

fn extract_tool_names(value: &Value) -> Vec<String> {
    let name = value
        .get("tool_name")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let namespace = value.get("namespace").and_then(Value::as_str);

    let mut names = Vec::new();
    if let Some(name) = name {
        if let Some(namespace) = namespace {
            names.push(format!("{namespace}.{name}"));
        }
        names.push(name);
    }
    names
}

#[cfg(test)]
mod tests {
    use agents_core::Agent;
    use serde_json::json;

    use super::*;

    #[test]
    fn trims_old_tool_outputs_only() {
        let trimmer = ToolOutputTrimmer {
            recent_turns: 1,
            max_output_chars: 50,
            ..ToolOutputTrimmer::default()
        };
        let data = CallModelData {
            model_data: ModelInputData {
                input: vec![
                    InputItem::Json {
                        value: json!({"role":"user","content":"older"}),
                    },
                    InputItem::Json {
                        value: json!({
                            "type":"function_call",
                            "call_id":"call-1",
                            "name":"search"
                        }),
                    },
                    InputItem::Json {
                        value: json!({
                            "type":"function_call_output",
                            "call_id":"call-1",
                            "output":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                        }),
                    },
                    InputItem::Json {
                        value: json!({"role":"user","content":"recent"}),
                    },
                ],
                instructions: None,
            },
            agent: Agent::builder("assistant").build(),
            context: None::<()>,
        };

        let trimmed = trimmer.apply(&data).expect("trimmer should succeed");
        let InputItem::Json { value } = &trimmed.input[2] else {
            panic!("expected json item");
        };
        let output = value
            .get("output")
            .and_then(Value::as_str)
            .expect("trimmed output should be text");
        assert!(output.starts_with("[Trimmed:"));
    }
}
