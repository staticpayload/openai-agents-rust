use crate::errors::Result;
use crate::exceptions::UserError;
use crate::items::{InputItem, RunItem};
use crate::run_config::RunConfig;
use crate::session::Session;
use crate::tracing::{custom_span, get_trace_provider};

pub(crate) async fn prepare_input_with_session(
    input: &[InputItem],
    session: &(dyn Session + Sync),
) -> Result<(Vec<InputItem>, Vec<InputItem>)> {
    let provider = get_trace_provider();
    let mut span = custom_span(
        "session.prepare_input",
        std::collections::BTreeMap::from([(
            "session_id".to_owned(),
            serde_json::Value::String(session.session_id().to_owned()),
        )]),
    );
    provider.start_span(&mut span, true);
    let mut prepared = session.get_items().await?;
    let original_input = input.to_vec();
    prepared.extend(original_input.clone());
    provider.finish_span(&mut span, true);
    Ok((prepared, original_input))
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

pub(crate) fn validate_session_conversation_settings(config: &RunConfig) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemorySession;

    #[tokio::test]
    async fn prepares_input_by_prefixing_session_history() {
        let session = MemorySession::new("session");
        session
            .add_items(vec![InputItem::from("history")])
            .await
            .expect("history should be added");

        let (prepared, original_input) =
            prepare_input_with_session(&[InputItem::from("new")], &session)
                .await
                .expect("prepared input should build");

        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].as_text(), Some("history"));
        assert_eq!(prepared[1].as_text(), Some("new"));
        assert_eq!(original_input.len(), 1);
        assert_eq!(original_input[0].as_text(), Some("new"));
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
}
