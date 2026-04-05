use crate::items::{InputItem, RunItem};
use crate::run_config::RunConfig;
use crate::run_state::RunState;
use crate::session::Session;
use crate::usage::Usage;

pub(crate) fn apply_resumed_conversation_settings(config: &mut RunConfig, state: &RunState) {
    if config.previous_response_id.is_none() {
        config.previous_response_id = state.previous_response_id.clone();
    }
    if config.conversation_id.is_none() {
        config.conversation_id = state.conversation_id.clone();
    }
    config.auto_previous_response_id |= state.auto_previous_response_id;
}

pub(crate) fn validate_session_conversation_settings(
    config: &RunConfig,
    session: &(dyn Session + Sync),
) -> crate::errors::Result<()> {
    crate::internal::session_persistence::validate_session_conversation_settings(config, session)
}

pub(crate) fn merge_usage(previous: Usage, next: Usage) -> Usage {
    Usage {
        input_tokens: previous.input_tokens.saturating_add(next.input_tokens),
        output_tokens: previous.output_tokens.saturating_add(next.output_tokens),
    }
}

pub(crate) fn build_generated_items_details(items: &[RunItem]) -> Vec<InputItem> {
    items.iter().filter_map(RunItem::to_input_item).collect()
}

pub(crate) async fn prepare_input_with_session(
    config: &RunConfig,
    input: &[InputItem],
    session: &(dyn Session + Sync),
) -> crate::errors::Result<(Vec<InputItem>, Vec<InputItem>)> {
    crate::internal::session_persistence::prepare_input_with_session(input, config, session).await
}
