use crate::errors::Result;
use crate::exceptions::UserError;
use crate::items::{InputItem, RunItem};
use crate::memory::resolve_session_limit;
use crate::run_config::RunConfig;
use crate::session::Session;
use crate::tracing::{custom_span, get_trace_provider};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) async fn prepare_input_with_session(
    input: &[InputItem],
    config: &RunConfig,
    session: &(dyn Session + Sync),
) -> Result<(Vec<InputItem>, Vec<InputItem>, Vec<InputItem>)> {
    let provider = get_trace_provider();
    let mut span = custom_span(
        "session.prepare_input",
        std::collections::BTreeMap::from([(
            "session_id".to_owned(),
            serde_json::Value::String(session.session_id().to_owned()),
        )]),
    );
    provider.start_span(&mut span, true);
    let resolved_settings = session
        .session_settings()
        .cloned()
        .unwrap_or_default()
        .resolve(config.session_settings.as_ref());
    let history = session
        .get_items_with_limit(resolve_session_limit(None, Some(&resolved_settings)))
        .await?;
    let original_input = input.to_vec();
    let (mut prepared, mut session_input_items) =
        if let Some(callback) = &config.session_input_callback {
            let history_for_callback = history.clone();
            let new_items_for_callback = original_input.clone();
            let mut history_refs = build_reference_map(&history_for_callback);
            let mut new_refs = build_reference_map(&new_items_for_callback);
            let mut history_counts = build_frequency_map(&history_for_callback);
            let mut new_counts = build_frequency_map(&new_items_for_callback);
            let combined = callback(history_for_callback, new_items_for_callback).await?;
            let mut session_input_items = Vec::new();

            for item in &combined {
                let key = session_item_key(item);
                if consume_reference(&mut new_refs, item) {
                    decrement_count(&mut new_counts, &key);
                    session_input_items.push(item.clone());
                    continue;
                }
                if consume_reference(&mut history_refs, item) {
                    decrement_count(&mut history_counts, &key);
                    continue;
                }
                if history_counts.get(&key).copied().unwrap_or_default() > 0 {
                    decrement_count(&mut history_counts, &key);
                    continue;
                }
                if new_counts.get(&key).copied().unwrap_or_default() > 0 {
                    decrement_count(&mut new_counts, &key);
                    session_input_items.push(item.clone());
                    continue;
                }

                session_input_items.push(item.clone());
            }

            (combined, session_input_items)
        } else {
            let mut prepared = history;
            prepared.extend(original_input.clone());
            (prepared, original_input.clone())
        };
    if prepared.is_empty() {
        prepared = original_input.clone();
        session_input_items = original_input.clone();
    }
    provider.finish_span(&mut span, true);
    Ok((prepared, original_input, session_input_items))
}

pub(crate) async fn save_result_to_session(
    session: &(dyn Session + Sync),
    original_input: &[InputItem],
    new_items: &[RunItem],
) -> Result<usize> {
    let provider = get_trace_provider();
    let mut span = custom_span(
        "session.save_result",
        std::collections::BTreeMap::from([(
            "session_id".to_owned(),
            serde_json::Value::String(session.session_id().to_owned()),
        )]),
    );
    provider.start_span(&mut span, true);
    let mut items = original_input.to_vec();
    items.extend(new_items.iter().filter_map(RunItem::to_input_item));
    let count = items.len();
    if count > 0 {
        session.add_items(items).await?;
    }
    provider.finish_span(&mut span, true);
    Ok(count)
}

pub(crate) fn validate_session_conversation_settings(
    config: &RunConfig,
    session: &(dyn Session + Sync),
) -> Result<()> {
    if session.conversation_session().is_some() {
        return Ok(());
    }

    if config.conversation_id.is_none()
        && config.previous_response_id.is_none()
        && !config.auto_previous_response_id
    {
        return Ok(());
    }

    Err(UserError {
        message: "Session persistence cannot be combined with conversation_id, previous_response_id, or auto_previous_response_id.".to_owned(),
    }
    .into())
}

fn build_reference_map(items: &[InputItem]) -> HashMap<String, Vec<InputItemIdentity>> {
    let mut refs = HashMap::new();
    for item in items {
        let Some(identity) = input_item_identity(item) else {
            continue;
        };
        refs.entry(session_item_key(item))
            .or_insert_with(Vec::new)
            .push(identity);
    }
    refs
}

fn consume_reference(refs: &mut HashMap<String, Vec<InputItemIdentity>>, item: &InputItem) -> bool {
    let Some(identity) = input_item_identity(item) else {
        return false;
    };
    let key = session_item_key(item);
    let Some(identities) = refs.get_mut(&key) else {
        return false;
    };
    let Some(index) = identities
        .iter()
        .position(|candidate| *candidate == identity)
    else {
        return false;
    };
    identities.remove(index);
    if identities.is_empty() {
        refs.remove(&key);
    }
    true
}

fn build_frequency_map(items: &[InputItem]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for item in items {
        let key = session_item_key(item);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn decrement_count(counts: &mut HashMap<String, usize>, key: &str) {
    if let Some(count) = counts.get_mut(key) {
        *count = count.saturating_sub(1);
    }
}

fn session_item_key(item: &InputItem) -> String {
    serde_json::to_string(item).expect("input items should serialize")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputItemIdentity {
    kind: InputItemIdentityKind,
    ptr: usize,
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputItemIdentityKind {
    Text,
    JsonString,
    JsonArray,
}

fn input_item_identity(item: &InputItem) -> Option<InputItemIdentity> {
    match item {
        InputItem::Text { text } => Some(InputItemIdentity {
            kind: InputItemIdentityKind::Text,
            ptr: text.as_ptr() as usize,
            len: text.len(),
        }),
        InputItem::Json { value } => json_value_identity(value),
    }
}

fn json_value_identity(value: &Value) -> Option<InputItemIdentity> {
    match value {
        Value::String(text) => Some(InputItemIdentity {
            kind: InputItemIdentityKind::JsonString,
            ptr: text.as_ptr() as usize,
            len: text.len(),
        }),
        Value::Array(items) => Some(InputItemIdentity {
            kind: InputItemIdentityKind::JsonArray,
            ptr: items.as_ptr() as usize,
            len: items.len(),
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemorySession;
    use futures::FutureExt;

    #[tokio::test]
    async fn prepares_input_by_prefixing_session_history() {
        let session = MemorySession::new("session");
        session
            .add_items(vec![InputItem::from("history")])
            .await
            .expect("history should be added");

        let (prepared, original_input, session_input_items) =
            prepare_input_with_session(&[InputItem::from("new")], &RunConfig::default(), &session)
                .await
                .expect("prepared input should build");

        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].as_text(), Some("history"));
        assert_eq!(prepared[1].as_text(), Some("new"));
        assert_eq!(original_input.len(), 1);
        assert_eq!(original_input[0].as_text(), Some("new"));
        assert_eq!(session_input_items.len(), 1);
        assert_eq!(session_input_items[0].as_text(), Some("new"));
    }

    #[tokio::test]
    async fn saves_original_input_and_generated_items_to_session() {
        let session = MemorySession::new("session");
        let count = save_result_to_session(
            &session,
            &[InputItem::from("hello")],
            &[RunItem::Reasoning {
                text: "thinking".to_owned(),
            }],
        )
        .await
        .expect("session should save");

        let items = session.get_items().await.expect("items should load");
        assert_eq!(count, 2);
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn session_input_callback_returns_transformed_items_for_persistence() {
        let session = MemorySession::new("session");
        session
            .add_items(vec![InputItem::from("history")])
            .await
            .expect("history should be added");
        let config = RunConfig {
            session_input_callback: Some(std::sync::Arc::new(|history, mut new_items| {
                async move {
                    let mut combined = history;
                    let transformed = InputItem::from("[redacted]");
                    new_items[0] = transformed.clone();
                    combined.extend(new_items);
                    Ok(combined)
                }
                .boxed()
            })),
            ..RunConfig::default()
        };

        let (prepared, _original_input, session_items) =
            prepare_input_with_session(&[InputItem::from("secret")], &config, &session)
                .await
                .expect("prepared input should build");

        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].as_text(), Some("history"));
        assert_eq!(prepared[1].as_text(), Some("[redacted]"));
        assert_eq!(session_items, vec![InputItem::from("[redacted]")]);
    }

    #[tokio::test]
    async fn session_input_callback_preserves_duplicate_value_provenance() {
        let session = MemorySession::new("session");
        session
            .add_items(vec![InputItem::from("same")])
            .await
            .expect("history should be added");
        let config = RunConfig {
            session_input_callback: Some(std::sync::Arc::new(|mut history, mut new_items| {
                async move {
                    let history_item = history.remove(0);
                    let _dropped_new_item = new_items.remove(0);
                    Ok(vec![history_item])
                }
                .boxed()
            })),
            ..RunConfig::default()
        };

        let (prepared, _original_input, session_items) =
            prepare_input_with_session(&[InputItem::from("same")], &config, &session)
                .await
                .expect("prepared input should build");

        assert_eq!(prepared, vec![InputItem::from("same")]);
        assert!(session_items.is_empty());
    }
}
