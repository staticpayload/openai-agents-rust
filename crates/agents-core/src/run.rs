use std::sync::{Arc, OnceLock, RwLock};

use uuid::Uuid;

use crate::agent::Agent;
use crate::errors::Result;
use crate::exceptions::{MaxTurnsExceeded, ModelBehaviorError, UserError};
use crate::handoff::{Handoff, HandoffInputData, nest_handoff_history_with_mapper};
use crate::internal::agent_runner_helpers as internal_agent_runner_helpers;
use crate::internal::error_handlers as internal_error_handlers;
use crate::internal::guardrails as internal_guardrails;
use crate::internal::items as internal_items;
use crate::internal::model_retry as internal_model_retry;
use crate::internal::oai_conversation as internal_oai_conversation;
use crate::internal::session_persistence as internal_session_persistence;
use crate::internal::streaming::StreamRecorder;
use crate::internal::tool_execution as internal_tool_execution;
use crate::internal::turn_preparation as internal_turn_preparation;
use crate::internal::turn_resolution as internal_turn_resolution;
use crate::items::{InputItem, OutputItem, RunItem};
use crate::lifecycle::{SharedAgentHooks, SharedRunHooks};
use crate::model::{ModelProvider, ModelRequest, ModelResponse, get_default_model_settings};
use crate::result::{RunResult, RunResultStreaming};
use crate::run_config::{DEFAULT_MAX_TURNS, ModelInputData, RunConfig, RunOptions};
use crate::run_error_handlers::{RunErrorData, RunErrorHandlerInput};
use crate::run_state::{RunInterruptionKind, RunState};
use crate::session::Session;
use crate::tracing::{
    SpanData, TraceCtxManager, get_model_tracing_impl, get_trace_provider, handoff_span,
};
use crate::usage::Usage;

/// Entry point for executing agents.
#[derive(Clone, Default)]
pub struct Runner {
    model_provider: Option<Arc<dyn ModelProvider>>,
    config: RunConfig,
}

pub type AgentRunner = Runner;

#[derive(Clone)]
struct ResolvedRunCall {
    runner: Runner,
    context: Option<crate::run_context::RunContextWrapper>,
    session: Option<Arc<dyn Session + Sync>>,
}

#[derive(Debug)]
struct HandoffTransition {
    input_history: Vec<InputItem>,
    pre_handoff_items: Vec<RunItem>,
    normalized_step_items: Vec<RunItem>,
    session_step_items: Vec<RunItem>,
}

fn default_agent_runner_cell() -> &'static RwLock<AgentRunner> {
    static DEFAULT_AGENT_RUNNER: OnceLock<RwLock<AgentRunner>> = OnceLock::new();
    DEFAULT_AGENT_RUNNER.get_or_init(|| RwLock::new(AgentRunner::new()))
}

pub fn set_default_agent_runner(runner: Option<AgentRunner>) {
    *default_agent_runner_cell()
        .write()
        .expect("default agent runner lock") = runner.unwrap_or_default();
}

pub fn get_default_agent_runner() -> AgentRunner {
    default_agent_runner_cell()
        .read()
        .expect("default agent runner lock")
        .clone()
}

impl Runner {
    pub fn new() -> Self {
        Self {
            model_provider: None,
            config: RunConfig {
                max_turns: DEFAULT_MAX_TURNS,
                workflow_name: "Agent workflow".to_owned(),
                ..RunConfig::default()
            },
        }
    }

    pub fn with_config(mut self, config: RunConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_model_provider(mut self, model_provider: Arc<dyn ModelProvider>) -> Self {
        self.model_provider = Some(model_provider);
        self
    }

    fn resolve_run_call(&self, options: RunOptions) -> ResolvedRunCall {
        let mut runner = self.clone();
        if let Some(model_provider) = options.model_provider {
            runner.model_provider = Some(model_provider);
        }

        if let Some(run_config) = options.run_config {
            runner.config = run_config;
        }
        if let Some(max_turns) = options.max_turns {
            runner.config.max_turns = max_turns;
        }
        if let Some(hooks) = options.hooks {
            runner.config.run_hooks = Some(hooks);
        }
        if let Some(error_handlers) = options.error_handlers {
            runner.config.run_error_handlers = error_handlers;
        }
        if options.previous_response_id.is_some() {
            runner.config.previous_response_id = options.previous_response_id;
        }
        if let Some(auto_previous_response_id) = options.auto_previous_response_id {
            runner.config.auto_previous_response_id = auto_previous_response_id;
        }
        if options.conversation_id.is_some() {
            runner.config.conversation_id = options.conversation_id;
        }

        let context = options
            .context
            .map(crate::run_context::RunContextWrapper::new);

        ResolvedRunCall {
            runner,
            context,
            session: options.session,
        }
    }

    pub fn run_sync(&self, agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
        self.run_sync_with_options(agent, vec![input.into()], RunOptions::default())
    }

    pub fn run_sync_with_options(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        options: RunOptions,
    ) -> Result<RunResult> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(UserError {
                message:
                    "Runner::run_sync() cannot be called when an event loop is already running."
                        .to_owned(),
            }
            .into());
        }

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| UserError {
                message: format!("failed to create runtime for run_sync(): {error}"),
            })?
            .block_on(self.run_with_options(agent, input, options))
    }

    pub async fn run(&self, agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
        self.run_with_options(agent, vec![input.into()], RunOptions::default())
            .await
    }

    pub async fn run_with_options(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        options: RunOptions,
    ) -> Result<RunResult> {
        let resolved = self.resolve_run_call(options);
        resolved
            .runner
            .run_resolved_items(agent, input, resolved.session, resolved.context, None)
            .await
    }

    pub async fn run_streamed(
        &self,
        agent: &Agent,
        input: impl Into<InputItem>,
    ) -> Result<RunResultStreaming> {
        self.run_streamed_with_options(agent, vec![input.into()], RunOptions::default())
            .await
    }

    pub async fn run_streamed_with_options(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        options: RunOptions,
    ) -> Result<RunResultStreaming> {
        let resolved = self.resolve_run_call(options);
        let max_turns = resolved.runner.config.max_turns;
        let recorder = StreamRecorder::new();
        let shared_state = recorder.shared_state();
        let runner = resolved.runner.clone();
        let agent = agent.clone();
        let session = resolved.session;
        let context = resolved.context;

        tokio::spawn(async move {
            let result = runner
                .run_resolved_items(&agent, input, session, context, Some(recorder.clone()))
                .await;
            recorder.complete(result).await;
        });

        Ok(RunResultStreaming::from_live(max_turns, shared_state))
    }

    pub async fn run_items(&self, agent: &Agent, input: Vec<InputItem>) -> Result<RunResult> {
        self.run_with_options(agent, input, RunOptions::default())
            .await
    }

    pub async fn run_items_streamed(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
    ) -> Result<RunResultStreaming> {
        self.run_streamed_with_options(agent, input, RunOptions::default())
            .await
    }

    pub async fn run_with_session(
        &self,
        agent: &Agent,
        input: impl Into<InputItem>,
        session: &(dyn Session + Sync),
    ) -> Result<RunResult> {
        self.run_items_with_session(agent, vec![input.into()], session)
            .await
    }

    pub async fn run_items_with_session(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        session: &(dyn Session + Sync),
    ) -> Result<RunResult> {
        self.run_items_with_session_and_context(
            agent,
            input,
            session,
            crate::run_context::RunContextWrapper::new(crate::run_context::RunContext::default()),
        )
        .await
    }

    pub(crate) async fn run_items_with_context(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        context: crate::run_context::RunContextWrapper,
    ) -> Result<RunResult> {
        self.run_items_internal(agent, input.clone(), input, None, None, Some(context), None)
            .await
    }

    pub(crate) async fn run_items_with_session_and_context(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        session: &(dyn Session + Sync),
        context: crate::run_context::RunContextWrapper,
    ) -> Result<RunResult> {
        self.run_items_with_session_and_context_internal(agent, input, session, context, None)
            .await
    }

    pub(crate) async fn run_items_streamed_with_context(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        context: crate::run_context::RunContextWrapper,
    ) -> Result<RunResultStreaming> {
        let recorder = StreamRecorder::new();
        let shared_state = recorder.shared_state();
        let runner = self.clone();
        let agent = agent.clone();

        tokio::spawn(async move {
            let result = runner
                .run_items_internal(
                    &agent,
                    input.clone(),
                    input,
                    None,
                    None,
                    Some(context),
                    Some(recorder.clone()),
                )
                .await;
            recorder.complete(result).await;
        });

        Ok(RunResultStreaming::from_live(
            self.config.max_turns,
            shared_state,
        ))
    }

    pub(crate) async fn run_items_streamed_with_session_and_context(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        session: Arc<dyn Session + Sync>,
        context: crate::run_context::RunContextWrapper,
    ) -> Result<RunResultStreaming> {
        let recorder = StreamRecorder::new();
        let shared_state = recorder.shared_state();
        let runner = self.clone();
        let agent = agent.clone();

        tokio::spawn(async move {
            let result = runner
                .run_items_with_session_and_context_internal(
                    &agent,
                    input,
                    session.as_ref(),
                    context,
                    Some(recorder.clone()),
                )
                .await;
            recorder.complete(result).await;
        });

        Ok(RunResultStreaming::from_live(
            self.config.max_turns,
            shared_state,
        ))
    }

    async fn run_resolved_items(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        session: Option<Arc<dyn Session + Sync>>,
        context: Option<crate::run_context::RunContextWrapper>,
        recorder: Option<StreamRecorder>,
    ) -> Result<RunResult> {
        if let Some(session) = session {
            self.run_items_with_session_and_context_internal(
                agent,
                input,
                session.as_ref(),
                context.unwrap_or_else(|| {
                    crate::run_context::RunContextWrapper::new(
                        crate::run_context::RunContext::default(),
                    )
                }),
                recorder,
            )
            .await
        } else if let Some(context) = context {
            self.run_items_internal(
                agent,
                input.clone(),
                input,
                None,
                None,
                Some(context),
                recorder,
            )
            .await
        } else {
            self.run_items_internal(agent, input.clone(), input, None, None, None, recorder)
                .await
        }
    }

    async fn run_items_with_session_and_context_internal(
        &self,
        agent: &Agent,
        input: Vec<InputItem>,
        session: &(dyn Session + Sync),
        context: crate::run_context::RunContextWrapper,
        recorder: Option<StreamRecorder>,
    ) -> Result<RunResult> {
        internal_agent_runner_helpers::validate_session_conversation_settings(
            &self.config,
            session,
        )?;
        let (prepared_input, original_input, session_input_items) =
            internal_agent_runner_helpers::prepare_input_with_session(
                &self.config,
                &input,
                session,
            )
            .await?;
        self.run_items_internal(
            agent,
            original_input,
            prepared_input,
            Some(session_input_items),
            Some(session),
            Some(context),
            recorder,
        )
        .await
    }

    async fn run_items_internal(
        &self,
        agent: &Agent,
        original_input: Vec<InputItem>,
        base_input: Vec<InputItem>,
        session_input_items: Option<Vec<InputItem>>,
        session: Option<&(dyn Session + Sync)>,
        context_override: Option<crate::run_context::RunContextWrapper>,
        stream_recorder: Option<StreamRecorder>,
    ) -> Result<RunResult> {
        internal_turn_preparation::validate_run_hooks()?;

        let workflow_name = if self.config.workflow_name.is_empty() {
            agent.name.clone()
        } else {
            self.config.workflow_name.clone()
        };
        let trace_id = self
            .config
            .trace_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok());
        let mut trace_manager = TraceCtxManager::with_options(
            &workflow_name,
            trace_id,
            self.config.group_id.clone(),
            self.config.trace_metadata.clone(),
            self.config.tracing.as_ref(),
            self.config.tracing_disabled
                || self
                    .config
                    .tracing
                    .as_ref()
                    .is_some_and(|config| config.disabled),
        );
        let trace = trace_manager.trace().clone();
        let session_conversation_state = if let Some(session) = session {
            if let Some(conversation_session) = session.conversation_session() {
                Some(
                    conversation_session
                        .load_openai_conversation_state()
                        .await?,
                )
            } else {
                None
            }
        } else {
            None
        };

        let mut context = context_override
            .unwrap_or_else(|| internal_guardrails::new_run_context(trace.workflow_name.clone()));
        if context.context.workflow_name.is_none() {
            context.context.workflow_name = Some(trace.workflow_name.clone());
        }
        if context.context.conversation_id.is_none() {
            context.context.conversation_id = self.config.conversation_id.clone().or_else(|| {
                session_conversation_state
                    .as_ref()
                    .and_then(|state| state.conversation_id.clone())
            });
        }
        context.turn_input = original_input.clone();

        dispatch_agent_start(
            self.config.run_hooks.as_ref(),
            agent.hooks.as_ref(),
            &context,
            agent,
            0,
        )
        .await;
        if let Some(recorder) = &stream_recorder {
            recorder
                .push_lifecycle(
                    "agent_start",
                    Some(serde_json::json!({
                        "agent_name": agent.name.clone(),
                        "turn": 0,
                    })),
                )
                .await;
        }

        let all_input_guardrails = merged_input_guardrails(agent, &self.config);
        let input_guardrail_results = internal_guardrails::run_input_guardrails(
            agent,
            &all_input_guardrails,
            &original_input,
            &context,
        )
        .await?;
        if let Some(recorder) = &stream_recorder {
            recorder
                .push_lifecycle(
                    "input_guardrails_completed",
                    Some(serde_json::json!({
                        "count": input_guardrail_results.len(),
                    })),
                )
                .await;
        }

        let mut current_agent = agent.clone();
        let mut current_input_history = base_input;
        let mut normalized_generated_items = Vec::new();
        let mut session_generated_items = Vec::new();
        let mut raw_responses = Vec::new();
        let mut usage = Usage::default();
        let mut output_guardrail_results = Vec::new();
        let mut tool_input_guardrail_results = Vec::new();
        let mut tool_output_guardrail_results = Vec::new();
        let mut interruptions = Vec::new();
        let mut final_output = None;
        let mut final_output_items = Vec::new();
        let mut conversation_tracker =
            internal_oai_conversation::OpenAIServerConversationTracker::new(&self.config);
        if let Some(session_state) = &session_conversation_state {
            conversation_tracker.apply_session_state(session_state);
        }

        for _turn in 0..self.config.max_turns {
            if let Some(recorder) = &stream_recorder {
                recorder
                    .push_lifecycle(
                        "turn_started",
                        Some(serde_json::json!({
                            "agent_name": current_agent.name.clone(),
                            "turn": raw_responses.len(),
                        })),
                    )
                    .await;
            }
            let prepared_input = internal_items::prepare_model_input_items(
                &current_input_history,
                &normalized_generated_items,
                self.config.reasoning_item_id_policy,
            );
            let model_data = internal_turn_preparation::maybe_filter_model_input(
                &self.config,
                &current_agent,
                &context,
                ModelInputData {
                    input: prepared_input,
                    instructions: current_agent.instructions.clone(),
                },
            )
            .await?;

            let response = self
                .call_model_with_retry(
                    &current_agent,
                    &context,
                    trace.id,
                    model_data,
                    conversation_tracker.previous_response_id(),
                    conversation_tracker.conversation_id(),
                )
                .await?;
            usage = internal_agent_runner_helpers::merge_usage(usage, response.usage.clone());
            context.usage = usage;
            conversation_tracker.apply_response(&response);

            let output = response.output.clone();
            let response_items = internal_turn_resolution::build_message_output_items(&output);
            if let Some(recorder) = &stream_recorder {
                recorder.push_raw_response(&response).await;
                recorder.push_run_items(&response_items).await;
            }
            raw_responses.push(response);

            if let Some((handoff, target_agent)) = resolve_handoff(&current_agent, &output)? {
                dispatch_handoff(
                    self.config.run_hooks.as_ref(),
                    target_agent.hooks.as_ref(),
                    &context,
                    &current_agent,
                    &target_agent,
                )
                .await;
                let provider = get_trace_provider();
                let mut span = handoff_span(
                    Some(current_agent.name.clone()),
                    Some(target_agent.name.clone()),
                );
                provider.start_span(&mut span, true);
                provider.finish_span(&mut span, true);
                let mut step_items = response_items;
                let handoff_output = RunItem::HandoffOutput {
                    source_agent: current_agent.name.clone(),
                };
                step_items.push(handoff_output.clone());
                if let Some(recorder) = &stream_recorder {
                    recorder
                        .push_lifecycle(
                            "handoff",
                            Some(serde_json::json!({
                                "from_agent": current_agent.name.clone(),
                                "to_agent": target_agent.name.clone(),
                            })),
                        )
                        .await;
                    recorder.push_run_items(&[handoff_output]).await;
                    recorder.push_agent_updated(&target_agent).await;
                }
                let handoff_transition = apply_handoff_transition(
                    &self.config,
                    &handoff,
                    &current_input_history,
                    &normalized_generated_items,
                    step_items,
                )
                .await?;
                current_input_history = handoff_transition.input_history;
                normalized_generated_items = handoff_transition.pre_handoff_items.clone();
                normalized_generated_items.extend(handoff_transition.normalized_step_items);
                session_generated_items = handoff_transition.pre_handoff_items;
                session_generated_items.extend(handoff_transition.session_step_items);
                current_agent = target_agent;
                dispatch_agent_start(
                    self.config.run_hooks.as_ref(),
                    current_agent.hooks.as_ref(),
                    &context,
                    &current_agent,
                    raw_responses.len(),
                )
                .await;
                if let Some(recorder) = &stream_recorder {
                    recorder
                        .push_lifecycle(
                            "agent_start",
                            Some(serde_json::json!({
                                "agent_name": current_agent.name.clone(),
                                "turn": raw_responses.len(),
                            })),
                        )
                        .await;
                }
                continue;
            }
            normalized_generated_items.extend(response_items.clone());
            session_generated_items.extend(response_items);

            let tool_calls = internal_tool_execution::extract_tool_calls(&output);
            let all_output_guardrails = merged_output_guardrails(&current_agent, &self.config);
            if tool_calls.is_empty() {
                output_guardrail_results = internal_guardrails::run_output_guardrails(
                    &current_agent,
                    &all_output_guardrails,
                    &output,
                    &context,
                )
                .await?;
                if let Some(recorder) = &stream_recorder {
                    recorder
                        .push_lifecycle(
                            "output_guardrails_completed",
                            Some(serde_json::json!({
                                "count": output_guardrail_results.len(),
                            })),
                        )
                        .await;
                }
                final_output = internal_turn_resolution::extract_final_output_text(&output);
                final_output_items = output;
                dispatch_agent_end(
                    self.config.run_hooks.as_ref(),
                    current_agent.hooks.as_ref(),
                    &context,
                    &current_agent,
                    final_output.as_deref(),
                    raw_responses.len(),
                )
                .await;
                if let Some(recorder) = &stream_recorder {
                    recorder
                        .push_lifecycle(
                            "agent_end",
                            Some(serde_json::json!({
                                "agent_name": current_agent.name.clone(),
                                "final_output": final_output.clone(),
                            })),
                        )
                        .await;
                }
                break;
            }

            let tool_outcome = internal_tool_execution::execute_local_function_tools(
                &current_agent,
                &self.config,
                &context,
                tool_calls,
                stream_recorder.as_ref(),
            )
            .await?;
            if let Some(recorder) = &stream_recorder {
                recorder.push_run_items(&tool_outcome.new_items).await;
            }
            if tool_outcome.interruptions.is_empty() {
                let tool_final_output = current_agent
                    .tool_use_behavior
                    .evaluate(&context, &tool_outcome.tool_results)
                    .await?;
                if tool_final_output.is_final_output {
                    if let Some(value) = tool_final_output.final_output {
                        final_output =
                            Some(internal_error_handlers::format_final_output_text(&value));
                        final_output_items =
                            vec![internal_error_handlers::create_message_output_item(&value)];
                        normalized_generated_items.extend(tool_outcome.new_items.clone());
                        session_generated_items.extend(tool_outcome.new_items);
                        tool_input_guardrail_results.extend(tool_outcome.input_guardrail_results);
                        tool_output_guardrail_results.extend(tool_outcome.output_guardrail_results);
                        dispatch_agent_end(
                            self.config.run_hooks.as_ref(),
                            current_agent.hooks.as_ref(),
                            &context,
                            &current_agent,
                            final_output.as_deref(),
                            raw_responses.len(),
                        )
                        .await;
                        if let Some(recorder) = &stream_recorder {
                            recorder
                                .push_lifecycle(
                                    "agent_end",
                                    Some(serde_json::json!({
                                        "agent_name": current_agent.name.clone(),
                                        "final_output": final_output.clone(),
                                    })),
                                )
                                .await;
                        }
                        break;
                    }
                }
            }
            tool_input_guardrail_results.extend(tool_outcome.input_guardrail_results);
            tool_output_guardrail_results.extend(tool_outcome.output_guardrail_results);
            normalized_generated_items.extend(tool_outcome.new_items.clone());
            session_generated_items.extend(tool_outcome.new_items);
            if !tool_outcome.interruptions.is_empty() {
                if let Some(recorder) = &stream_recorder {
                    recorder
                        .push_lifecycle(
                            "run_interrupted",
                            Some(serde_json::json!({
                                "interruptions": tool_outcome.interruptions.iter().map(|item| {
                                    serde_json::json!({
                                        "call_id": item.call_id,
                                        "tool_name": item.tool_name,
                                        "reason": item.reason,
                                    })
                                }).collect::<Vec<_>>(),
                            })),
                        )
                        .await;
                }
                interruptions = tool_outcome.interruptions;
                break;
            }

            if let Some(recorder) = &stream_recorder {
                recorder
                    .push_lifecycle(
                        "turn_completed",
                        Some(serde_json::json!({
                            "agent_name": current_agent.name.clone(),
                            "turn": raw_responses.len(),
                        })),
                    )
                    .await;
            }
        }

        if final_output_items.is_empty() && final_output.is_none() && interruptions.is_empty() {
            let max_turns_error = MaxTurnsExceeded {
                message: format!(
                    "run for agent `{}` exceeded max_turns ({}) before producing a final output",
                    agent.name, self.config.max_turns
                ),
            };

            if let Some(handler) = &self.config.run_error_handlers.max_turns {
                let history = internal_items::prepare_model_input_items(
                    &current_input_history,
                    &normalized_generated_items,
                    self.config.reasoning_item_id_policy,
                );
                let handler_result = handler(RunErrorHandlerInput {
                    error: MaxTurnsExceeded {
                        message: max_turns_error.message.clone(),
                    },
                    context: context.clone(),
                    run_data: RunErrorData {
                        input: original_input.clone(),
                        new_items: session_generated_items.clone(),
                        history,
                        output: final_output_items.clone(),
                        raw_responses: raw_responses.clone(),
                        last_agent: current_agent.clone(),
                    },
                })
                .await;

                if let Some(handler_result) = handler_result {
                    internal_error_handlers::validate_handler_final_output(
                        &handler_result.final_output,
                    )?;
                    if let Some((text, output_item, include_in_history)) =
                        internal_error_handlers::resolve_run_error_handler_result(Some(
                            handler_result,
                        ))
                    {
                        final_output = Some(text);
                        final_output_items = vec![output_item.clone()];
                        if include_in_history {
                            let history_item = RunItem::MessageOutput {
                                content: output_item,
                            };
                            normalized_generated_items.push(history_item.clone());
                            session_generated_items.push(history_item);
                        }
                    }
                } else {
                    let _ = trace_manager.finish();
                    return Err(max_turns_error.into());
                }
            } else {
                let _ = trace_manager.finish();
                return Err(max_turns_error.into());
            }
        }

        let persisted_item_count = if let Some(session) = session {
            internal_session_persistence::save_result_to_session(
                session,
                session_input_items.as_deref().unwrap_or(&original_input),
                &session_generated_items,
            )
            .await?
        } else {
            0
        };
        if let Some(session) = session {
            if let Some(conversation_session) = session.conversation_session() {
                conversation_session
                    .save_openai_conversation_state(conversation_tracker.session_state())
                    .await?;
            }
            if let Some(compaction_session) = session.compaction_session() {
                compaction_session.run_compaction(None).await?;
            }
        }

        let mut run_state = RunState::new(
            &context,
            original_input.clone(),
            agent.clone(),
            self.config.max_turns,
        )?;
        run_state.current_turn = raw_responses.len();
        run_state.set_current_agent(current_agent.clone());
        run_state.set_trace(trace.clone());
        conversation_tracker.apply_to_state(&mut run_state);
        run_state.persisted_item_count = persisted_item_count;
        run_state.normalized_input =
            (current_input_history != original_input).then_some(current_input_history.clone());
        run_state.extend_generated_items(normalized_generated_items.clone());
        run_state.extend_session_items(session_generated_items.clone());
        for result in input_guardrail_results.iter().cloned() {
            run_state.record_input_guardrail_result(result);
        }
        for result in output_guardrail_results.iter().cloned() {
            run_state.record_output_guardrail_result(result);
        }
        for result in tool_input_guardrail_results.iter().cloned() {
            run_state.record_tool_input_guardrail_result(result);
        }
        for result in tool_output_guardrail_results.iter().cloned() {
            run_state.record_tool_output_guardrail_result(result);
        }
        for response in raw_responses.iter().cloned() {
            run_state.push_model_response(response);
        }
        if let Some(interruption) = interruptions.first().cloned() {
            run_state.current_step = Some(interruption);
        }
        let trace = trace_manager.finish();
        run_state.set_trace(trace.clone());
        let normalized_result_items = (normalized_generated_items != session_generated_items)
            .then_some(normalized_generated_items.clone());

        Ok(RunResult {
            agent_name: agent.name.clone(),
            last_agent: Some(current_agent),
            input: original_input,
            normalized_input: run_state.normalized_input.clone(),
            new_items: session_generated_items,
            raw_responses,
            output: final_output_items,
            final_output,
            input_guardrail_results,
            output_guardrail_results,
            tool_input_guardrail_results,
            tool_output_guardrail_results,
            context_snapshot: run_state.context_snapshot.clone(),
            run_state: Some(run_state),
            interruptions,
            usage,
            trace: Some(trace),
            conversation_id: conversation_tracker.conversation_id.clone(),
            previous_response_id: conversation_tracker.previous_response_id.clone(),
            auto_previous_response_id: conversation_tracker.auto_previous_response_id,
            reasoning_item_id_policy: self.config.reasoning_item_id_policy,
            normalized_new_items: normalized_result_items,
            agent_tool_invocation: None,
        })
    }

    pub async fn resume(&self, state: &RunState) -> Result<RunResult> {
        if state.remaining_turns() == 0 {
            return Err(MaxTurnsExceeded {
                message: "cannot resume a run state that has exhausted max_turns".to_owned(),
            }
            .into());
        }

        let agent = state.current_agent.clone().ok_or_else(|| UserError {
            message: "cannot resume a run state without a current agent".to_owned(),
        })?;

        let mut resumed_config = self.config.clone();
        resumed_config.max_turns = state.remaining_turns();
        if let Some(trace) = &state.trace {
            resumed_config.workflow_name = trace.workflow_name.clone();
        }
        internal_agent_runner_helpers::apply_resumed_conversation_settings(
            &mut resumed_config,
            state,
        );

        let runner = Self {
            model_provider: self.model_provider.clone(),
            config: resumed_config,
        };

        let mut result = runner.run_items(&agent, state.resume_input()).await?;

        let result_preserve_items = result.new_items.clone();
        let result_normalized_items = result
            .normalized_new_items
            .clone()
            .unwrap_or_else(|| result.new_items.clone());
        let mut merged_new_items = state.session_items.clone();
        merged_new_items.extend(result_preserve_items);
        let mut merged_normalized_items = state.generated_items.clone();
        merged_normalized_items.extend(result_normalized_items);
        result.input = state.original_input.clone();
        result.normalized_input = state.normalized_input.clone();
        result.new_items = merged_new_items;
        result.normalized_new_items =
            (merged_normalized_items != result.new_items).then_some(merged_normalized_items);

        let mut merged_raw_responses = state.model_responses.clone();
        merged_raw_responses.extend(result.raw_responses.clone());
        result.raw_responses = merged_raw_responses;

        let mut merged_input_guardrails = state.input_guardrail_results.clone();
        merged_input_guardrails.extend(result.input_guardrail_results.clone());
        result.input_guardrail_results = merged_input_guardrails;

        let mut merged_output_guardrails = state.output_guardrail_results.clone();
        merged_output_guardrails.extend(result.output_guardrail_results.clone());
        result.output_guardrail_results = merged_output_guardrails;

        let mut merged_tool_input_guardrails = state.tool_input_guardrail_results.clone();
        merged_tool_input_guardrails.extend(result.tool_input_guardrail_results.clone());
        result.tool_input_guardrail_results = merged_tool_input_guardrails;

        let mut merged_tool_output_guardrails = state.tool_output_guardrail_results.clone();
        merged_tool_output_guardrails.extend(result.tool_output_guardrail_results.clone());
        result.tool_output_guardrail_results = merged_tool_output_guardrails;

        if let Some(resumed_state) = result.run_state.as_mut() {
            merge_run_states(state, resumed_state);
            result.context_snapshot = resumed_state.context_snapshot.clone();
            result.conversation_id = resumed_state.conversation_id.clone();
            result.previous_response_id = resumed_state.previous_response_id.clone();
            result.auto_previous_response_id = resumed_state.auto_previous_response_id;
            result.reasoning_item_id_policy = resumed_state.reasoning_item_id_policy;
            result.normalized_input = resumed_state.normalized_input.clone();
        }

        result.trace = state.trace.clone().or(result.trace);

        Ok(result)
    }

    pub async fn resume_with_agent(&self, state: &RunState, agent: &Agent) -> Result<RunResult> {
        let mut rebound_state = state.clone();
        rebound_state.set_current_agent(agent.clone());
        if matches!(
            rebound_state
                .current_step
                .as_ref()
                .and_then(|step| step.kind.clone()),
            Some(RunInterruptionKind::ToolApproval)
        ) {
            return self
                .resume_pending_tool_approval(&rebound_state, agent)
                .await;
        }
        self.resume(&rebound_state).await
    }

    async fn resume_pending_tool_approval(
        &self,
        state: &RunState,
        agent: &Agent,
    ) -> Result<RunResult> {
        let interruption = state.current_step.clone().ok_or_else(|| UserError {
            message: "cannot resume a pending approval without an interruption record".to_owned(),
        })?;
        let call_id = interruption.call_id.clone().ok_or_else(|| UserError {
            message: "cannot resume a pending approval without a tool call id".to_owned(),
        })?;

        let tool_call = internal_tool_execution::find_pending_tool_call(state, &call_id)
            .ok_or_else(|| ModelBehaviorError {
                message: format!("cannot find pending tool call `{call_id}` in run state"),
            })?;
        let approval = state.approval(&call_id).cloned().ok_or_else(|| UserError {
            message: format!("approval decision for `{call_id}` is missing"),
        })?;

        let context = state.restore_context::<crate::run_context::RunContext>()?;
        let tool_outcome = internal_tool_execution::execute_local_function_tools(
            agent,
            &self.config,
            &context,
            vec![tool_call],
            None,
        )
        .await?;
        if !tool_outcome.interruptions.is_empty() {
            return Err(UserError {
                message: "resumed approval unexpectedly produced another interruption".to_owned(),
            }
            .into());
        }

        let mut continued_state = state.clone();
        continued_state.clear_interruption();
        continued_state
            .context_snapshot
            .approvals
            .insert(call_id, approval);
        continued_state.extend_generated_items(tool_outcome.new_items.clone());
        continued_state.extend_session_items(tool_outcome.new_items.clone());
        for result in tool_outcome.input_guardrail_results.iter().cloned() {
            continued_state.record_tool_input_guardrail_result(result);
        }
        for result in tool_outcome.output_guardrail_results.iter().cloned() {
            continued_state.record_tool_output_guardrail_result(result);
        }

        let mut resumed_config = self.config.clone();
        resumed_config.max_turns = continued_state.remaining_turns();
        if let Some(trace) = &continued_state.trace {
            resumed_config.workflow_name = trace.workflow_name.clone();
        }
        internal_agent_runner_helpers::apply_resumed_conversation_settings(
            &mut resumed_config,
            &continued_state,
        );

        let runner = Self {
            model_provider: self.model_provider.clone(),
            config: resumed_config,
        };
        let mut result = runner
            .run_items(agent, continued_state.resume_input())
            .await?;

        let result_preserve_items = result.new_items.clone();
        let result_normalized_items = result
            .normalized_new_items
            .clone()
            .unwrap_or_else(|| result.new_items.clone());
        let mut merged_new_items = continued_state.session_items.clone();
        merged_new_items.extend(result_preserve_items);
        let mut merged_normalized_items = continued_state.generated_items.clone();
        merged_normalized_items.extend(result_normalized_items);
        result.input = continued_state.original_input.clone();
        result.normalized_input = continued_state.normalized_input.clone();
        result.new_items = merged_new_items;
        result.normalized_new_items =
            (merged_normalized_items != result.new_items).then_some(merged_normalized_items);

        let mut merged_raw_responses = continued_state.model_responses.clone();
        merged_raw_responses.extend(result.raw_responses.clone());
        result.raw_responses = merged_raw_responses;

        let mut merged_input_guardrails = continued_state.input_guardrail_results.clone();
        merged_input_guardrails.extend(result.input_guardrail_results.clone());
        result.input_guardrail_results = merged_input_guardrails;

        let mut merged_output_guardrails = continued_state.output_guardrail_results.clone();
        merged_output_guardrails.extend(result.output_guardrail_results.clone());
        result.output_guardrail_results = merged_output_guardrails;

        let mut merged_tool_input_guardrails = continued_state.tool_input_guardrail_results.clone();
        merged_tool_input_guardrails.extend(result.tool_input_guardrail_results.clone());
        result.tool_input_guardrail_results = merged_tool_input_guardrails;

        let mut merged_tool_output_guardrails =
            continued_state.tool_output_guardrail_results.clone();
        merged_tool_output_guardrails.extend(result.tool_output_guardrail_results.clone());
        result.tool_output_guardrail_results = merged_tool_output_guardrails;

        if let Some(resumed_state) = result.run_state.as_mut() {
            merge_run_states(&continued_state, resumed_state);
            result.context_snapshot = resumed_state.context_snapshot.clone();
            result.conversation_id = resumed_state.conversation_id.clone();
            result.previous_response_id = resumed_state.previous_response_id.clone();
            result.auto_previous_response_id = resumed_state.auto_previous_response_id;
            result.reasoning_item_id_policy = resumed_state.reasoning_item_id_policy;
            result.normalized_input = resumed_state.normalized_input.clone();
        }

        result.trace = continued_state.trace.clone().or(result.trace);
        Ok(result)
    }

    async fn call_model(
        &self,
        agent: &Agent,
        context: &crate::run_context::RunContextWrapper,
        trace_id: Uuid,
        model_data: ModelInputData,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ModelResponse> {
        let provider = get_trace_provider();
        let mut span = get_model_tracing_impl(agent.model.as_deref());
        provider.start_span(&mut span, true);
        dispatch_llm_start(
            self.config.run_hooks.as_ref(),
            agent.hooks.as_ref(),
            context,
            agent,
            model_data.instructions.as_deref(),
            &model_data.input,
        )
        .await;

        let requested_model = self
            .config
            .model
            .clone()
            .or_else(|| internal_turn_preparation::get_model(agent));
        let settings = get_default_model_settings(requested_model.as_deref())
            .resolve(agent.model_settings.as_ref())
            .resolve(self.config.model_settings.as_ref());
        let tools = internal_turn_preparation::get_all_tools(agent, context).await?;

        if let Some(model_provider) = self
            .model_provider
            .clone()
            .or_else(|| self.config.model_provider.clone())
        {
            let request = ModelRequest {
                trace_id: Some(trace_id),
                model: requested_model.clone(),
                instructions: model_data.instructions,
                previous_response_id: previous_response_id.map(ToOwned::to_owned),
                conversation_id: conversation_id.map(ToOwned::to_owned),
                settings: settings.clone(),
                input: model_data.input,
                tools,
            };
            let response = model_provider
                .resolve_with_settings(requested_model.as_deref(), &settings)
                .generate(request)
                .await;
            match response {
                Ok(response) => {
                    dispatch_llm_end(
                        self.config.run_hooks.as_ref(),
                        agent.hooks.as_ref(),
                        context,
                        agent,
                        &response,
                    )
                    .await;
                    if let SpanData::Generation(data) = &mut span.data {
                        data.model = response.model.clone().or(requested_model);
                        data.usage = serde_json::to_value(&response.usage).ok();
                    }
                    provider.finish_span(&mut span, true);
                    Ok(response)
                }
                Err(error) => {
                    span.set_error(error.to_string(), None);
                    provider.finish_span(&mut span, true);
                    Err(error)
                }
            }
        } else {
            let text = model_data
                .input
                .iter()
                .rev()
                .find_map(InputItem::as_text)
                .unwrap_or_default()
                .to_owned();
            let response = ModelResponse {
                model: self.config.model.clone().or_else(|| agent.model.clone()),
                output: vec![OutputItem::Text { text }],
                usage: Usage::default(),
                response_id: None,
                request_id: None,
            };
            dispatch_llm_end(
                self.config.run_hooks.as_ref(),
                agent.hooks.as_ref(),
                context,
                agent,
                &response,
            )
            .await;
            if let SpanData::Generation(data) = &mut span.data {
                data.model = response.model.clone();
                data.usage = serde_json::to_value(&response.usage).ok();
            }
            provider.finish_span(&mut span, true);
            Ok(response)
        }
    }

    async fn call_model_with_retry(
        &self,
        agent: &Agent,
        context: &crate::run_context::RunContextWrapper,
        trace_id: Uuid,
        model_data: ModelInputData,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ModelResponse> {
        internal_model_retry::get_response_with_retry(|| {
            self.call_model(
                agent,
                context,
                trace_id,
                model_data,
                previous_response_id,
                conversation_id,
            )
        })
        .await
    }
}

pub async fn run(agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
    get_default_agent_runner().run(agent, input).await
}

pub async fn run_with_options(
    agent: &Agent,
    input: Vec<InputItem>,
    options: RunOptions,
) -> Result<RunResult> {
    get_default_agent_runner()
        .run_with_options(agent, input, options)
        .await
}

pub async fn run_with_session(
    agent: &Agent,
    input: impl Into<InputItem>,
    session: &(dyn Session + Sync),
) -> Result<RunResult> {
    get_default_agent_runner()
        .run_with_session(agent, input, session)
        .await
}

pub async fn run_streamed(
    agent: &Agent,
    input: impl Into<InputItem>,
) -> Result<RunResultStreaming> {
    get_default_agent_runner().run_streamed(agent, input).await
}

pub async fn run_streamed_with_options(
    agent: &Agent,
    input: Vec<InputItem>,
    options: RunOptions,
) -> Result<RunResultStreaming> {
    get_default_agent_runner()
        .run_streamed_with_options(agent, input, options)
        .await
}

pub fn run_sync(agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
    get_default_agent_runner().run_sync(agent, input)
}

pub fn run_sync_with_options(
    agent: &Agent,
    input: Vec<InputItem>,
    options: RunOptions,
) -> Result<RunResult> {
    get_default_agent_runner().run_sync_with_options(agent, input, options)
}

fn resolve_handoff(
    current_agent: &Agent,
    output: &[OutputItem],
) -> Result<Option<(Handoff, Agent)>> {
    let target = output.iter().find_map(|item| match item {
        OutputItem::Handoff { target_agent } => Some(target_agent.as_str()),
        _ => None,
    });

    let Some(target) = target else {
        return Ok(None);
    };

    let handoff =
        current_agent
            .find_handoff(target)
            .cloned()
            .ok_or_else(|| ModelBehaviorError {
                message: format!(
                    "model requested unknown handoff target `{}` from agent `{}`",
                    target, current_agent.name
                ),
            })?;

    let target_agent = handoff.runtime_agent().cloned().ok_or_else(|| UserError {
        message: format!(
            "handoff target `{}` is not bound to a runtime agent instance",
            target
        ),
    })?;

    Ok(Some((handoff, target_agent)))
}

async fn apply_handoff_transition(
    run_config: &RunConfig,
    handoff: &Handoff,
    input_history: &[InputItem],
    pre_handoff_items: &[RunItem],
    new_step_items: Vec<RunItem>,
) -> Result<HandoffTransition> {
    let input_filter = handoff
        .input_filter
        .clone()
        .or_else(|| run_config.handoff_input_filter.clone());
    let should_nest_history = handoff
        .nest_handoff_history
        .unwrap_or(run_config.nest_handoff_history);

    if let Some(input_filter) = input_filter {
        let filtered = input_filter(HandoffInputData {
            input_history: input_history.to_vec(),
            pre_handoff_items: pre_handoff_items.to_vec(),
            new_items: new_step_items,
            input_items: None,
        })
        .await;

        let session_step_items = filtered.new_items.clone();
        let normalized_step_items = filtered
            .input_items
            .clone()
            .unwrap_or_else(|| session_step_items.clone());
        return Ok(HandoffTransition {
            input_history: filtered.input_history,
            pre_handoff_items: filtered.pre_handoff_items,
            normalized_step_items,
            session_step_items,
        });
    }

    if should_nest_history {
        let nested = nest_handoff_history_with_mapper(
            HandoffInputData {
                input_history: input_history.to_vec(),
                pre_handoff_items: pre_handoff_items.to_vec(),
                new_items: new_step_items,
                input_items: None,
            },
            handoff
                .history_mapper
                .clone()
                .or_else(|| run_config.handoff_history_mapper.clone()),
        );
        let session_step_items = nested.new_items.clone();
        let normalized_step_items = nested
            .input_items
            .clone()
            .unwrap_or_else(|| session_step_items.clone());
        return Ok(HandoffTransition {
            input_history: nested.input_history,
            pre_handoff_items: nested.pre_handoff_items,
            normalized_step_items,
            session_step_items,
        });
    }

    Ok(HandoffTransition {
        input_history: input_history.to_vec(),
        pre_handoff_items: pre_handoff_items.to_vec(),
        normalized_step_items: new_step_items.clone(),
        session_step_items: new_step_items,
    })
}

fn merged_input_guardrails(
    agent: &Agent,
    config: &RunConfig,
) -> Vec<crate::guardrail::InputGuardrail> {
    let mut guardrails = config.input_guardrails.clone().unwrap_or_default();
    guardrails.extend(agent.input_guardrails.clone());
    guardrails
}

fn merged_output_guardrails(
    agent: &Agent,
    config: &RunConfig,
) -> Vec<crate::guardrail::OutputGuardrail> {
    let mut guardrails = config.output_guardrails.clone().unwrap_or_default();
    guardrails.extend(agent.output_guardrails.clone());
    guardrails
}

async fn dispatch_agent_start(
    run_hooks: Option<&SharedRunHooks>,
    agent_hooks: Option<&SharedAgentHooks>,
    context: &crate::run_context::RunContextWrapper,
    agent: &Agent,
    turn: usize,
) {
    let hook_context = crate::run_context::AgentHookContext::new(context.context.clone(), turn);
    if let Some(hooks) = run_hooks {
        hooks.on_agent_start(&hook_context, agent).await;
    }
    if let Some(hooks) = agent_hooks {
        hooks.on_start(&hook_context, agent).await;
    }
}

async fn dispatch_agent_end(
    run_hooks: Option<&SharedRunHooks>,
    agent_hooks: Option<&SharedAgentHooks>,
    context: &crate::run_context::RunContextWrapper,
    agent: &Agent,
    output: Option<&str>,
    turn: usize,
) {
    let hook_context = crate::run_context::AgentHookContext::new(context.context.clone(), turn);
    if let Some(hooks) = run_hooks {
        hooks.on_agent_end(&hook_context, agent, output).await;
    }
    if let Some(hooks) = agent_hooks {
        hooks.on_end(&hook_context, agent, output).await;
    }
}

async fn dispatch_handoff(
    run_hooks: Option<&SharedRunHooks>,
    agent_hooks: Option<&SharedAgentHooks>,
    context: &crate::run_context::RunContextWrapper,
    from_agent: &Agent,
    to_agent: &Agent,
) {
    if let Some(hooks) = run_hooks {
        hooks.on_handoff(context, from_agent, to_agent).await;
    }
    if let Some(hooks) = agent_hooks {
        hooks.on_handoff(context, from_agent, to_agent).await;
    }
}

async fn dispatch_llm_start(
    run_hooks: Option<&SharedRunHooks>,
    agent_hooks: Option<&SharedAgentHooks>,
    context: &crate::run_context::RunContextWrapper,
    agent: &Agent,
    system_prompt: Option<&str>,
    input_items: &[InputItem],
) {
    if let Some(hooks) = run_hooks {
        hooks
            .on_llm_start(context, agent, system_prompt, input_items)
            .await;
    }
    if let Some(hooks) = agent_hooks {
        hooks
            .on_llm_start(context, agent, system_prompt, input_items)
            .await;
    }
}

async fn dispatch_llm_end(
    run_hooks: Option<&SharedRunHooks>,
    agent_hooks: Option<&SharedAgentHooks>,
    context: &crate::run_context::RunContextWrapper,
    agent: &Agent,
    response: &ModelResponse,
) {
    if let Some(hooks) = run_hooks {
        hooks.on_llm_end(context, agent, response).await;
    }
    if let Some(hooks) = agent_hooks {
        hooks.on_llm_end(context, agent, response).await;
    }
}

fn merge_run_states(previous: &RunState, next: &mut RunState) {
    next.current_turn += previous.current_turn;
    next.current_agent = previous
        .current_agent
        .clone()
        .or_else(|| next.current_agent.clone());
    next.original_input = previous.original_input.clone();
    next.normalized_input = previous
        .normalized_input
        .clone()
        .or_else(|| next.normalized_input.clone());

    let mut model_responses = previous.model_responses.clone();
    model_responses.extend(next.model_responses.clone());
    next.model_responses = model_responses;

    let mut generated_items = previous.generated_items.clone();
    generated_items.extend(next.generated_items.clone());
    next.generated_items = generated_items;

    let mut session_items = previous.session_items.clone();
    session_items.extend(next.session_items.clone());
    next.session_items = session_items;

    next.conversation_id = previous
        .conversation_id
        .clone()
        .or(next.conversation_id.clone());
    next.previous_response_id = previous
        .previous_response_id
        .clone()
        .or(next.previous_response_id.clone());
    next.auto_previous_response_id =
        previous.auto_previous_response_id || next.auto_previous_response_id;
    next.reasoning_item_id_policy = previous.reasoning_item_id_policy;

    let mut input_guardrail_results = previous.input_guardrail_results.clone();
    input_guardrail_results.extend(next.input_guardrail_results.clone());
    next.input_guardrail_results = input_guardrail_results;

    let mut output_guardrail_results = previous.output_guardrail_results.clone();
    output_guardrail_results.extend(next.output_guardrail_results.clone());
    next.output_guardrail_results = output_guardrail_results;

    let mut tool_input_guardrail_results = previous.tool_input_guardrail_results.clone();
    tool_input_guardrail_results.extend(next.tool_input_guardrail_results.clone());
    next.tool_input_guardrail_results = tool_input_guardrail_results;

    let mut tool_output_guardrail_results = previous.tool_output_guardrail_results.clone();
    tool_output_guardrail_results.extend(next.tool_output_guardrail_results.clone());
    next.tool_output_guardrail_results = tool_output_guardrail_results;

    next.persisted_item_count += previous.persisted_item_count;
    next.trace = previous.trace.clone().or(next.trace.clone());
    next.context_snapshot.context = previous.context_snapshot.context.clone();
    next.context_snapshot.usage = internal_agent_runner_helpers::merge_usage(
        previous.context_snapshot.usage,
        next.context_snapshot.usage,
    );

    let mut approvals = previous.context_snapshot.approvals.clone();
    approvals.extend(next.context_snapshot.approvals.clone());
    next.context_snapshot.approvals = approvals;
    next.context_snapshot.tool_input = previous
        .context_snapshot
        .tool_input
        .clone()
        .or(next.context_snapshot.tool_input.clone());
    next.context_snapshot.agent_tool_state_scope = previous
        .context_snapshot
        .agent_tool_state_scope
        .clone()
        .or(next.context_snapshot.agent_tool_state_scope.clone());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    use async_trait::async_trait;
    use futures::FutureExt;
    use futures::StreamExt;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::{Value, json};

    use crate::errors::AgentsError;
    use crate::guardrail::{GuardrailFunctionOutput, input_guardrail, output_guardrail};
    use crate::lifecycle::{AgentHooks, RunHooks};
    use crate::model::Model;
    use crate::session::MemorySession;
    use crate::tool::function_tool;

    use super::*;

    #[derive(Clone, Default)]
    struct FakeModel {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl Model for FakeModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            let mut calls = self.calls.lock().expect("fake model lock");
            *calls += 1;

            if *calls == 1 {
                return Ok(ModelResponse {
                    model: request.model,
                    output: vec![
                        OutputItem::Reasoning {
                            text: "need a tool".to_owned(),
                        },
                        OutputItem::ToolCall {
                            call_id: "call-1".to_owned(),
                            tool_name: "search".to_owned(),
                            arguments: json!({"query":"rust"}),
                            namespace: None,
                        },
                    ],
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                    },
                    response_id: Some("resp-1".to_owned()),
                    request_id: Some("req-1".to_owned()),
                });
            }

            let tool_result = request
                .input
                .iter()
                .filter_map(|item| match item {
                    InputItem::Json { value } => Some(value),
                    InputItem::Text { .. } => None,
                })
                .find_map(|value| {
                    value
                        .get("type")
                        .and_then(Value::as_str)
                        .filter(|kind| *kind == "tool_call_output")
                        .and_then(|_| {
                            value
                                .get("output")
                                .and_then(Value::as_object)
                                .and_then(|output| output.get("text"))
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned)
                        })
                })
                .unwrap_or_else(|| "missing".to_owned());

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: format!("final:{tool_result}"),
                }],
                usage: Usage {
                    input_tokens: 12,
                    output_tokens: 6,
                },
                response_id: Some("resp-2".to_owned()),
                request_id: Some("req-2".to_owned()),
            })
        }
    }

    struct FakeProvider {
        model: Arc<FakeModel>,
    }

    impl ModelProvider for FakeProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct RequestCaptureModel {
        previous_response_id: Arc<Mutex<Option<String>>>,
        conversation_id: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl Model for RequestCaptureModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            *self
                .previous_response_id
                .lock()
                .expect("request capture previous response id lock") =
                request.previous_response_id.clone();
            *self
                .conversation_id
                .lock()
                .expect("request capture conversation id lock") = request.conversation_id.clone();

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: "ok".to_owned(),
                }],
                usage: Usage::default(),
                response_id: None,
                request_id: None,
            })
        }
    }

    struct RequestCaptureProvider {
        model: Arc<RequestCaptureModel>,
    }

    impl ModelProvider for RequestCaptureProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct HandoffCaptureModel {
        calls: Arc<Mutex<usize>>,
        second_turn_input: Arc<Mutex<Vec<InputItem>>>,
    }

    #[async_trait]
    impl Model for HandoffCaptureModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            let mut calls = self.calls.lock().expect("handoff capture model lock");
            *calls += 1;

            if *calls == 1 {
                return Ok(ModelResponse {
                    model: request.model,
                    output: vec![OutputItem::Handoff {
                        target_agent: "specialist".to_owned(),
                    }],
                    usage: Usage::default(),
                    response_id: Some("resp-handoff-capture-1".to_owned()),
                    request_id: None,
                });
            }

            *self
                .second_turn_input
                .lock()
                .expect("handoff capture input lock") = request.input.clone();

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: "specialist:done".to_owned(),
                }],
                usage: Usage::default(),
                response_id: Some("resp-handoff-capture-2".to_owned()),
                request_id: None,
            })
        }
    }

    struct HandoffCaptureProvider {
        model: Arc<HandoffCaptureModel>,
    }

    impl ModelProvider for HandoffCaptureProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct FakeHandoffModel {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl Model for FakeHandoffModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            let mut calls = self.calls.lock().expect("fake handoff model lock");
            *calls += 1;

            if *calls == 1 {
                return Ok(ModelResponse {
                    model: request.model,
                    output: vec![OutputItem::Handoff {
                        target_agent: "specialist".to_owned(),
                    }],
                    usage: Usage {
                        input_tokens: 3,
                        output_tokens: 1,
                    },
                    response_id: Some("resp-handoff-1".to_owned()),
                    request_id: None,
                });
            }

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: "specialist:done".to_owned(),
                }],
                usage: Usage {
                    input_tokens: 4,
                    output_tokens: 2,
                },
                response_id: Some("resp-handoff-2".to_owned()),
                request_id: None,
            })
        }
    }

    struct FakeHandoffProvider {
        model: Arc<FakeHandoffModel>,
    }

    impl ModelProvider for FakeHandoffProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct AutoPreviousResponseModel {
        calls: Arc<Mutex<usize>>,
        seen_previous_response_ids: Arc<Mutex<Vec<Option<String>>>>,
    }

    #[async_trait]
    impl Model for AutoPreviousResponseModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            self.seen_previous_response_ids
                .lock()
                .expect("auto previous response capture lock")
                .push(request.previous_response_id.clone());

            let mut calls = self
                .calls
                .lock()
                .expect("auto previous response calls lock");
            *calls += 1;

            if *calls == 1 {
                return Ok(ModelResponse {
                    model: request.model,
                    output: vec![OutputItem::ToolCall {
                        call_id: "call-1".to_owned(),
                        tool_name: "search".to_owned(),
                        arguments: json!({"query":"rust"}),
                        namespace: None,
                    }],
                    usage: Usage::default(),
                    response_id: Some("resp-auto-1".to_owned()),
                    request_id: Some("req-auto-1".to_owned()),
                });
            }

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: "done".to_owned(),
                }],
                usage: Usage::default(),
                response_id: Some("resp-auto-2".to_owned()),
                request_id: Some("req-auto-2".to_owned()),
            })
        }
    }

    struct AutoPreviousResponseProvider {
        model: Arc<AutoPreviousResponseModel>,
    }

    impl ModelProvider for AutoPreviousResponseProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct SessionCaptureModel {
        seen_inputs: Arc<Mutex<Vec<Vec<InputItem>>>>,
    }

    #[async_trait]
    impl Model for SessionCaptureModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            self.seen_inputs
                .lock()
                .expect("session capture inputs lock")
                .push(request.input.clone());

            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::Text {
                    text: "session-ok".to_owned(),
                }],
                usage: Usage::default(),
                response_id: None,
                request_id: None,
            })
        }
    }

    struct SessionCaptureProvider {
        model: Arc<SessionCaptureModel>,
    }

    impl ModelProvider for SessionCaptureProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct LoopingToolModel;

    #[async_trait]
    impl Model for LoopingToolModel {
        async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
            Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: format!("call-{}", request.input.len()),
                    tool_name: "search".to_owned(),
                    arguments: json!({"query":"rust"}),
                    namespace: None,
                }],
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
                response_id: Some(format!("resp-{}", request.input.len())),
                request_id: None,
            })
        }
    }

    struct LoopingToolProvider {
        model: Arc<LoopingToolModel>,
    }

    impl ModelProvider for LoopingToolProvider {
        fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
            self.model.clone()
        }
    }

    #[derive(Clone, Default)]
    struct HookRecorder {
        events: Arc<Mutex<BTreeMap<&'static str, usize>>>,
    }

    impl HookRecorder {
        fn bump(&self, event: &'static str) {
            *self
                .events
                .lock()
                .expect("hook recorder lock")
                .entry(event)
                .or_default() += 1;
        }

        fn count(&self, event: &'static str) -> usize {
            self.events
                .lock()
                .expect("hook recorder lock")
                .get(event)
                .copied()
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl RunHooks for HookRecorder {
        async fn on_llm_start(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _system_prompt: Option<&str>,
            _input_items: &[InputItem],
        ) {
            self.bump("run.llm_start");
        }

        async fn on_llm_end(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _response: &ModelResponse,
        ) {
            self.bump("run.llm_end");
        }

        async fn on_agent_start(
            &self,
            _context: &crate::run_context::AgentHookContext,
            _agent: &Agent,
        ) {
            self.bump("run.agent_start");
        }

        async fn on_agent_end(
            &self,
            _context: &crate::run_context::AgentHookContext,
            _agent: &Agent,
            _output: Option<&str>,
        ) {
            self.bump("run.agent_end");
        }

        async fn on_tool_start(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _tool: &crate::tool::ToolDefinition,
        ) {
            self.bump("run.tool_start");
        }

        async fn on_tool_end(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _tool: &crate::tool::ToolDefinition,
            _result: &str,
        ) {
            self.bump("run.tool_end");
        }
    }

    #[async_trait]
    impl AgentHooks for HookRecorder {
        async fn on_start(&self, _context: &crate::run_context::AgentHookContext, _agent: &Agent) {
            self.bump("agent.start");
        }

        async fn on_end(
            &self,
            _context: &crate::run_context::AgentHookContext,
            _agent: &Agent,
            _output: Option<&str>,
        ) {
            self.bump("agent.end");
        }

        async fn on_tool_start(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _tool: &crate::tool::ToolDefinition,
        ) {
            self.bump("agent.tool_start");
        }

        async fn on_tool_end(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _tool: &crate::tool::ToolDefinition,
            _result: &str,
        ) {
            self.bump("agent.tool_end");
        }

        async fn on_llm_start(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _system_prompt: Option<&str>,
            _input_items: &[InputItem],
        ) {
            self.bump("agent.llm_start");
        }

        async fn on_llm_end(
            &self,
            _context: &crate::run_context::RunContextWrapper,
            _agent: &Agent,
            _response: &ModelResponse,
        ) {
            self.bump("agent.llm_end");
        }
    }

    fn default_runner_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    struct DefaultRunnerReset(AgentRunner);

    impl Drop for DefaultRunnerReset {
        fn drop(&mut self) {
            set_default_agent_runner(Some(self.0.clone()));
        }
    }

    #[derive(Debug, Deserialize, JsonSchema)]
    struct SearchArgs {
        query: String,
    }

    #[tokio::test]
    async fn runner_records_guardrail_results() {
        let agent = Agent::builder("assistant")
            .input_guardrail(input_guardrail(
                "allow-in",
                |_ctx, _agent, input| async move {
                    Ok(GuardrailFunctionOutput::allow(Some(
                        json!({"items": input.len()}),
                    )))
                },
            ))
            .output_guardrail(output_guardrail(
                "allow-out",
                |_ctx, _agent, output| async move {
                    Ok(GuardrailFunctionOutput::allow(Some(
                        json!({"items": output.len()}),
                    )))
                },
            ))
            .build();

        let result = Runner::new()
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(result.input_guardrail_results.len(), 1);
        assert_eq!(result.output_guardrail_results.len(), 1);
        assert!(result.durable_state().is_some());
    }

    #[tokio::test]
    async fn runner_executes_local_function_tools() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("final:result:rust"));
        assert_eq!(result.raw_responses.len(), 2);
        assert!(result.new_items.iter().any(|item| {
            matches!(
                item,
                RunItem::ToolCallOutput {
                    tool_name,
                    call_id,
                    ..
                } if tool_name == "search" && call_id.as_deref() == Some("call-1")
            )
        }));
        assert_eq!(result.usage.input_tokens, 22);
        assert_eq!(result.usage.output_tokens, 11);
    }

    #[tokio::test]
    async fn runner_rewrites_local_tool_outputs_via_tool_input_guardrails() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let invocations = Arc::new(Mutex::new(Vec::new()));
        let seen_queries = invocations.clone();
        let search_tool = function_tool(
            "search",
            "Search documents",
            move |_ctx, args: SearchArgs| {
                let seen_queries = seen_queries.clone();
                async move {
                    seen_queries
                        .lock()
                        .expect("tool invocation tracker lock")
                        .push(args.query.clone());
                    Ok::<_, AgentsError>(format!("result:{}", args.query))
                }
            },
        )
        .expect("function tool should build")
        .with_input_guardrail(crate::tool_guardrails::tool_input_guardrail(
            "sanitize",
            |_data| async move {
                Ok(
                    crate::tool_guardrails::ToolGuardrailFunctionOutput::reject_content(
                        "guarded:search",
                        Some(json!({"reason":"blocked"})),
                    ),
                )
            },
        ));
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(
            invocations
                .lock()
                .expect("tool invocation tracker lock")
                .len(),
            0
        );
        assert_eq!(result.final_output.as_deref(), Some("final:guarded:search"));
        assert_eq!(result.tool_input_guardrail_results.len(), 1);
        assert_eq!(
            result.tool_input_guardrail_results[0]
                .output
                .rejection_message(),
            Some("guarded:search")
        );
        assert!(result.new_items.iter().any(|item| {
            matches!(
                item,
                RunItem::ToolCallOutput {
                    tool_name,
                    output: OutputItem::Text { text },
                    call_id,
                    ..
                } if tool_name == "search"
                    && text == "guarded:search"
                    && call_id.as_deref() == Some("call-1")
            )
        }));
    }

    #[tokio::test]
    async fn runner_surfaces_tool_output_guardrail_tripwire_failures() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let invocations = Arc::new(Mutex::new(0usize));
        let invocation_count = invocations.clone();
        let search_tool = function_tool(
            "search",
            "Search documents",
            move |_ctx, args: SearchArgs| {
                let invocation_count = invocation_count.clone();
                async move {
                    *invocation_count
                        .lock()
                        .expect("tool invocation counter lock") += 1;
                    Ok::<_, AgentsError>(format!("result:{}", args.query))
                }
            },
        )
        .expect("function tool should build")
        .with_output_guardrail(crate::tool_guardrails::tool_output_guardrail(
            "explode",
            |_data| async move {
                Ok(
                    crate::tool_guardrails::ToolGuardrailFunctionOutput::raise_exception(Some(
                        json!({"guardrail":"explode"}),
                    )),
                )
            },
        ));
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let error = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect_err("tripwire should fail");

        assert_eq!(
            *invocations.lock().expect("tool invocation counter lock"),
            1
        );
        match error {
            AgentsError::ToolOutputGuardrailTripwire(error) => {
                assert_eq!(error.guardrail_name, "explode");
                assert_eq!(
                    error.output.output_info,
                    Some(json!({"guardrail":"explode"}))
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn runner_propagates_input_tripwire() {
        let agent = Agent::builder("assistant")
            .input_guardrail(input_guardrail(
                "block",
                |_ctx, _agent, _input| async move { Ok(GuardrailFunctionOutput::tripwire(None)) },
            ))
            .build();

        let error = Runner::new()
            .run(&agent, "hello")
            .await
            .expect_err("tripwire should fail");

        assert!(matches!(error, AgentsError::InputGuardrailTripwire(_)));
    }

    #[tokio::test]
    async fn runner_propagates_output_tripwire() {
        let agent = Agent::builder("assistant")
            .output_guardrail(output_guardrail(
                "block",
                |_ctx, _agent, _output| async move { Ok(GuardrailFunctionOutput::tripwire(None)) },
            ))
            .build();

        let error = Runner::new()
            .run(&agent, "hello")
            .await
            .expect_err("tripwire should fail");

        assert!(matches!(error, AgentsError::OutputGuardrailTripwire(_)));
    }

    #[tokio::test]
    async fn runner_resumes_from_run_state() {
        let agent = Agent::builder("assistant").build();
        let first = Runner::new()
            .run(&agent, "hello")
            .await
            .expect("first run should succeed");
        let state = first
            .durable_state()
            .cloned()
            .expect("first run should expose state");

        let resumed = Runner::new()
            .resume(&state)
            .await
            .expect("resume should succeed");

        assert_eq!(resumed.input[0].as_text(), Some("hello"));
        assert_eq!(resumed.new_items.len(), 2);
        assert_eq!(
            resumed
                .durable_state()
                .and_then(|state| state.current_agent_name()),
            Some("assistant")
        );
        assert_eq!(
            resumed.durable_state().map(|state| state.current_turn),
            Some(2)
        );
    }

    #[tokio::test]
    async fn runner_can_resume_with_reattached_runtime_agent() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let first = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("initial run should succeed");
        let state_json = first
            .durable_state()
            .expect("state should exist")
            .to_json_string()
            .expect("state should serialize");
        let restored_state =
            RunState::from_json_str(&state_json).expect("state should deserialize");

        let failing_resume = Runner::new()
            .with_model_provider(Arc::new(FakeProvider {
                model: Arc::new(FakeModel::default()),
            }))
            .resume(&restored_state)
            .await
            .expect_err("resume without runtime agent should fail");
        assert!(matches!(failing_resume, AgentsError::ModelBehavior(_)));

        let resumed = Runner::new()
            .with_model_provider(Arc::new(FakeProvider {
                model: Arc::new(FakeModel::default()),
            }))
            .resume_with_agent(&restored_state, &agent)
            .await
            .expect("resume with attached agent should succeed");

        assert_eq!(resumed.final_output.as_deref(), Some("final:result:rust"));
        assert!(
            resumed
                .durable_state()
                .and_then(|state| state.current_agent_name())
                .is_some()
        );
    }

    #[tokio::test]
    async fn runner_interrupts_and_resumes_tool_approval() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build")
        .with_needs_approval(true);
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let initial = Runner::new()
            .with_model_provider(provider.clone())
            .run(&agent, "hello")
            .await
            .expect("initial run should succeed");

        assert!(initial.final_output.is_none());
        assert_eq!(initial.interruptions.len(), 1);
        assert!(matches!(
            initial
                .interruptions
                .first()
                .and_then(|step| step.kind.clone()),
            Some(RunInterruptionKind::ToolApproval)
        ));

        let mut state = initial
            .durable_state()
            .cloned()
            .expect("state should exist");
        state.approve("call-1", Some("approved".to_owned()));

        let resumed = Runner::new()
            .with_model_provider(provider)
            .resume_with_agent(&state, &agent)
            .await
            .expect("resume should succeed");

        assert_eq!(resumed.final_output.as_deref(), Some("final:result:rust"));
        assert!(resumed.new_items.iter().any(|item| {
            matches!(
                item,
                RunItem::ToolCallOutput {
                    tool_name,
                    call_id,
                    ..
                } if tool_name == "search" && call_id.as_deref() == Some("call-1")
            )
        }));
    }

    #[tokio::test]
    async fn runner_follows_runtime_handoffs() {
        let provider = Arc::new(FakeHandoffProvider {
            model: Arc::new(FakeHandoffModel::default()),
        });
        let specialist = Agent::builder("specialist")
            .instructions("Handle specialist tasks.")
            .build();
        let agent = Agent::builder("assistant")
            .handoff_to_agent(specialist)
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("handoff run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("specialist:done"));
        assert_eq!(
            result.last_agent().map(|agent| agent.name.as_str()),
            Some("specialist")
        );
        assert!(result.new_items.iter().any(|item| {
            matches!(
                item,
                RunItem::HandoffCall { target_agent } if target_agent == "specialist"
            )
        }));
        assert!(result.new_items.iter().any(|item| {
            matches!(
                item,
                RunItem::HandoffOutput { source_agent } if source_agent == "assistant"
            )
        }));
    }

    #[tokio::test]
    async fn runner_applies_handoff_input_filter_to_normalized_history() {
        let model = Arc::new(HandoffCaptureModel::default());
        let provider = Arc::new(HandoffCaptureProvider {
            model: model.clone(),
        });
        let specialist = Agent::builder("specialist")
            .instructions("Handle specialist tasks.")
            .build();
        let handoff = Handoff::to_agent(specialist).with_input_filter(Arc::new(|_data| {
            async move {
                HandoffInputData {
                    input_history: vec![InputItem::from("filtered-history")],
                    pre_handoff_items: vec![],
                    new_items: vec![RunItem::HandoffOutput {
                        source_agent: "assistant".to_owned(),
                    }],
                    input_items: Some(vec![RunItem::MessageOutput {
                        content: OutputItem::Text {
                            text: "filtered-item".to_owned(),
                        },
                    }]),
                }
            }
            .boxed()
        }));
        let agent = Agent::builder("assistant").handoff(handoff).build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("handoff run should succeed");

        let second_turn_input = model
            .second_turn_input
            .lock()
            .expect("handoff second turn input lock")
            .clone();
        assert_eq!(second_turn_input.len(), 2);
        assert_eq!(second_turn_input[0].as_text(), Some("filtered-history"));
        assert_eq!(second_turn_input[1].as_text(), Some("filtered-item"));
        let preserve_all = result.to_input_list_mode(crate::result::ToInputListMode::PreserveAll);
        let normalized = result.to_input_list_mode(crate::result::ToInputListMode::Normalized);
        assert_ne!(preserve_all, normalized);
        assert_eq!(
            normalized.first().and_then(InputItem::as_text),
            Some("filtered-history")
        );
        assert_eq!(
            result
                .durable_state()
                .and_then(|state| state.normalized_input.clone())
                .and_then(|items| items.first().cloned())
                .and_then(|item| item.as_text().map(ToOwned::to_owned))
                .as_deref(),
            Some("filtered-history")
        );
    }

    #[tokio::test]
    async fn runner_nests_handoff_history_for_next_agent_turn() {
        let model = Arc::new(HandoffCaptureModel::default());
        let provider = Arc::new(HandoffCaptureProvider {
            model: model.clone(),
        });
        let specialist = Agent::builder("specialist").build();
        let agent = Agent::builder("assistant")
            .handoff(Handoff::to_agent(specialist).with_nest_handoff_history(true))
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run(&agent, "hello")
            .await
            .expect("handoff run should succeed");

        let second_turn_input = model
            .second_turn_input
            .lock()
            .expect("handoff second turn input lock")
            .clone();
        let nested_history = second_turn_input
            .first()
            .expect("nested history should exist");
        let nested_content = match nested_history {
            InputItem::Json { value } => value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            InputItem::Text { .. } => String::new(),
        };

        assert!(nested_content.contains("<CONVERSATION HISTORY>"));
        assert!(nested_content.contains("hello"));
        assert_eq!(
            result
                .to_input_list_mode(crate::result::ToInputListMode::Normalized)
                .first()
                .map(|item| matches!(item, InputItem::Json { .. })),
            Some(true)
        );
    }

    #[tokio::test]
    async fn runner_passes_conversation_tracking_to_model_requests() {
        let model = Arc::new(RequestCaptureModel::default());
        let provider = Arc::new(RequestCaptureProvider {
            model: model.clone(),
        });
        let agent = Agent::builder("assistant").build();
        let runner = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                previous_response_id: Some("resp_123".to_owned()),
                conversation_id: Some("conv_123".to_owned()),
                ..RunConfig::default()
            });

        let result = runner
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("ok"));
        assert_eq!(
            model
                .previous_response_id
                .lock()
                .expect("previous response capture lock")
                .as_deref(),
            Some("resp_123")
        );
        assert_eq!(
            model
                .conversation_id
                .lock()
                .expect("conversation capture lock")
                .as_deref(),
            Some("conv_123")
        );
        assert_eq!(
            result
                .durable_state()
                .and_then(|state| state.previous_response_id.as_deref()),
            Some("resp_123")
        );
        assert_eq!(
            result
                .durable_state()
                .and_then(|state| state.conversation_id.as_deref()),
            Some("conv_123")
        );
    }

    #[tokio::test]
    async fn runner_run_with_options_overrides_config_for_a_single_call() {
        let model = Arc::new(RequestCaptureModel::default());
        let provider = Arc::new(RequestCaptureProvider {
            model: model.clone(),
        });
        let agent = Agent::builder("assistant").build();

        let result = Runner::new()
            .with_config(RunConfig {
                previous_response_id: Some("resp-base".to_owned()),
                conversation_id: Some("conv-base".to_owned()),
                ..RunConfig::default()
            })
            .run_with_options(
                &agent,
                vec![InputItem::from("hello")],
                RunOptions {
                    max_turns: Some(1),
                    previous_response_id: Some("resp-options".to_owned()),
                    conversation_id: Some("conv-options".to_owned()),
                    model_provider: Some(provider),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("options-backed run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("ok"));
        assert_eq!(
            model
                .previous_response_id
                .lock()
                .expect("previous response capture lock")
                .as_deref(),
            Some("resp-options")
        );
        assert_eq!(
            model
                .conversation_id
                .lock()
                .expect("conversation capture lock")
                .as_deref(),
            Some("conv-options")
        );
        assert_eq!(result.durable_state().map(|state| state.max_turns), Some(1));
    }

    #[tokio::test]
    async fn runner_run_with_options_does_not_mutate_shared_defaults() {
        let model = Arc::new(RequestCaptureModel::default());
        let provider = Arc::new(RequestCaptureProvider {
            model: model.clone(),
        });
        let agent = Agent::builder("assistant").build();
        let runner = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                previous_response_id: Some("resp-base".to_owned()),
                conversation_id: Some("conv-base".to_owned()),
                ..RunConfig::default()
            });

        runner
            .run_with_options(
                &agent,
                vec![InputItem::from("override")],
                RunOptions {
                    previous_response_id: Some("resp-override".to_owned()),
                    conversation_id: Some("conv-override".to_owned()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("override-backed run should succeed");

        runner
            .run(&agent, "base")
            .await
            .expect("follow-up run should succeed");

        assert_eq!(
            model
                .previous_response_id
                .lock()
                .expect("previous response capture lock")
                .as_deref(),
            Some("resp-base")
        );
        assert_eq!(
            model
                .conversation_id
                .lock()
                .expect("conversation capture lock")
                .as_deref(),
            Some("conv-base")
        );
        assert_eq!(
            runner.config.previous_response_id.as_deref(),
            Some("resp-base")
        );
        assert_eq!(runner.config.conversation_id.as_deref(), Some("conv-base"));
    }

    #[tokio::test]
    async fn runner_advances_auto_previous_response_id_across_turns() {
        let model = Arc::new(AutoPreviousResponseModel::default());
        let provider = Arc::new(AutoPreviousResponseProvider {
            model: model.clone(),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                auto_previous_response_id: true,
                ..RunConfig::default()
            })
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("done"));
        assert_eq!(
            model
                .seen_previous_response_ids
                .lock()
                .expect("auto previous response ids lock")
                .as_slice(),
            &[None, Some("resp-auto-1".to_owned())]
        );
        assert_eq!(result.previous_response_id(), Some("resp-auto-2"));
        assert_eq!(
            result
                .durable_state()
                .and_then(|state| state.previous_response_id.as_deref()),
            Some("resp-auto-2")
        );
    }

    #[tokio::test]
    async fn runner_uses_session_history_and_persists_new_turn() {
        let model = Arc::new(SessionCaptureModel::default());
        let provider = Arc::new(SessionCaptureProvider {
            model: model.clone(),
        });
        let session = MemorySession::new("session");
        session
            .add_items(vec![InputItem::from("history")])
            .await
            .expect("history should be stored");
        let agent = Agent::builder("assistant").build();

        let result = Runner::new()
            .with_model_provider(provider)
            .run_with_session(&agent, "hello", &session)
            .await
            .expect("session-backed run should succeed");

        let seen_inputs = model.seen_inputs.lock().expect("session seen inputs lock");
        assert_eq!(seen_inputs.len(), 1);
        assert_eq!(seen_inputs[0][0].as_text(), Some("history"));
        assert_eq!(seen_inputs[0][1].as_text(), Some("hello"));
        drop(seen_inputs);

        let persisted = session
            .get_items()
            .await
            .expect("session items should load");
        assert_eq!(persisted.len(), 3);
        assert_eq!(persisted[0].as_text(), Some("history"));
        assert_eq!(persisted[1].as_text(), Some("hello"));
        assert_eq!(persisted[2].as_text(), Some("session-ok"));
        assert_eq!(result.final_output.as_deref(), Some("session-ok"));
        assert_eq!(
            result
                .durable_state()
                .map(|state| state.persisted_item_count),
            Some(2)
        );
    }

    #[tokio::test]
    async fn runner_rejects_session_persistence_with_conversation_settings() {
        let session = MemorySession::new("session");
        let agent = Agent::builder("assistant").build();
        let error = Runner::new()
            .with_config(RunConfig {
                previous_response_id: Some("resp_123".to_owned()),
                ..RunConfig::default()
            })
            .run_with_session(&agent, "hello", &session)
            .await
            .expect_err("session with previous response id should fail");

        assert!(matches!(error, AgentsError::User(_)));
    }

    #[tokio::test]
    async fn runner_rejects_session_persistence_with_conversation_id() {
        let session = MemorySession::new("session");
        let agent = Agent::builder("assistant").build();
        let error = Runner::new()
            .with_config(RunConfig {
                conversation_id: Some("conv_123".to_owned()),
                ..RunConfig::default()
            })
            .run_with_session(&agent, "hello", &session)
            .await
            .expect_err("session with conversation id should fail");

        assert!(matches!(error, AgentsError::User(_)));
    }

    #[tokio::test]
    async fn runner_rejects_session_persistence_with_auto_previous_response_id() {
        let session = MemorySession::new("session");
        let agent = Agent::builder("assistant").build();
        let error = Runner::new()
            .with_config(RunConfig {
                auto_previous_response_id: true,
                ..RunConfig::default()
            })
            .run_with_session(&agent, "hello", &session)
            .await
            .expect_err("session with auto previous response id should fail");

        assert!(matches!(error, AgentsError::User(_)));
    }

    #[tokio::test]
    async fn runner_streamed_matches_non_streamed_result() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let runner = Runner::new().with_model_provider(provider);
        let streamed = runner
            .run_streamed(&agent, "hello")
            .await
            .expect("streamed run should succeed");
        let events = streamed.stream_events().collect::<Vec<_>>().await;
        let final_result = streamed
            .wait_for_completion()
            .await
            .expect("streamed run should finish");

        assert!(!streamed.is_complete);
        assert_eq!(streamed.current_turn, 0);
        assert_eq!(
            final_result.final_output.as_deref(),
            Some("final:result:rust")
        );
        assert_eq!(final_result.raw_responses.len(), 2);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, crate::stream_events::StreamEvent::RunItemEvent(_)))
        );
    }

    #[tokio::test]
    async fn default_runner_roundtrip_and_free_run_use_global_runner() {
        let _guard = default_runner_test_lock().lock().await;
        let original_runner = get_default_agent_runner();
        let _reset = DefaultRunnerReset(original_runner.clone());
        let configured_runner = Runner::new().with_config(RunConfig {
            model: Some("gpt-default-runner".to_owned()),
            ..RunConfig::default()
        });

        set_default_agent_runner(Some(configured_runner.clone()));

        let current_runner = get_default_agent_runner();
        assert_eq!(
            current_runner.config.model.as_deref(),
            Some("gpt-default-runner")
        );

        let agent = Agent::builder("assistant").build();
        let result = run(&agent, "hello")
            .await
            .expect("free run should use the configured default runner");

        assert_eq!(
            result
                .raw_responses
                .first()
                .and_then(|response| response.model.as_deref()),
            Some("gpt-default-runner")
        );
    }

    #[tokio::test]
    async fn run_sync_errors_when_runtime_is_already_running() {
        let agent = Agent::builder("assistant").build();

        let error = Runner::new()
            .run_sync(&agent, "hello")
            .expect_err("run_sync should reject active runtimes");

        assert!(matches!(error, AgentsError::User(_)));
        assert!(
            error.to_string().contains("event loop is already running"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn runner_uses_session_input_callback_to_prepare_history() {
        let model = Arc::new(SessionCaptureModel::default());
        let provider = Arc::new(SessionCaptureProvider {
            model: model.clone(),
        });
        let session = MemorySession::new("session");
        session
            .add_items(vec![
                InputItem::from("history-1"),
                InputItem::from("history-2"),
            ])
            .await
            .expect("history should be stored");
        let agent = Agent::builder("assistant").build();

        let result = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                session_input_callback: Some(Arc::new(|history, new_items| {
                    async move {
                        Ok(vec![
                            history
                                .last()
                                .cloned()
                                .unwrap_or_else(|| InputItem::from("")),
                            new_items
                                .first()
                                .cloned()
                                .unwrap_or_else(|| InputItem::from("")),
                        ])
                    }
                    .boxed()
                })),
                ..RunConfig::default()
            })
            .run_with_session(&agent, "hello", &session)
            .await
            .expect("session-backed run should succeed");

        let seen_inputs = model.seen_inputs.lock().expect("session seen inputs lock");
        assert_eq!(seen_inputs.len(), 1);
        assert_eq!(seen_inputs[0].len(), 2);
        assert_eq!(seen_inputs[0][0].as_text(), Some("history-2"));
        assert_eq!(seen_inputs[0][1].as_text(), Some("hello"));
        drop(seen_inputs);
        let persisted = session
            .get_items()
            .await
            .expect("session items should load");
        assert_eq!(persisted.len(), 4);
        assert_eq!(persisted[0].as_text(), Some("history-1"));
        assert_eq!(persisted[1].as_text(), Some("history-2"));
        assert_eq!(persisted[2].as_text(), Some("hello"));
        assert_eq!(persisted[3].as_text(), Some("session-ok"));
        assert_eq!(result.final_output.as_deref(), Some("session-ok"));
    }

    #[tokio::test]
    async fn runner_fires_run_and_agent_hooks_for_llm_and_tool_events() {
        let provider = Arc::new(FakeProvider {
            model: Arc::new(FakeModel::default()),
        });
        let search_tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
        let run_hooks = Arc::new(HookRecorder::default());
        let agent_hooks = Arc::new(HookRecorder::default());
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .hooks(agent_hooks.clone())
            .build();

        let result = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                run_hooks: Some(run_hooks.clone()),
                ..RunConfig::default()
            })
            .run(&agent, "hello")
            .await
            .expect("run should succeed");

        assert_eq!(result.final_output.as_deref(), Some("final:result:rust"));
        assert_eq!(run_hooks.count("run.agent_start"), 1);
        assert_eq!(run_hooks.count("run.agent_end"), 1);
        assert_eq!(run_hooks.count("run.llm_start"), 2);
        assert_eq!(run_hooks.count("run.llm_end"), 2);
        assert_eq!(run_hooks.count("run.tool_start"), 1);
        assert_eq!(run_hooks.count("run.tool_end"), 1);

        assert_eq!(agent_hooks.count("agent.start"), 1);
        assert_eq!(agent_hooks.count("agent.end"), 1);
        assert_eq!(agent_hooks.count("agent.llm_start"), 2);
        assert_eq!(agent_hooks.count("agent.llm_end"), 2);
        assert_eq!(agent_hooks.count("agent.tool_start"), 1);
        assert_eq!(agent_hooks.count("agent.tool_end"), 1);
    }

    #[tokio::test]
    async fn runner_uses_max_turn_handler_for_terminal_output() {
        let provider = Arc::new(LoopingToolProvider {
            model: Arc::new(LoopingToolModel),
        });
        let search_tool =
            function_tool(
                "search",
                "Search documents",
                |_ctx, args: SearchArgs| async move {
                    Ok::<_, AgentsError>(format!("loop:{}", args.query))
                },
            )
            .expect("function tool should build");
        let agent = Agent::builder("assistant")
            .function_tool(search_tool)
            .build();

        let runner = Runner::new()
            .with_model_provider(provider)
            .with_config(RunConfig {
                max_turns: 2,
                run_error_handlers: crate::run_error_handlers::RunErrorHandlers {
                    max_turns: Some(Arc::new(|input| {
                        async move {
                            assert_eq!(input.run_data.last_agent.name, "assistant");
                            Some(crate::run_error_handlers::RunErrorHandlerResult {
                                final_output: json!({"status":"max_turns","turns":2}),
                                include_in_history: true,
                            })
                        }
                        .boxed()
                    })),
                },
                ..RunConfig::default()
            });

        let result = runner
            .run(&agent, "hello")
            .await
            .expect("run should resolve through the max-turn handler");

        assert!(
            result
                .final_output
                .as_deref()
                .is_some_and(|value| value.contains("max_turns"))
        );
        assert!(
            result
                .output
                .iter()
                .any(|item| matches!(item, OutputItem::Json { .. }))
        );
    }
}
