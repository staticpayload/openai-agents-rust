use crate::items::{OutputItem, RunItem};

pub(crate) fn extract_text_outputs(output: &[OutputItem]) -> Vec<String> {
    output
        .iter()
        .filter_map(|item| match item {
            OutputItem::Text { text } => Some(text.clone()),
            OutputItem::Json { .. }
            | OutputItem::ToolCall { .. }
            | OutputItem::Handoff { .. }
            | OutputItem::Reasoning { .. } => None,
        })
        .collect()
}

pub(crate) fn extract_final_output_text(output: &[OutputItem]) -> Option<String> {
    extract_text_outputs(output).into_iter().next()
}

pub(crate) fn build_message_output_items(output: &[OutputItem]) -> Vec<RunItem> {
    output
        .iter()
        .cloned()
        .map(|content| match content {
            OutputItem::ToolCall {
                call_id,
                tool_name,
                arguments,
                namespace,
            } => RunItem::ToolCall {
                tool_name,
                arguments,
                call_id: Some(call_id),
                namespace,
            },
            OutputItem::Handoff { target_agent } => RunItem::HandoffCall { target_agent },
            OutputItem::Reasoning { text } => RunItem::Reasoning { text },
            content => RunItem::MessageOutput { content },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_run_items_from_output() {
        let output = vec![OutputItem::Text {
            text: "hello".to_owned(),
        }];

        let items = build_message_output_items(&output);

        assert_eq!(items.len(), 1);
        assert_eq!(extract_final_output_text(&output).as_deref(), Some("hello"));
    }
}
