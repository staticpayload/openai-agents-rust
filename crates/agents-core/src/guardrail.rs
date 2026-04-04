use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::FutureExt;
use futures::future::BoxFuture;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::errors::{AgentsError, Result};
use crate::items::{InputItem, OutputItem};
use crate::run_context::{RunContext, RunContextWrapper};

/// Serializable output produced by an input or output guardrail.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GuardrailFunctionOutput {
    pub output_info: Option<Value>,
    pub tripwire_triggered: bool,
}

impl GuardrailFunctionOutput {
    pub fn allow(output_info: Option<Value>) -> Self {
        Self {
            output_info,
            tripwire_triggered: false,
        }
    }

    pub fn tripwire(output_info: Option<Value>) -> Self {
        Self {
            output_info,
            tripwire_triggered: true,
        }
    }
}

/// Serializable result of an input guardrail run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InputGuardrailResult {
    pub guardrail_name: String,
    pub output: GuardrailFunctionOutput,
}

/// Serializable result of an output guardrail run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OutputGuardrailResult {
    pub guardrail_name: String,
    pub agent_name: String,
    pub agent_output: Vec<OutputItem>,
    pub output: GuardrailFunctionOutput,
}

type InputGuardrailExecutor<TContext> = Arc<
    dyn Fn(
            RunContextWrapper<TContext>,
            Agent,
            Vec<InputItem>,
        ) -> BoxFuture<'static, Result<GuardrailFunctionOutput>>
        + Send
        + Sync,
>;

type OutputGuardrailExecutor<TContext> = Arc<
    dyn Fn(
            RunContextWrapper<TContext>,
            Agent,
            Vec<OutputItem>,
        ) -> BoxFuture<'static, Result<GuardrailFunctionOutput>>
        + Send
        + Sync,
>;

/// Input guardrails run before or alongside the agent.
#[derive(Clone, Serialize, Deserialize)]
pub struct InputGuardrail<TContext = RunContext> {
    pub name: Option<String>,
    pub description: Option<String>,
    pub run_in_parallel: bool,
    #[serde(skip)]
    executor: Option<InputGuardrailExecutor<TContext>>,
    #[serde(skip)]
    _marker: PhantomData<fn() -> TContext>,
}

impl<TContext> std::fmt::Debug for InputGuardrail<TContext> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputGuardrail")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("run_in_parallel", &self.run_in_parallel)
            .finish()
    }
}

impl<TContext> Default for InputGuardrail<TContext> {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            run_in_parallel: true,
            executor: None,
            _marker: PhantomData,
        }
    }
}

impl<TContext> InputGuardrail<TContext> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_parallelism(mut self, run_in_parallel: bool) -> Self {
        self.run_in_parallel = run_in_parallel;
        self
    }

    pub fn get_name(&self) -> &str {
        self.name.as_deref().unwrap_or("input_guardrail")
    }

    pub fn with_executor<F, Fut>(mut self, executor: F) -> Self
    where
        TContext: Clone + Send + Sync + 'static,
        F: Fn(RunContextWrapper<TContext>, Agent, Vec<InputItem>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GuardrailFunctionOutput>> + Send + 'static,
    {
        self.executor = Some(Arc::new(move |context, agent, input| {
            executor(context, agent, input).boxed()
        }));
        self
    }

    pub async fn run(
        &self,
        agent: Agent,
        input: Vec<InputItem>,
        context: RunContextWrapper<TContext>,
    ) -> Result<InputGuardrailResult>
    where
        TContext: Clone + Send + Sync + 'static,
    {
        let Some(executor) = &self.executor else {
            return Err(AgentsError::message(format!(
                "input guardrail `{}` has no executor",
                self.get_name()
            )));
        };

        let output = executor(context, agent, input).await?;
        Ok(InputGuardrailResult {
            guardrail_name: self.get_name().to_owned(),
            output,
        })
    }
}

/// Output guardrails validate the final agent output.
#[derive(Clone, Serialize, Deserialize)]
pub struct OutputGuardrail<TContext = RunContext> {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(skip)]
    executor: Option<OutputGuardrailExecutor<TContext>>,
    #[serde(skip)]
    _marker: PhantomData<fn() -> TContext>,
}

impl<TContext> std::fmt::Debug for OutputGuardrail<TContext> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputGuardrail")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl<TContext> Default for OutputGuardrail<TContext> {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            executor: None,
            _marker: PhantomData,
        }
    }
}

impl<TContext> OutputGuardrail<TContext> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn get_name(&self) -> &str {
        self.name.as_deref().unwrap_or("output_guardrail")
    }

    pub fn with_executor<F, Fut>(mut self, executor: F) -> Self
    where
        TContext: Clone + Send + Sync + 'static,
        F: Fn(RunContextWrapper<TContext>, Agent, Vec<OutputItem>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GuardrailFunctionOutput>> + Send + 'static,
    {
        self.executor = Some(Arc::new(move |context, agent, output| {
            executor(context, agent, output).boxed()
        }));
        self
    }

    pub async fn run(
        &self,
        context: RunContextWrapper<TContext>,
        agent: Agent,
        agent_output: Vec<OutputItem>,
    ) -> Result<OutputGuardrailResult>
    where
        TContext: Clone + Send + Sync + 'static,
    {
        let Some(executor) = &self.executor else {
            return Err(AgentsError::message(format!(
                "output guardrail `{}` has no executor",
                self.get_name()
            )));
        };

        let output = executor(context, agent.clone(), agent_output.clone()).await?;
        Ok(OutputGuardrailResult {
            guardrail_name: self.get_name().to_owned(),
            agent_name: agent.name,
            agent_output,
            output,
        })
    }
}

pub fn input_guardrail<TContext, F, Fut>(
    name: impl Into<String>,
    guardrail_function: F,
) -> InputGuardrail<TContext>
where
    TContext: Clone + Send + Sync + 'static,
    F: Fn(RunContextWrapper<TContext>, Agent, Vec<InputItem>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<GuardrailFunctionOutput>> + Send + 'static,
{
    InputGuardrail::new(name).with_executor(guardrail_function)
}

pub fn output_guardrail<TContext, F, Fut>(
    name: impl Into<String>,
    guardrail_function: F,
) -> OutputGuardrail<TContext>
where
    TContext: Clone + Send + Sync + 'static,
    F: Fn(RunContextWrapper<TContext>, Agent, Vec<OutputItem>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<GuardrailFunctionOutput>> + Send + 'static,
{
    OutputGuardrail::new(name).with_executor(guardrail_function)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::run_context::RunContext;

    use super::*;

    #[tokio::test]
    async fn runs_input_guardrail_executor() {
        let guardrail = input_guardrail("input-check", |_ctx, _agent, input| async move {
            Ok(GuardrailFunctionOutput::tripwire(Some(json!({
                "items": input.len(),
            }))))
        })
        .with_parallelism(false);

        let result = guardrail
            .run(
                Agent::builder("router").build(),
                vec![InputItem::from("hello")],
                RunContextWrapper::new(RunContext::default()),
            )
            .await
            .expect("guardrail should run");

        assert_eq!(result.guardrail_name, "input-check");
        assert!(result.output.tripwire_triggered);
        assert_eq!(result.output.output_info, Some(json!({"items":1})));
    }

    #[tokio::test]
    async fn runs_output_guardrail_executor() {
        let guardrail = output_guardrail("output-check", |_ctx, agent, output| async move {
            Ok(GuardrailFunctionOutput::allow(Some(json!({
                "agent": agent.name,
                "items": output.len(),
            }))))
        });

        let result = guardrail
            .run(
                RunContextWrapper::new(RunContext::default()),
                Agent::builder("writer").build(),
                vec![OutputItem::Text {
                    text: "done".to_owned(),
                }],
            )
            .await
            .expect("guardrail should run");

        assert_eq!(result.guardrail_name, "output-check");
        assert_eq!(result.agent_name, "writer");
        assert!(!result.output.tripwire_triggered);
        assert_eq!(
            result.output.output_info,
            Some(json!({"agent":"writer","items":1}))
        );
    }
}
