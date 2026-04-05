use crate::items::RunItem;
use crate::memory::OpenAIConversationSessionState;
use crate::model::ModelResponse;
use crate::run_config::RunConfig;
use crate::run_state::RunState;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct OpenAIServerConversationTracker {
    pub conversation_id: Option<String>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: bool,
    sent_initial_input: bool,
    remaining_initial_input: Option<Vec<crate::items::InputItem>>,
    sent_item_fingerprints: HashSet<String>,
    server_item_ids: HashSet<String>,
    server_tool_call_ids: HashSet<String>,
    prepared_item_sources_by_fingerprint: HashMap<String, Vec<crate::items::InputItem>>,
}

impl OpenAIServerConversationTracker {
    pub fn new(config: &RunConfig) -> Self {
        Self {
            conversation_id: config.conversation_id.clone(),
            previous_response_id: config.previous_response_id.clone(),
            auto_previous_response_id: config.auto_previous_response_id,
            sent_initial_input: false,
            remaining_initial_input: None,
            sent_item_fingerprints: HashSet::new(),
            server_item_ids: HashSet::new(),
            server_tool_call_ids: HashSet::new(),
            prepared_item_sources_by_fingerprint: HashMap::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.conversation_id.is_some()
            || self.previous_response_id.is_some()
            || self.auto_previous_response_id
    }

    pub fn previous_response_id(&self) -> Option<&str> {
        self.previous_response_id.as_deref()
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub fn apply_session_state(&mut self, state: &OpenAIConversationSessionState) {
        if self.conversation_id.is_none() {
            self.conversation_id = state.conversation_id.clone();
        }
        if self.previous_response_id.is_none() {
            self.previous_response_id = state.previous_response_id.clone();
        }
        self.auto_previous_response_id |= state.auto_previous_response_id;
    }

    pub fn apply_response(&mut self, response: &ModelResponse) {
        self.track_server_items(response);
        if (self.auto_previous_response_id || self.previous_response_id.is_some())
            && response.response_id.is_some()
        {
            self.previous_response_id = response.response_id.clone();
        }
    }

    pub fn prepare_input(
        &mut self,
        original_input: &[crate::items::InputItem],
        generated_items: &[RunItem],
    ) -> Vec<crate::items::InputItem> {
        let mut prepared = Vec::new();

        if !self.sent_initial_input {
            prepared.extend(original_input.iter().cloned());
            for item in original_input {
                self.register_prepared_item_source(item.clone(), item.clone());
            }
            self.remaining_initial_input =
                (!original_input.is_empty()).then(|| original_input.to_vec());
            self.sent_initial_input = true;
        } else if let Some(remaining) = self.remaining_initial_input.clone() {
            for item in &remaining {
                self.register_prepared_item_source(item.clone(), item.clone());
            }
            prepared.extend(remaining);
        }

        for run_item in generated_items {
            let Some(item) = run_item.to_input_item() else {
                continue;
            };
            if self
                .extract_item_id(&item)
                .is_some_and(|item_id| self.server_item_ids.contains(item_id))
            {
                continue;
            }
            if self
                .extract_output_call_id(&item)
                .is_some_and(|call_id| self.server_tool_call_ids.contains(call_id))
            {
                continue;
            }
            let fingerprint = fingerprint_input_item(&item);
            if self.sent_item_fingerprints.contains(&fingerprint) {
                continue;
            }
            self.register_prepared_item_source(item.clone(), item.clone());
            prepared.push(item);
        }

        prepared
    }

    pub fn mark_input_as_sent(&mut self, items: &[crate::items::InputItem]) {
        for item in items {
            let source = self.consume_prepared_item_source(item);
            let fingerprint = fingerprint_input_item(&source);
            self.sent_item_fingerprints.insert(fingerprint);
            self.remove_remaining_initial_item(&source);
        }
    }

    pub fn register_filtered_input_sources(
        &mut self,
        prepared_input: &[crate::items::InputItem],
        filtered_input: &[crate::items::InputItem],
    ) {
        if prepared_input == filtered_input {
            return;
        }

        let mut available_sources = prepared_input
            .iter()
            .map(|item| {
                (
                    fingerprint_input_item(item),
                    self.resolve_prepared_item_source(item),
                )
            })
            .collect::<Vec<_>>();

        for item in filtered_input {
            let filtered_fingerprint = fingerprint_input_item(item);
            let source_index = available_sources
                .iter()
                .position(|(prepared_fingerprint, _)| *prepared_fingerprint == filtered_fingerprint)
                .unwrap_or(0);
            let (_, source_item) = available_sources.remove(source_index);
            self.register_prepared_item_source(item.clone(), source_item);
            if available_sources.is_empty() {
                break;
            }
        }
    }

    pub fn rewind_input(&mut self, items: &[crate::items::InputItem]) {
        let mut rewind_items = Vec::new();
        for item in items {
            let source = self.consume_prepared_item_source(item);
            self.sent_item_fingerprints
                .remove(&fingerprint_input_item(&source));
            rewind_items.push(source);
        }

        if rewind_items.is_empty() {
            return;
        }

        let mut remaining = rewind_items;
        if let Some(existing) = self.remaining_initial_input.take() {
            remaining.extend(existing);
        }
        self.remaining_initial_input = Some(remaining);
    }

    pub fn track_server_items(&mut self, response: &ModelResponse) {
        let mut server_fingerprints = HashSet::new();
        for item in response.to_input_items() {
            if let Some(item_id) = self.extract_item_id(&item).map(ToOwned::to_owned) {
                self.server_item_ids.insert(item_id);
            }
            if let Some(call_id) = self.extract_output_call_id(&item).map(ToOwned::to_owned) {
                self.server_tool_call_ids.insert(call_id);
            }
            let fingerprint = fingerprint_input_item(&item);
            self.sent_item_fingerprints.insert(fingerprint.clone());
            server_fingerprints.insert(fingerprint);
        }

        if let Some(remaining) = self.remaining_initial_input.take() {
            let filtered = remaining
                .into_iter()
                .filter(|item| !server_fingerprints.contains(&fingerprint_input_item(item)))
                .collect::<Vec<_>>();
            self.remaining_initial_input = (!filtered.is_empty()).then_some(filtered);
        }
    }

    pub fn session_state(&self) -> OpenAIConversationSessionState {
        OpenAIConversationSessionState {
            conversation_id: self.conversation_id.clone(),
            previous_response_id: self.previous_response_id.clone(),
            auto_previous_response_id: self.auto_previous_response_id,
        }
    }

    pub fn apply_to_state(&self, state: &mut RunState) {
        state.conversation_id = self.conversation_id.clone();
        state.previous_response_id = self.previous_response_id.clone();
        state.auto_previous_response_id = self.auto_previous_response_id;
    }

    fn register_prepared_item_source(
        &mut self,
        prepared_item: crate::items::InputItem,
        source_item: crate::items::InputItem,
    ) {
        let fingerprint = fingerprint_input_item(&prepared_item);
        self.prepared_item_sources_by_fingerprint
            .entry(fingerprint)
            .or_default()
            .push(source_item);
    }

    fn consume_prepared_item_source(
        &mut self,
        item: &crate::items::InputItem,
    ) -> crate::items::InputItem {
        let source_item = self.resolve_prepared_item_source(item);
        let fingerprint = fingerprint_input_item(item);
        if let Some(source_items) = self
            .prepared_item_sources_by_fingerprint
            .get_mut(&fingerprint)
        {
            let source_item = source_items.remove(0);
            if source_items.is_empty() {
                self.prepared_item_sources_by_fingerprint
                    .remove(&fingerprint);
            }
            return source_item;
        }
        source_item
    }

    fn resolve_prepared_item_source(
        &self,
        item: &crate::items::InputItem,
    ) -> crate::items::InputItem {
        let fingerprint = fingerprint_input_item(item);
        self.prepared_item_sources_by_fingerprint
            .get(&fingerprint)
            .and_then(|items| items.first().cloned())
            .unwrap_or_else(|| item.clone())
    }

    fn remove_remaining_initial_item(&mut self, item: &crate::items::InputItem) {
        let Some(remaining) = self.remaining_initial_input.as_mut() else {
            return;
        };
        let target = fingerprint_input_item(item);
        if let Some(index) = remaining
            .iter()
            .position(|candidate| fingerprint_input_item(candidate) == target)
        {
            remaining.remove(index);
        }
        if remaining.is_empty() {
            self.remaining_initial_input = None;
        }
    }

    fn extract_item_id<'a>(&self, item: &'a crate::items::InputItem) -> Option<&'a str> {
        match item {
            crate::items::InputItem::Text { .. } => None,
            crate::items::InputItem::Json { value } => {
                value.get("id").and_then(serde_json::Value::as_str)
            }
        }
    }

    fn extract_output_call_id<'a>(&self, item: &'a crate::items::InputItem) -> Option<&'a str> {
        match item {
            crate::items::InputItem::Text { .. } => None,
            crate::items::InputItem::Json { value } => value
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .filter(|_| value.get("output").is_some()),
        }
    }
}

fn fingerprint_input_item(item: &crate::items::InputItem) -> String {
    serde_json::to_string(item).unwrap_or_else(|_| format!("{item:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::InputItem;
    use crate::run_config::RunConfig;

    #[test]
    fn tracker_only_replays_unsent_filtered_deltas() {
        let mut tracker = OpenAIServerConversationTracker::new(&RunConfig {
            conversation_id: Some("conv-1".to_owned()),
            ..RunConfig::default()
        });
        let original_input = vec![InputItem::from("first"), InputItem::from("second")];

        let first_prepared = tracker.prepare_input(&original_input, &[]);
        assert_eq!(first_prepared, original_input);

        tracker.mark_input_as_sent(&[InputItem::from("first")]);

        let retried = tracker.prepare_input(&original_input, &[]);
        assert_eq!(retried, vec![InputItem::from("second")]);
    }

    #[test]
    fn tracker_rewinds_sent_state_after_retry() {
        let mut tracker = OpenAIServerConversationTracker::new(&RunConfig {
            conversation_id: Some("conv-1".to_owned()),
            ..RunConfig::default()
        });
        let original_input = vec![InputItem::from("first"), InputItem::from("second")];

        let first_prepared = tracker.prepare_input(&original_input, &[]);
        tracker.mark_input_as_sent(&first_prepared);

        tracker.rewind_input(&first_prepared);

        let retried = tracker.prepare_input(&original_input, &[]);
        assert_eq!(
            retried,
            vec![InputItem::from("first"), InputItem::from("second")]
        );
    }

    #[test]
    fn tracker_marks_rewritten_filtered_items_as_original_sources() {
        let mut tracker = OpenAIServerConversationTracker::new(&RunConfig {
            conversation_id: Some("conv-1".to_owned()),
            ..RunConfig::default()
        });
        let original_input = vec![InputItem::from("hello")];

        let prepared = tracker.prepare_input(&original_input, &[]);
        let filtered = vec![InputItem::from("filtered-hello")];

        tracker.register_filtered_input_sources(&prepared, &filtered);
        tracker.mark_input_as_sent(&filtered);

        let retried = tracker.prepare_input(&original_input, &[]);
        assert!(retried.is_empty());
    }
}
