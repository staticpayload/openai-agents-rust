use crate::items::{InputItem, RunItem};
use crate::run_config::ReasoningItemIdPolicy;

pub(crate) fn copy_input_items(items: &[InputItem]) -> Vec<InputItem> {
    items.to_vec()
}

pub(crate) fn run_item_to_input_item(
    run_item: &RunItem,
    _reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Option<InputItem> {
    run_item.to_input_item()
}

pub(crate) fn run_items_to_input_items(
    run_items: &[RunItem],
    reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Vec<InputItem> {
    run_items
        .iter()
        .filter_map(|run_item| run_item_to_input_item(run_item, reasoning_item_id_policy))
        .collect()
}

pub(crate) fn prepare_model_input_items(
    caller_items: &[InputItem],
    generated_items: &[RunItem],
    reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Vec<InputItem> {
    let mut normalized = copy_input_items(caller_items);
    normalized.extend(run_items_to_input_items(
        generated_items,
        reasoning_item_id_policy,
    ));
    normalized
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::items::OutputItem;

    use super::*;

    #[test]
    fn prepares_model_input_from_history() {
        let input = vec![InputItem::from("hello")];
        let generated = vec![
            RunItem::Reasoning {
                text: "thinking".to_owned(),
            },
            RunItem::ToolCallOutput {
                tool_name: "search".to_owned(),
                output: OutputItem::Text {
                    text: "found".to_owned(),
                },
                call_id: Some("call-1".to_owned()),
                namespace: None,
            },
        ];

        let prepared =
            prepare_model_input_items(&input, &generated, ReasoningItemIdPolicy::Preserve);

        assert_eq!(prepared.len(), 3);
        assert_eq!(prepared[0].as_text(), Some("hello"));
        assert_eq!(
            prepared[1],
            InputItem::Json {
                value: json!({
                    "type": "reasoning",
                    "text": "thinking"
                })
            }
        );
        assert_eq!(
            prepared[2],
            InputItem::Json {
                value: json!({
                    "type": "tool_call_output",
                    "tool_name": "search",
                    "output": {
                        "type": "text",
                        "text": "found"
                    },
                    "call_id": "call-1",
                    "namespace": null
                })
            }
        );
    }
}
