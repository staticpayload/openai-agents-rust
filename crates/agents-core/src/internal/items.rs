use crate::items::{InputItem, RunItem};
use crate::run_config::ReasoningItemIdPolicy;
use serde_json::Value;

pub(crate) fn copy_input_items(items: &[InputItem]) -> Vec<InputItem> {
    items.to_vec()
}

pub(crate) fn run_item_to_input_item(
    run_item: &RunItem,
    reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Option<InputItem> {
    if matches!(
        (run_item, reasoning_item_id_policy),
        (RunItem::Reasoning { .. }, ReasoningItemIdPolicy::Omit)
    ) {
        return None;
    }
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
    compose_replay_input_items(caller_items, generated_items, reasoning_item_id_policy)
}

pub(crate) fn compose_replay_input_items(
    base_items: &[InputItem],
    generated_items: &[RunItem],
    reasoning_item_id_policy: ReasoningItemIdPolicy,
) -> Vec<InputItem> {
    let mut replay = copy_input_items(base_items);
    let generated_inputs = run_items_to_input_items(generated_items, reasoning_item_id_policy);
    let overlap = trailing_generated_overlap(base_items, &generated_inputs);
    replay.extend(generated_inputs.into_iter().skip(overlap));
    replay
}

fn trailing_generated_overlap(base_items: &[InputItem], generated_inputs: &[InputItem]) -> usize {
    let max_overlap = base_items.len().min(generated_inputs.len());
    (1..=max_overlap)
        .rev()
        .find(|overlap| {
            let compared_items = base_items[base_items.len() - overlap..]
                .iter()
                .zip(generated_inputs[..*overlap].iter())
                .collect::<Vec<_>>();

            compared_items
                .iter()
                .all(|(base_item, generated_item)| base_item == generated_item)
                && compared_items.iter().any(|(base_item, generated_item)| {
                    stable_replay_dedupe_key(base_item)
                        .zip(stable_replay_dedupe_key(generated_item))
                        .is_some_and(|(base_key, generated_key)| base_key == generated_key)
                })
        })
        .unwrap_or(0)
}

fn stable_replay_dedupe_key(item: &InputItem) -> Option<String> {
    let value = match item {
        InputItem::Text { .. } => return None,
        InputItem::Json { value } => value,
    };

    let item_type = value
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("role").and_then(Value::as_str))?;

    if value.get("role").is_some() || item_type == "message" {
        return None;
    }

    if let Some(item_id) = value.get("id").and_then(Value::as_str) {
        if item_id != "__fake_id__" {
            return Some(format!("id:{item_type}:{item_id}"));
        }
    }

    value
        .get("call_id")
        .and_then(Value::as_str)
        .map(|call_id| format!("call:{item_type}:{call_id}"))
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

    #[test]
    fn omits_reasoning_items_when_policy_requests_it() {
        let prepared = prepare_model_input_items(
            &[InputItem::from("hello")],
            &[
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
            ],
            ReasoningItemIdPolicy::Omit,
        );

        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].as_text(), Some("hello"));
        assert_eq!(
            prepared[1],
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

    #[test]
    fn composes_replay_without_duplicating_overlapping_generated_items() {
        let replay = compose_replay_input_items(
            &[
                InputItem::from("filtered"),
                InputItem::Json {
                    value: json!({
                        "type": "tool_call",
                        "tool_name": "search",
                        "arguments": {"query": "rust"},
                        "call_id": "call-1",
                        "namespace": null
                    }),
                },
                InputItem::Json {
                    value: json!({
                        "type": "tool_call_output",
                        "tool_name": "search",
                        "output": {"type": "text", "text": "found"},
                        "call_id": "call-1",
                        "namespace": null
                    }),
                },
            ],
            &[
                RunItem::ToolCall {
                    tool_name: "search".to_owned(),
                    arguments: json!({"query":"rust"}),
                    call_id: Some("call-1".to_owned()),
                    namespace: None,
                },
                RunItem::ToolCallOutput {
                    tool_name: "search".to_owned(),
                    output: OutputItem::Text {
                        text: "found".to_owned(),
                    },
                    call_id: Some("call-1".to_owned()),
                    namespace: None,
                },
                RunItem::MessageOutput {
                    content: OutputItem::Text {
                        text: "done".to_owned(),
                    },
                },
            ],
            ReasoningItemIdPolicy::Preserve,
        );

        assert_eq!(replay[0].as_text(), Some("filtered"));
        assert_eq!(
            replay
                .iter()
                .filter(|item| item.as_text() == Some("done"))
                .count(),
            1
        );
        assert_eq!(replay.len(), 4);
    }

    #[test]
    fn composes_replay_preserving_repeated_generated_message_items() {
        let replay = compose_replay_input_items(
            &[InputItem::from("done")],
            &[RunItem::MessageOutput {
                content: OutputItem::Text {
                    text: "done".to_owned(),
                },
            }],
            ReasoningItemIdPolicy::Preserve,
        );

        assert_eq!(
            replay,
            vec![InputItem::from("done"), InputItem::from("done")]
        );
    }
}
