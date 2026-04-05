use std::sync::Arc;

use agents_core::{Agent, InputItem, Result, RunItem, RunResultStreaming, Runner, StreamEvent};
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use tokio::sync::{Mutex, mpsc};

pub trait VoiceWorkflowBase: Send + Sync {
    fn run(&self, transcription: String) -> BoxStream<'static, Result<String>>;

    fn on_start(&self) -> BoxStream<'static, Result<String>> {
        stream::empty().boxed()
    }
}

pub struct VoiceWorkflowHelper;

impl VoiceWorkflowHelper {
    pub fn stream_text_from(result: RunResultStreaming) -> BoxStream<'static, Result<String>> {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut emitted_text = false;
            let mut events = Box::pin(result.stream_events());
            while let Some(event) = events.next().await {
                if let Some(text) = stream_event_text(&event) {
                    if text.is_empty() {
                        continue;
                    }
                    emitted_text = true;
                    if tx.send(Ok(text)).is_err() {
                        return;
                    }
                }
            }

            match result.wait_for_completion().await {
                Ok(final_result) => {
                    if !emitted_text {
                        if let Some(final_output) = final_result.final_output {
                            let _ = tx.send(Ok(final_output));
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error));
                }
            }
        });

        stream::unfold(
            rx,
            |mut rx: mpsc::UnboundedReceiver<Result<String>>| async move {
                rx.recv().await.map(|item| (item, rx))
            },
        )
        .boxed()
    }
}

fn stream_event_text(event: &StreamEvent) -> Option<String> {
    match event {
        StreamEvent::RunItemEvent(event) => match &event.item {
            RunItem::MessageOutput { content } => match content {
                agents_core::OutputItem::Text { text } => Some(text.clone()),
                agents_core::OutputItem::Json { value } => serde_json::to_string(value).ok(),
                agents_core::OutputItem::Reasoning { .. }
                | agents_core::OutputItem::ToolCall { .. }
                | agents_core::OutputItem::Handoff { .. } => None,
            },
            RunItem::ToolCallOutput { output, .. } => match output {
                agents_core::OutputItem::Text { text } => Some(text.clone()),
                agents_core::OutputItem::Json { value } => serde_json::to_string(value).ok(),
                agents_core::OutputItem::Reasoning { .. }
                | agents_core::OutputItem::ToolCall { .. }
                | agents_core::OutputItem::Handoff { .. } => None,
            },
            RunItem::ToolCall { .. }
            | RunItem::HandoffCall { .. }
            | RunItem::HandoffOutput { .. }
            | RunItem::Reasoning { .. } => None,
        },
        StreamEvent::RawResponseEvent(_)
        | StreamEvent::AgentUpdated(_)
        | StreamEvent::Lifecycle(_) => None,
    }
}

pub trait SingleAgentWorkflowCallbacks: Send + Sync {
    fn on_run(&self, _transcription: &str) {}
}

#[derive(Clone)]
struct WorkflowState {
    current_agent: Agent,
    input_history: Vec<InputItem>,
}

#[derive(Clone)]
pub struct SingleAgentVoiceWorkflow {
    state: Arc<Mutex<WorkflowState>>,
    callbacks: Option<Arc<dyn SingleAgentWorkflowCallbacks>>,
}

impl SingleAgentVoiceWorkflow {
    pub fn new(agent: Agent) -> Self {
        Self {
            state: Arc::new(Mutex::new(WorkflowState {
                current_agent: agent,
                input_history: Vec::new(),
            })),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<dyn SingleAgentWorkflowCallbacks>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }
}

impl VoiceWorkflowBase for SingleAgentVoiceWorkflow {
    fn run(&self, transcription: String) -> BoxStream<'static, Result<String>> {
        if let Some(callbacks) = &self.callbacks {
            callbacks.on_run(&transcription);
        }

        let state = self.state.clone();
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let (agent, input_history) = {
                let mut state = state.lock().await;
                state
                    .input_history
                    .push(InputItem::from(transcription.clone()));
                (state.current_agent.clone(), state.input_history.clone())
            };

            let streamed = match Runner::new()
                .run_items_streamed(&agent, input_history)
                .await
            {
                Ok(streamed) => streamed,
                Err(error) => {
                    let _ = tx.send(Err(error));
                    return;
                }
            };

            let mut text_stream = Box::pin(VoiceWorkflowHelper::stream_text_from(streamed.clone()));
            while let Some(chunk) = text_stream.next().await {
                if tx.send(chunk).is_err() {
                    return;
                }
            }

            if let Ok(final_result) = streamed.wait_for_completion().await {
                let mut state = state.lock().await;
                state.input_history = final_result.to_input_list();
                if let Some(last_agent) = final_result.last_agent {
                    state.current_agent = last_agent;
                }
            }
        });

        stream::unfold(
            rx,
            |mut rx: mpsc::UnboundedReceiver<Result<String>>| async move {
                rx.recv().await.map(|item| (item, rx))
            },
        )
        .boxed()
    }
}
