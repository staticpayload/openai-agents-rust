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
use crate::run_context::RunContext;
use crate::tool::ToolOutput;
use crate::tool_context::ToolContext;

/// Behavior emitted by a tool guardrail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolGuardrailBehavior {
    Allow,
    RejectContent { message: String },
    RaiseException,
}

/// Serializable tool guardrail output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolGuardrailFunctionOutput {
    pub output_info: Option<Value>,
    pub behavior: ToolGuardrailBehavior,
}

impl ToolGuardrailFunctionOutput {
    pub fn allow(output_info: Option<Value>) -> Self {
        Self {
            output_info,
            behavior: ToolGuardrailBehavior::Allow,
        }
    }

    pub fn reject_content(message: impl Into<String>, output_info: Option<Value>) -> Self {
        Self {
            output_info,
            behavior: ToolGuardrailBehavior::RejectContent {
                message: message.into(),
            },
        }
    }

    pub fn raise_exception(output_info: Option<Value>) -> Self {
        Self {
            output_info,
            behavior: ToolGuardrailBehavior::RaiseException,
        }
    }

    pub fn rejection_message(&self) -> Option<&str> {
        match &self.behavior {
            ToolGuardrailBehavior::RejectContent { message } => Some(message.as_str()),
            ToolGuardrailBehavior::Allow | ToolGuardrailBehavior::RaiseException => None,
        }
    }
}

/// Input data passed to a tool input guardrail.
#[derive(Clone, Debug)]
pub struct ToolInputGuardrailData<TContext = RunContext> {
    pub context: ToolContext<TContext>,
    pub agent: Agent,
}

/// Input data passed to a tool output guardrail.
#[derive(Clone, Debug)]
pub struct ToolOutputGuardrailData<TContext = RunContext> {
    pub context: ToolContext<TContext>,
    pub agent: Agent,
    pub output: ToolOutput,
}

/// Serializable result of a tool input guardrail run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolInputGuardrailResult {
    pub guardrail_name: String,
    pub output: ToolGuardrailFunctionOutput,
}

/// Serializable result of a tool output guardrail run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolOutputGuardrailResult {
    pub guardrail_name: String,
    pub output: ToolGuardrailFunctionOutput,
}

type ToolInputGuardrailExecutor<TContext> = Arc<
    dyn Fn(
            ToolInputGuardrailData<TContext>,
        ) -> BoxFuture<'static, Result<ToolGuardrailFunctionOutput>>
        + Send
        + Sync,
>;

type ToolOutputGuardrailExecutor<TContext> = Arc<
    dyn Fn(
            ToolOutputGuardrailData<TContext>,
        ) -> BoxFuture<'static, Result<ToolGuardrailFunctionOutput>>
        + Send
        + Sync,
>;

/// Guardrail run before invoking a function tool.
#[derive(Clone, Serialize, Deserialize)]
pub struct ToolInputGuardrail<TContext = RunContext> {
    pub name: Option<String>,
    #[serde(skip)]
    executor: Option<ToolInputGuardrailExecutor<TContext>>,
    #[serde(skip)]
    _marker: PhantomData<fn() -> TContext>,
}

impl<TContext> std::fmt::Debug for ToolInputGuardrail<TContext> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolInputGuardrail")
            .field("name", &self.name)
            .finish()
    }
}

impl<TContext> Default for ToolInputGuardrail<TContext> {
    fn default() -> Self {
        Self {
            name: None,
            executor: None,
            _marker: PhantomData,
        }
    }
}

impl<TContext> ToolInputGuardrail<TContext> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_deref().unwrap_or("tool_input_guardrail")
    }

    pub fn with_executor<F, Fut>(mut self, executor: F) -> Self
    where
        TContext: Clone + Send + Sync + 'static,
        F: Fn(ToolInputGuardrailData<TContext>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolGuardrailFunctionOutput>> + Send + 'static,
    {
        self.executor = Some(Arc::new(move |data| executor(data).boxed()));
        self
    }

    pub async fn run(
        &self,
        data: ToolInputGuardrailData<TContext>,
    ) -> Result<ToolInputGuardrailResult>
    where
        TContext: Clone + Send + Sync + 'static,
    {
        let Some(executor) = &self.executor else {
            return Err(AgentsError::message(format!(
                "tool input guardrail `{}` has no executor",
                self.get_name()
            )));
        };

        let output = executor(data).await?;
        Ok(ToolInputGuardrailResult {
            guardrail_name: self.get_name().to_owned(),
            output,
        })
    }
}

/// Guardrail run after a function tool returns.
#[derive(Clone, Serialize, Deserialize)]
pub struct ToolOutputGuardrail<TContext = RunContext> {
    pub name: Option<String>,
    #[serde(skip)]
    executor: Option<ToolOutputGuardrailExecutor<TContext>>,
    #[serde(skip)]
    _marker: PhantomData<fn() -> TContext>,
}

impl<TContext> std::fmt::Debug for ToolOutputGuardrail<TContext> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolOutputGuardrail")
            .field("name", &self.name)
            .finish()
    }
}

impl<TContext> Default for ToolOutputGuardrail<TContext> {
    fn default() -> Self {
        Self {
            name: None,
            executor: None,
            _marker: PhantomData,
        }
    }
}

impl<TContext> ToolOutputGuardrail<TContext> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_deref().unwrap_or("tool_output_guardrail")
    }

    pub fn with_executor<F, Fut>(mut self, executor: F) -> Self
    where
        TContext: Clone + Send + Sync + 'static,
        F: Fn(ToolOutputGuardrailData<TContext>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolGuardrailFunctionOutput>> + Send + 'static,
    {
        self.executor = Some(Arc::new(move |data| executor(data).boxed()));
        self
    }

    pub async fn run(
        &self,
        data: ToolOutputGuardrailData<TContext>,
    ) -> Result<ToolOutputGuardrailResult>
    where
        TContext: Clone + Send + Sync + 'static,
    {
        let Some(executor) = &self.executor else {
            return Err(AgentsError::message(format!(
                "tool output guardrail `{}` has no executor",
                self.get_name()
            )));
        };

        let output = executor(data).await?;
        Ok(ToolOutputGuardrailResult {
            guardrail_name: self.get_name().to_owned(),
            output,
        })
    }
}

pub fn tool_input_guardrail<TContext, F, Fut>(
    name: impl Into<String>,
    guardrail_function: F,
) -> ToolInputGuardrail<TContext>
where
    TContext: Clone + Send + Sync + 'static,
    F: Fn(ToolInputGuardrailData<TContext>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ToolGuardrailFunctionOutput>> + Send + 'static,
{
    ToolInputGuardrail::new(name).with_executor(guardrail_function)
}

pub fn tool_output_guardrail<TContext, F, Fut>(
    name: impl Into<String>,
    guardrail_function: F,
) -> ToolOutputGuardrail<TContext>
where
    TContext: Clone + Send + Sync + 'static,
    F: Fn(ToolOutputGuardrailData<TContext>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ToolGuardrailFunctionOutput>> + Send + 'static,
{
    ToolOutputGuardrail::new(name).with_executor(guardrail_function)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::run_context::{RunContext, RunContextWrapper};

    use super::*;

    #[tokio::test]
    async fn runs_tool_input_guardrail() {
        let guardrail = tool_input_guardrail("tool-input", |data| async move {
            Ok(ToolGuardrailFunctionOutput::reject_content(
                format!("blocked {}", data.context.tool_name),
                Some(json!({"agent": data.agent.name})),
            ))
        });

        let result = guardrail
            .run(ToolInputGuardrailData {
                context: ToolContext::new(
                    RunContextWrapper::new(RunContext::default()),
                    "search",
                    "call-1",
                    "{}",
                ),
                agent: Agent::builder("router").build(),
            })
            .await
            .expect("guardrail should run");

        assert_eq!(result.guardrail_name, "tool-input");
        assert_eq!(result.output.rejection_message(), Some("blocked search"));
    }

    #[tokio::test]
    async fn runs_tool_output_guardrail() {
        let guardrail = tool_output_guardrail("tool-output", |data| async move {
            Ok(ToolGuardrailFunctionOutput::allow(Some(json!({
                "tool": data.context.qualified_tool_name(),
                "has_text": matches!(data.output, ToolOutput::Text(_)),
            }))))
        });

        let result = guardrail
            .run(ToolOutputGuardrailData {
                context: ToolContext::new(
                    RunContextWrapper::new(RunContext::default()),
                    "search",
                    "call-1",
                    "{}",
                ),
                agent: Agent::builder("router").build(),
                output: ToolOutput::from("ok"),
            })
            .await
            .expect("guardrail should run");

        assert_eq!(result.guardrail_name, "tool-output");
        assert_eq!(
            result.output.output_info,
            Some(json!({"tool":"search","has_text":true}))
        );
    }
}
