use std::sync::{OnceLock, RwLock};

use crate::handoff::HandoffHistoryMapper;
use crate::items::{InputItem, RunItem};

pub const DEFAULT_CONVERSATION_HISTORY_START: &str = "<CONVERSATION HISTORY>";
pub const DEFAULT_CONVERSATION_HISTORY_END: &str = "</CONVERSATION HISTORY>";

static CONVERSATION_HISTORY_WRAPPERS: OnceLock<RwLock<(String, String)>> = OnceLock::new();

fn wrappers() -> &'static RwLock<(String, String)> {
    CONVERSATION_HISTORY_WRAPPERS.get_or_init(|| {
        RwLock::new((
            DEFAULT_CONVERSATION_HISTORY_START.to_owned(),
            DEFAULT_CONVERSATION_HISTORY_END.to_owned(),
        ))
    })
}

pub fn set_conversation_history_wrappers(start: Option<&str>, end: Option<&str>) {
    let mut wrappers = wrappers().write().expect("conversation history wrappers");
    if let Some(start) = start {
        wrappers.0 = start.to_owned();
    }
    if let Some(end) = end {
        wrappers.1 = end.to_owned();
    }
}

pub fn reset_conversation_history_wrappers() {
    *wrappers().write().expect("conversation history wrappers") = (
        DEFAULT_CONVERSATION_HISTORY_START.to_owned(),
        DEFAULT_CONVERSATION_HISTORY_END.to_owned(),
    );
}

pub fn get_conversation_history_wrappers() -> (String, String) {
    wrappers()
        .read()
        .expect("conversation history wrappers")
        .clone()
}

pub fn default_handoff_history_mapper(transcript: Vec<InputItem>) -> Vec<InputItem> {
    vec![build_summary_message(&transcript)]
}

pub fn nest_handoff_history(
    input_data: crate::handoff::HandoffInputData,
) -> crate::handoff::HandoffInputData {
    nest_handoff_history_with_mapper(input_data, None)
}

pub fn nest_handoff_history_with_mapper(
    input_data: crate::handoff::HandoffInputData,
    history_mapper: Option<HandoffHistoryMapper>,
) -> crate::handoff::HandoffInputData {
    let transcript = build_transcript(
        &input_data.input_history,
        &input_data.pre_handoff_items,
        &input_data.new_items,
    );
    let mapped_history = history_mapper
        .map(|mapper| mapper(transcript.clone()))
        .unwrap_or_else(|| default_handoff_history_mapper(transcript));

    crate::handoff::HandoffInputData {
        input_history: mapped_history,
        pre_handoff_items: input_data
            .pre_handoff_items
            .into_iter()
            .filter(|item| should_forward_run_item(item))
            .collect(),
        new_items: input_data.new_items.clone(),
        input_items: Some(
            input_data
                .new_items
                .into_iter()
                .filter(|item| should_forward_run_item(item))
                .collect(),
        ),
    }
}

fn build_transcript(
    input_history: &[InputItem],
    pre_handoff_items: &[RunItem],
    new_items: &[RunItem],
) -> Vec<InputItem> {
    let mut transcript = flatten_nested_history_messages(input_history);
    transcript.extend(pre_handoff_items.iter().filter_map(RunItem::to_input_item));
    transcript.extend(new_items.iter().filter_map(RunItem::to_input_item));
    transcript
}

fn build_summary_message(transcript: &[InputItem]) -> InputItem {
    let summary_lines = if transcript.is_empty() {
        vec!["(no previous turns recorded)".to_owned()]
    } else {
        transcript
            .iter()
            .enumerate()
            .map(|(index, item)| format!("{}. {}", index + 1, format_transcript_item(item)))
            .collect()
    };
    let (start, end) = get_conversation_history_wrappers();
    let content = std::iter::once(
        "For context, here is the conversation so far between the user and the previous agent:"
            .to_owned(),
    )
    .chain(std::iter::once(start))
    .chain(summary_lines)
    .chain(std::iter::once(end))
    .collect::<Vec<_>>()
    .join("\n");

    InputItem::Json {
        value: serde_json::json!({
            "role": "assistant",
            "content": content,
        }),
    }
}

fn format_transcript_item(item: &InputItem) -> String {
    match item {
        InputItem::Text { text } => format!("user: {text}"),
        InputItem::Json { value } => {
            if let Some(role) = value.get("role").and_then(serde_json::Value::as_str) {
                let content = value
                    .get("content")
                    .map(stringify_content)
                    .unwrap_or_default();
                if content.is_empty() {
                    role.to_owned()
                } else {
                    format!("{role}: {content}")
                }
            } else {
                let item_type = value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("item");
                format!("{item_type}: {}", stringify_content(value))
            }
        }
    }
}

fn stringify_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn flatten_nested_history_messages(items: &[InputItem]) -> Vec<InputItem> {
    items
        .iter()
        .flat_map(|item| {
            extract_nested_history_transcript(item).unwrap_or_else(|| vec![item.clone()])
        })
        .collect()
}

fn extract_nested_history_transcript(item: &InputItem) -> Option<Vec<InputItem>> {
    let InputItem::Json { value } = item else {
        return None;
    };
    let content = value.get("content")?.as_str()?;
    let (start_marker, end_marker) = get_conversation_history_wrappers();
    let start_idx = content.find(&start_marker)?;
    let end_idx = content.find(&end_marker)?;
    if end_idx <= start_idx {
        return None;
    }
    let body = &content[start_idx + start_marker.len()..end_idx];
    let parsed = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(parse_summary_line)
        .collect::<Vec<_>>();
    Some(parsed)
}

fn parse_summary_line(line: &str) -> Option<InputItem> {
    let stripped = line
        .split_once('.')
        .and_then(|(prefix, rest)| prefix.parse::<usize>().ok().map(|_| rest.trim()))
        .unwrap_or(line)
        .trim();
    let (role_part, remainder) = stripped.split_once(':')?;
    if remainder.trim().is_empty() {
        return None;
    }
    Some(InputItem::Json {
        value: serde_json::json!({
            "role": role_part.trim(),
            "content": remainder.trim(),
        }),
    })
}

fn should_forward_run_item(item: &RunItem) -> bool {
    !matches!(
        item,
        RunItem::ToolCall { .. } | RunItem::ToolCallOutput { .. } | RunItem::Reasoning { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::OutputItem;

    #[test]
    fn nests_history_into_summary_message() {
        let input_data = crate::handoff::HandoffInputData {
            input_history: vec![InputItem::Json {
                value: serde_json::json!({"role":"user","content":"hello"}),
            }],
            pre_handoff_items: vec![],
            new_items: vec![RunItem::MessageOutput {
                content: OutputItem::Text {
                    text: "hi".to_owned(),
                },
            }],
            input_items: None,
        };

        let nested = nest_handoff_history(input_data);
        assert_eq!(nested.input_history.len(), 1);
    }

    #[test]
    fn applies_custom_history_mapper_when_requested() {
        let input_data = crate::handoff::HandoffInputData {
            input_history: vec![InputItem::from("hello")],
            pre_handoff_items: vec![],
            new_items: vec![],
            input_items: None,
        };

        let nested = nest_handoff_history_with_mapper(
            input_data,
            Some(std::sync::Arc::new(|items| {
                let mut items = items;
                items.push(InputItem::from("mapped"));
                items
            })),
        );

        assert_eq!(nested.input_history.len(), 2);
        assert_eq!(nested.input_history[1].as_text(), Some("mapped"));
    }
}
