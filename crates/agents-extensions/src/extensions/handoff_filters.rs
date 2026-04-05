use agents_core::{InputItem, RunItem};

fn is_filtered_run_item(item: &RunItem) -> bool {
    matches!(
        item,
        RunItem::ToolCall { .. }
            | RunItem::ToolCallOutput { .. }
            | RunItem::HandoffCall { .. }
            | RunItem::HandoffOutput { .. }
            | RunItem::Reasoning { .. }
    )
}

fn is_filtered_input_item(item: &InputItem) -> bool {
    let InputItem::Json { value } = item else {
        return false;
    };
    matches!(
        value.get("type").and_then(serde_json::Value::as_str),
        Some(
            "function_call"
                | "function_call_output"
                | "computer_call"
                | "computer_call_output"
                | "file_search_call"
                | "tool_search_call"
                | "tool_search_output"
                | "web_search_call"
                | "mcp_call"
                | "mcp_list_tools"
                | "mcp_approval_request"
                | "mcp_approval_response"
                | "reasoning"
                | "tool_call"
                | "tool_call_output"
                | "handoff_call"
                | "handoff_output"
        )
    )
}

/// Removes tool, handoff, and reasoning items from replayable run history.
pub fn remove_all_tools(items: &[RunItem]) -> Vec<RunItem> {
    items
        .iter()
        .filter(|item| !is_filtered_run_item(item))
        .cloned()
        .collect()
}

/// Removes tool and reasoning records from model input history.
pub fn remove_tool_types_from_input(items: &[InputItem]) -> Vec<InputItem> {
    items
        .iter()
        .filter(|item| !is_filtered_input_item(item))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use agents_core::OutputItem;
    use serde_json::json;

    use super::*;

    #[test]
    fn filters_toolish_run_items() {
        let items = vec![
            RunItem::MessageOutput {
                content: OutputItem::Text {
                    text: "hello".to_owned(),
                },
            },
            RunItem::ToolCall {
                tool_name: "search".to_owned(),
                arguments: json!({"q":"rust"}),
                call_id: None,
                namespace: None,
            },
            RunItem::Reasoning {
                text: "thinking".to_owned(),
            },
        ];

        let filtered = remove_all_tools(&items);
        assert_eq!(filtered.len(), 1);
    }
}
