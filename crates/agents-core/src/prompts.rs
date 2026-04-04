use std::collections::BTreeMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::Agent;
use crate::errors::Result;
use crate::run_context::{RunContext, RunContextWrapper};

/// Prompt configuration for model execution.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Prompt {
    pub id: String,
    pub version: Option<String>,
    pub variables: BTreeMap<String, Value>,
}

/// Inputs provided to a dynamic prompt callback.
#[derive(Clone, Debug)]
pub struct GenerateDynamicPromptData<TContext = RunContext> {
    pub context: RunContextWrapper<TContext>,
    pub agent: Agent,
}

pub type DynamicPromptFunction<TContext = RunContext> = Arc<
    dyn Fn(GenerateDynamicPromptData<TContext>) -> BoxFuture<'static, Result<Prompt>> + Send + Sync,
>;

/// Prompt input supported by the Rust surface.
#[derive(Clone)]
pub enum PromptSpec<TContext = RunContext> {
    Static(Prompt),
    Dynamic(DynamicPromptFunction<TContext>),
}

pub struct PromptUtil;

impl PromptUtil {
    pub async fn to_model_input<TContext: Clone + Send + Sync + 'static>(
        prompt: Option<PromptSpec<TContext>>,
        context: RunContextWrapper<TContext>,
        agent: Agent,
    ) -> Result<Option<Prompt>> {
        match prompt {
            None => Ok(None),
            Some(PromptSpec::Static(prompt)) => Ok(Some(prompt)),
            Some(PromptSpec::Dynamic(callback)) => {
                callback(GenerateDynamicPromptData { context, agent })
                    .await
                    .map(Some)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::FutureExt;

    use super::*;

    #[tokio::test]
    async fn resolves_static_prompt() {
        let prompt = Prompt {
            id: "prompt-123".to_owned(),
            version: Some("v1".to_owned()),
            variables: BTreeMap::new(),
        };
        let resolved = PromptUtil::to_model_input(
            Some(PromptSpec::Static(prompt.clone())),
            RunContextWrapper::new(RunContext::default()),
            Agent::builder("assistant").build(),
        )
        .await
        .expect("static prompt should resolve");
        assert_eq!(resolved.unwrap().id, prompt.id);
    }

    #[tokio::test]
    async fn resolves_dynamic_prompt() {
        let callback: DynamicPromptFunction = Arc::new(|_| {
            async move {
                Ok(Prompt {
                    id: "dynamic".to_owned(),
                    version: None,
                    variables: BTreeMap::new(),
                })
            }
            .boxed()
        });
        let resolved = PromptUtil::to_model_input(
            Some(PromptSpec::Dynamic(callback)),
            RunContextWrapper::new(RunContext::default()),
            Agent::builder("assistant").build(),
        )
        .await
        .expect("dynamic prompt should resolve");
        assert_eq!(resolved.unwrap().id, "dynamic");
    }
}
