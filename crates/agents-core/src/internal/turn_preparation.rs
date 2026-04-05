use crate::agent::Agent;
use crate::agent_output::OutputSchemaDefinition;
use crate::errors::Result;
use crate::handoff::Handoff;
use crate::internal::oai_conversation;
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
    prepared_source_refs: Option<&[Option<oai_conversation::PreparedSourceRef>]>,
) -> Result<FilteredModelInput> {
    let Some(filter) = &config.call_model_input_filter else {
        return Ok(FilteredModelInput {
            model_data,
            source_items: None,
        });
    };
    let prepared_input = model_data.input.clone();
    let exact_source_index =
        oai_conversation::build_filtered_input_identity_index(&model_data.input);
    let filtered = filter(CallModelData {
        model_data,
        agent: agent.clone(),
        context: Some(context.context.clone()),
    })
    .await?;
    let source_items = prepared_source_refs.map(|prepared_source_refs| {
        oai_conversation::derive_filtered_input_source_indices(
            &prepared_input,
            &filtered.input,
            &exact_source_index,
        )
        .into_iter()
        .map(|prepared_index| {
            prepared_index.and_then(|prepared_index| {
                prepared_source_refs.get(prepared_index).copied().flatten()
            })
        })
        .collect()
    });
    Ok(FilteredModelInput {
        model_data: filtered,
        source_items,
    })
}

pub(crate) struct FilteredModelInput {
    pub model_data: ModelInputData,
    pub source_items: Option<Vec<Option<oai_conversation::PreparedSourceRef>>>,
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
