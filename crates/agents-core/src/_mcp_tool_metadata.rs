use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MCPToolMetadata {
    pub description: Option<String>,
    pub title: Option<String>,
}

fn get_mapping_or_attr<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key),
        _ => None,
    }
}

fn get_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn resolve_mcp_tool_title(tool: &Value) -> Option<String> {
    let explicit_title = get_non_empty_string(get_mapping_or_attr(tool, "title"));
    if explicit_title.is_some() {
        return explicit_title;
    }

    let annotations = get_mapping_or_attr(tool, "annotations")?;
    get_non_empty_string(get_mapping_or_attr(annotations, "title"))
}

pub fn resolve_mcp_tool_description(tool: &Value) -> Option<String> {
    get_non_empty_string(get_mapping_or_attr(tool, "description"))
}

pub fn resolve_mcp_tool_description_for_model(tool: &Value) -> String {
    resolve_mcp_tool_description(tool)
        .or_else(|| resolve_mcp_tool_title(tool))
        .unwrap_or_default()
}

pub fn extract_mcp_tool_metadata(tool: &Value) -> MCPToolMetadata {
    MCPToolMetadata {
        description: resolve_mcp_tool_description(tool),
        title: resolve_mcp_tool_title(tool),
    }
}

pub fn collect_mcp_list_tools_metadata(
    items: &[Value],
) -> HashMap<(String, String), MCPToolMetadata> {
    let mut metadata = HashMap::new();

    for item in items {
        let raw_item = item.get("raw_item").unwrap_or(item);
        if raw_item.get("type").and_then(Value::as_str) != Some("mcp_list_tools") {
            continue;
        }

        let Some(server_label) = raw_item
            .get("server_label")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(tools) = raw_item.get("tools").and_then(Value::as_array) else {
            continue;
        };

        for tool in tools {
            let Some(name) = tool
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            metadata.insert(
                (server_label.to_owned(), name.to_owned()),
                extract_mcp_tool_metadata(tool),
            );
        }
    }

    metadata
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn falls_back_to_title_for_model_description() {
        let tool = json!({
            "name": "search",
            "annotations": {"title": "Search"},
        });

        assert_eq!(resolve_mcp_tool_description_for_model(&tool), "Search");
    }
}
