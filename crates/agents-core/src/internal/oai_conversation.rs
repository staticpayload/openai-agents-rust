use crate::memory::OpenAIConversationSessionState;
use crate::model::ModelResponse;
use crate::run_config::RunConfig;
use crate::run_state::RunState;

#[derive(Clone, Debug, Default)]
pub(crate) struct OpenAIServerConversationTracker {
    pub conversation_id: Option<String>,
    pub previous_response_id: Option<String>,
    pub auto_previous_response_id: bool,
}

impl OpenAIServerConversationTracker {
    pub fn new(config: &RunConfig) -> Self {
        Self {
            conversation_id: config.conversation_id.clone(),
            previous_response_id: config.previous_response_id.clone(),
            auto_previous_response_id: config.auto_previous_response_id,
        }
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
        if (self.auto_previous_response_id || self.previous_response_id.is_some())
            && response.response_id.is_some()
        {
            self.previous_response_id = response.response_id.clone();
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
}
