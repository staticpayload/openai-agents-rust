use std::sync::Arc;

use uuid::Uuid;

use crate::agent::Agent;
use crate::errors::Result;
use crate::items::{InputItem, OutputItem};
use crate::model::{ModelProvider, ModelRequest};
use crate::result::RunResult;
use crate::run_config::{DEFAULT_MAX_TURNS, RunConfig};
use crate::tracing::Trace;

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

        let output = if let Some(model_provider) = &self.model_provider {
            let request = ModelRequest {
                trace_id: Some(trace.id),
                model: agent.model.clone(),
                instructions: agent.instructions.clone(),
                input: input.clone(),
                tools: agent
                    .tools
                    .iter()
                    .map(|tool| tool.definition.clone())
                    .collect(),
            };
            model_provider
                .resolve(agent.model.as_deref())
                .generate(request)
                .await?
                .output
        } else {
            let text = input
                .iter()
                .rev()
                .find_map(InputItem::as_text)
                .unwrap_or_default()
                .to_owned();
            vec![OutputItem::Text { text }]
        };

        let final_output = output
            .iter()
            .find_map(OutputItem::as_text)
            .map(ToOwned::to_owned);

        Ok(RunResult {
            agent_name: agent.name.clone(),
            input,
            output,
            final_output,
            usage: crate::usage::Usage::default(),
            trace: Some(trace),
        })
    }
}

pub async fn run(agent: &Agent, input: impl Into<InputItem>) -> Result<RunResult> {
    Runner::new().run(agent, input).await
}
