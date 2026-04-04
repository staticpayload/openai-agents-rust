use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use crate::agent::Agent;
use crate::errors::Result;
use crate::exceptions::{
    MaxTurnsExceeded, ModelBehaviorError, ToolInputGuardrailTripwireTriggered,
    ToolOutputGuardrailTripwireTriggered, UserError,
};
use crate::internal::guardrails as internal_guardrails;
use crate::internal::items as internal_items;
use crate::internal::turn_resolution as internal_turn_resolution;
use crate::items::{InputItem, OutputItem, RunItem};
use crate::model::{ModelProvider, ModelRequest, ModelResponse};
use crate::result::RunResult;
use crate::run_config::{DEFAULT_MAX_TURNS, RunConfig};
use crate::run_state::{RunInterruption, RunInterruptionKind, RunState};
use crate::tool::{Tool, ToolOutput};
use crate::tool_context::{ToolCall, ToolContext};
use crate::tool_guardrails::{
    ToolGuardrailBehavior, ToolInputGuardrailResult, ToolOutputGuardrailResult,
};
use crate::tracing::Trace;
use crate::usage::Usage;

const DEFAULT_APPROVAL_REJECTION_MESSAGE: &str = "Tool execution was not approved.";

/// Entry point for executing agents.
#[derive(Clone, Default)]
pub struct Runner {
    model_provider: Option<Arc<dyn ModelProvider>>,
    config: RunConfig,
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

    pub async fn run(&self, agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
        self.run_items(agent, vec![input.into()]).await
    }

    pub async fn run_items(&self, agent: &Agent, input: Vec<InputItem>) -> Result<RunResult> {
        let trace = Trace {
            id: Uuid::new_v4(),
            workflow_name: if self.config.workflow_name.is_empty() {
                agent.name.clone()
            } else {
                self.config.workflow_name.clone()
            },
        };

        let mut context = internal_guardrails::new_run_context(trace.workflow_name.clone());
        context.turn_input = input.clone();

        let input_guardrail_results = internal_guardrails::run_input_guardrails(
            agent,
            &agent.input_guardrails,
            &input,
            &context,
        )
        .await?;

        let mut current_agent = agent.clone();
        let mut generated_items = Vec::new();
        let mut raw_responses = Vec::new();
        let mut usage = Usage::default();
        let mut output_guardrail_results = Vec::new();
        let mut tool_input_guardrail_results = Vec::new();
        let mut tool_output_guardrail_results = Vec::new();
        let mut interruptions = Vec::new();
        let mut final_output = None;
        let mut final_output_items = Vec::new();

        for _turn in 0..self.config.max_turns {
            let prepared_input = internal_items::prepare_model_input_items(
                &input,
                &generated_items,
                self.config.reasoning_item_id_policy,
            );

            let response = self
                .call_model(&current_agent, trace.id, &prepared_input)
                .await?;
            usage = merge_usage(usage, response.usage);
            context.usage = usage;

            let output = response.output.clone();
            let response_items = internal_turn_resolution::build_message_output_items(&output);
            generated_items.extend(response_items);
            raw_responses.push(response);

            if let Some(target_agent) = resolve_handoff_agent(&current_agent, &output)? {
                generated_items.push(RunItem::HandoffOutput {
                    source_agent: current_agent.name.clone(),
                });
                current_agent = target_agent;
                continue;
            }

            let tool_calls = extract_tool_calls(&output);
            if tool_calls.is_empty() {
                output_guardrail_results = internal_guardrails::run_output_guardrails(
                    &current_agent,
                    &current_agent.output_guardrails,
                    &output,
                    &context,
                )
                .await?;
                final_output = internal_turn_resolution::extract_final_output_text(&output);
                final_output_items = output;
                break;
            }

            let tool_outcome =
                execute_local_function_tools(&current_agent, &self.config, &context, tool_calls)
                    .await?;
            tool_input_guardrail_results.extend(tool_outcome.input_guardrail_results);
            tool_output_guardrail_results.extend(tool_outcome.output_guardrail_results);
            generated_items.extend(tool_outcome.new_items);
            if !tool_outcome.interruptions.is_empty() {
                interruptions = tool_outcome.interruptions;
                break;
            }
        }

        if final_output_items.is_empty() && final_output.is_none() && interruptions.is_empty() {
            return Err(MaxTurnsExceeded {
                message: format!(
                    "run for agent `{}` exceeded max_turns ({}) before producing a final output",
                    agent.name, self.config.max_turns
                ),
            }
            .into());
        }

        let mut run_state = RunState::new(
            &context,
            input.clone(),
            agent.clone(),
            self.config.max_turns,
        )?;
        run_state.current_turn = raw_responses.len();
        run_state.set_current_agent(current_agent.clone());
        run_state.set_trace(trace.clone());
        run_state.conversation_id = self.config.conversation_id.clone();
        run_state.previous_response_id = self.config.previous_response_id.clone();
        run_state.auto_previous_response_id = self.config.auto_previous_response_id;
        run_state.extend_generated_items(generated_items.clone());
        run_state.extend_session_items(generated_items.clone());
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

        Ok(RunResult {
            agent_name: agent.name.clone(),
            last_agent: Some(current_agent),
            input,
            new_items: generated_items,
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
        if resumed_config.previous_response_id.is_none() {
            resumed_config.previous_response_id = state.previous_response_id.clone();
        }
        if resumed_config.conversation_id.is_none() {
            resumed_config.conversation_id = state.conversation_id.clone();
        }
        resumed_config.auto_previous_response_id |= state.auto_previous_response_id;

        let runner = Self {
            model_provider: self.model_provider.clone(),
            config: resumed_config,
        };

        let mut result = runner.run_items(&agent, state.resume_input()).await?;

        let mut merged_new_items = state.generated_items.clone();
        merged_new_items.extend(result.new_items.clone());
        result.input = state.original_input.clone();
        result.new_items = merged_new_items;

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

        let tool_call =
            find_pending_tool_call(state, &call_id).ok_or_else(|| ModelBehaviorError {
                message: format!("cannot find pending tool call `{call_id}` in run state"),
            })?;
        let approval = state.approval(&call_id).cloned().ok_or_else(|| UserError {
            message: format!("approval decision for `{call_id}` is missing"),
        })?;

        let context = state.restore_context::<crate::run_context::RunContext>()?;
        let tool_outcome =
            execute_local_function_tools(agent, &self.config, &context, vec![tool_call]).await?;
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
        if resumed_config.previous_response_id.is_none() {
            resumed_config.previous_response_id = continued_state.previous_response_id.clone();
        }
        if resumed_config.conversation_id.is_none() {
            resumed_config.conversation_id = continued_state.conversation_id.clone();
        }
        resumed_config.auto_previous_response_id |= continued_state.auto_previous_response_id;

        let runner = Self {
            model_provider: self.model_provider.clone(),
            config: resumed_config,
        };
        let mut result = runner
            .run_items(agent, continued_state.resume_input())
            .await?;

        let mut merged_new_items = continued_state.generated_items.clone();
        merged_new_items.extend(result.new_items.clone());
        result.input = continued_state.original_input.clone();
        result.new_items = merged_new_items;

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
        }

        result.trace = continued_state.trace.clone().or(result.trace);
        Ok(result)
    }

    async fn call_model(
        &self,
        agent: &Agent,
        trace_id: Uuid,
        prepared_input: &[InputItem],
    ) -> Result<ModelResponse> {
        if let Some(model_provider) = &self.model_provider {
            let request = ModelRequest {
                trace_id: Some(trace_id),
                model: agent.model.clone(),
                instructions: agent.instructions.clone(),
                previous_response_id: self.config.previous_response_id.clone(),
                conversation_id: self.config.conversation_id.clone(),
                input: prepared_input.to_vec(),
                tools: agent.tool_definitions(),
            };
            model_provider
                .resolve(agent.model.as_deref())
                .generate(request)
                .await
        } else {
            let text = prepared_input
                .iter()
                .rev()
                .find_map(InputItem::as_text)
                .unwrap_or_default()
                .to_owned();
            Ok(ModelResponse {
                model: agent.model.clone(),
                output: vec![OutputItem::Text { text }],
                usage: Usage::default(),
            })
        }
    }
}

pub async fn run(agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
    Runner::new().run(agent, input).await
}

struct ToolExecutionOutcome {
    new_items: Vec<RunItem>,
    input_guardrail_results: Vec<ToolInputGuardrailResult>,
    output_guardrail_results: Vec<ToolOutputGuardrailResult>,
    interruptions: Vec<RunInterruption>,
}

async fn execute_local_function_tools(
    agent: &Agent,
    run_config: &RunConfig,
    context: &crate::run_context::RunContextWrapper,
    tool_calls: Vec<ToolCall>,
) -> Result<ToolExecutionOutcome> {
    let mut new_items = Vec::new();
    let mut input_guardrail_results = Vec::new();
    let mut output_guardrail_results = Vec::new();
    let mut interruptions = Vec::new();

    for tool_call in tool_calls {
        let function_tool = agent
            .find_function_tool(&tool_call.name, tool_call.namespace.as_deref())
            .ok_or_else(|| ModelBehaviorError {
                message: format!(
                    "model requested unknown local function tool `{}`",
                    tool_call.name
                ),
            })?;

        let tool_context = ToolContext::from_tool_call(context, tool_call.clone())
            .with_agent(agent.clone())
            .with_run_config(run_config.clone());

        if function_tool.needs_approval {
            match context.approvals.get(&tool_call.id) {
                None => {
                    interruptions.push(RunInterruption {
                        kind: Some(RunInterruptionKind::ToolApproval),
                        call_id: Some(tool_call.id.clone()),
                        tool_name: Some(tool_call.name.clone()),
                        reason: Some("tool approval required".to_owned()),
                    });
                    break;
                }
                Some(approval) if !approval.approved => {
                    let output = ToolOutput::from(
                        approval
                            .reason
                            .as_deref()
                            .unwrap_or(DEFAULT_APPROVAL_REJECTION_MESSAGE),
                    );
                    new_items.push(RunItem::ToolCallOutput {
                        tool_name: tool_call.name,
                        output: output.to_output_item(),
                        call_id: Some(tool_call.id),
                        namespace: tool_call.namespace,
                    });
                    continue;
                }
                Some(_) => {}
            }
        }

        let mut invocation_rejected = None;
        for guardrail in &function_tool.tool_input_guardrails {
            let result = guardrail
                .run(crate::tool_guardrails::ToolInputGuardrailData {
                    context: tool_context.clone(),
                    agent: agent.clone(),
                })
                .await?;
            match &result.output.behavior {
                ToolGuardrailBehavior::Allow => {}
                ToolGuardrailBehavior::RejectContent { message } => {
                    invocation_rejected = Some(ToolOutput::from(message.as_str()));
                }
                ToolGuardrailBehavior::RaiseException => {
                    return Err(ToolInputGuardrailTripwireTriggered {
                        guardrail_name: result.guardrail_name.clone(),
                        output: result.output.clone(),
                    }
                    .into());
                }
            }
            input_guardrail_results.push(result);
        }

        let mut output = if let Some(rejected) = invocation_rejected {
            rejected
        } else {
            let parsed_arguments =
                serde_json::from_str::<Value>(&tool_call.arguments).unwrap_or(Value::Null);
            function_tool
                .invoke(tool_context.clone(), parsed_arguments)
                .await?
        };

        for guardrail in &function_tool.tool_output_guardrails {
            let result = guardrail
                .run(crate::tool_guardrails::ToolOutputGuardrailData {
                    context: tool_context.clone(),
                    agent: agent.clone(),
                    output: output.clone(),
                })
                .await?;
            match &result.output.behavior {
                ToolGuardrailBehavior::Allow => {}
                ToolGuardrailBehavior::RejectContent { message } => {
                    output = ToolOutput::from(message.as_str());
                }
                ToolGuardrailBehavior::RaiseException => {
                    return Err(ToolOutputGuardrailTripwireTriggered {
                        guardrail_name: result.guardrail_name.clone(),
                        output: result.output.clone(),
                    }
                    .into());
                }
            }
            output_guardrail_results.push(result);
        }

        new_items.push(RunItem::ToolCallOutput {
            tool_name: tool_call.name,
            output: output.to_output_item(),
            call_id: Some(tool_call.id),
            namespace: tool_call.namespace,
        });
    }

    Ok(ToolExecutionOutcome {
        new_items,
        input_guardrail_results,
        output_guardrail_results,
        interruptions,
    })
}

fn extract_tool_calls(output: &[OutputItem]) -> Vec<ToolCall> {
    output
        .iter()
        .filter_map(|item| match item {
            OutputItem::ToolCall {
                call_id,
                tool_name,
                arguments,
                namespace,
            } => Some(ToolCall {
                id: call_id.clone(),
                name: tool_name.clone(),
                arguments: serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_owned()),
                namespace: namespace.clone(),
            }),
            OutputItem::Text { .. }
            | OutputItem::Json { .. }
            | OutputItem::Handoff { .. }
            | OutputItem::Reasoning { .. } => None,
        })
        .collect()
}

fn resolve_handoff_agent(current_agent: &Agent, output: &[OutputItem]) -> Result<Option<Agent>> {
    let target = output.iter().find_map(|item| match item {
        OutputItem::Handoff { target_agent } => Some(target_agent.as_str()),
        OutputItem::Text { .. }
        | OutputItem::Json { .. }
        | OutputItem::ToolCall { .. }
        | OutputItem::Reasoning { .. } => None,
    });

    let Some(target) = target else {
        return Ok(None);
    };

    let handoff = current_agent
        .find_handoff(target)
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

    Ok(Some(target_agent))
}

fn find_pending_tool_call(state: &RunState, call_id: &str) -> Option<ToolCall> {
    state
        .generated_items
        .iter()
        .rev()
        .find_map(|item| match item {
            RunItem::ToolCall {
                tool_name,
                arguments,
                call_id: Some(existing_call_id),
                namespace,
            } if existing_call_id == call_id => Some(ToolCall {
                id: existing_call_id.clone(),
                name: tool_name.clone(),
                arguments: serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_owned()),
                namespace: namespace.clone(),
            }),
            RunItem::MessageOutput { .. }
            | RunItem::ToolCallOutput { .. }
            | RunItem::HandoffCall { .. }
            | RunItem::HandoffOutput { .. }
            | RunItem::Reasoning { .. }
            | RunItem::ToolCall { .. } => None,
        })
}

fn merge_usage(previous: Usage, next: Usage) -> Usage {
    Usage {
        input_tokens: previous.input_tokens.saturating_add(next.input_tokens),
        output_tokens: previous.output_tokens.saturating_add(next.output_tokens),
    }
}

fn merge_run_states(previous: &RunState, next: &mut RunState) {
    next.current_turn += previous.current_turn;
    next.current_agent = previous
        .current_agent
        .clone()
        .or_else(|| next.current_agent.clone());
    next.original_input = previous.original_input.clone();

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
    next.context_snapshot.usage =
        merge_usage(previous.context_snapshot.usage, next.context_snapshot.usage);

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
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    use crate::errors::AgentsError;
    use crate::guardrail::{GuardrailFunctionOutput, input_guardrail, output_guardrail};
    use crate::model::Model;
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
}
