use crate::agent::Agent;
use crate::agent_output::OutputSchemaDefinition;
use crate::errors::Result;
use crate::handoff::Handoff;
use crate::run_config::{CallModelData, ModelInputData, RunConfig};
use crate::run_context::RunContextWrapper;
use crate::tool::ToolDefinition;

pub(crate) fn validate_run_hooks() -> Result<()> {
    Ok(())
}

pub(crate) async fn maybe_filter_model_input(
    config: &RunConfig,
    agent: &Agent,
    context: &RunContextWrapper,
    model_data: ModelInputData,
) -> Result<ModelInputData> {
    let Some(filter) = &config.call_model_input_filter else {
        return Ok(model_data);
    };
    filter(CallModelData {
        model_data,
        agent: agent.clone(),
        context: Some(context.context.clone()),
    })
    .await
}

pub(crate) fn get_handoffs(agent: &Agent) -> Vec<Handoff> {
    agent.handoffs.clone()
}

pub(crate) async fn get_all_tools(
    agent: &Agent,
    context: &RunContextWrapper,
) -> Result<Vec<ToolDefinition>> {
    agent.runtime_tool_definitions(context).await
}

pub(crate) fn get_output_schema(agent: &Agent) -> Option<OutputSchemaDefinition> {
    agent.output_schema.clone()
}

pub(crate) fn get_model(agent: &Agent) -> Option<String> {
    agent.model.clone()
}
