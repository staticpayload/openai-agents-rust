use futures::future::try_join_all;

use crate::agent::Agent;
use crate::errors::Result;
use crate::exceptions::{InputGuardrailTripwireTriggered, OutputGuardrailTripwireTriggered};
use crate::guardrail::{
    InputGuardrail, InputGuardrailResult, OutputGuardrail, OutputGuardrailResult,
};
use crate::items::{InputItem, OutputItem};
use crate::run_context::{RunContext, RunContextWrapper};

pub(crate) async fn run_input_guardrails<TContext>(
    agent: &Agent,
    guardrails: &[InputGuardrail<TContext>],
    input: &[InputItem],
    context: &RunContextWrapper<TContext>,
) -> Result<Vec<InputGuardrailResult>>
where
    TContext: Clone + Send + Sync + 'static,
{
    if guardrails.is_empty() {
        return Ok(Vec::new());
    }

    let futures = guardrails
        .iter()
        .map(|guardrail| guardrail.run(agent.clone(), input.to_vec(), context.clone()));
    let results = try_join_all(futures).await?;

    if let Some(result) = results
        .iter()
        .find(|result| result.output.tripwire_triggered)
        .cloned()
    {
        return Err(InputGuardrailTripwireTriggered {
            guardrail_result: result,
        }
        .into());
    }

    Ok(results)
}

pub(crate) async fn run_output_guardrails<TContext>(
    agent: &Agent,
    guardrails: &[OutputGuardrail<TContext>],
    agent_output: &[OutputItem],
    context: &RunContextWrapper<TContext>,
) -> Result<Vec<OutputGuardrailResult>>
where
    TContext: Clone + Send + Sync + 'static,
{
    if guardrails.is_empty() {
        return Ok(Vec::new());
    }

    let futures = guardrails
        .iter()
        .map(|guardrail| guardrail.run(context.clone(), agent.clone(), agent_output.to_vec()));
    let results = try_join_all(futures).await?;

    if let Some(result) = results
        .iter()
        .find(|result| result.output.tripwire_triggered)
        .cloned()
    {
        return Err(OutputGuardrailTripwireTriggered {
            guardrail_result: result,
        }
        .into());
    }

    Ok(results)
}

pub(crate) fn new_run_context(workflow_name: impl Into<String>) -> RunContextWrapper {
    RunContextWrapper::new(RunContext {
        conversation_id: None,
        workflow_name: Some(workflow_name.into()),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::guardrail::{GuardrailFunctionOutput, input_guardrail, output_guardrail};

    use super::*;

    #[tokio::test]
    async fn raises_on_input_tripwire() {
        let agent = Agent::builder("router").build();
        let guardrails = vec![input_guardrail(
            "block",
            |_ctx, _agent, _input| async move {
                Ok(GuardrailFunctionOutput::tripwire(Some(
                    json!({"blocked":true}),
                )))
            },
        )];

        let error = run_input_guardrails(
            &agent,
            &guardrails,
            &[InputItem::from("hello")],
            &new_run_context("workflow"),
        )
        .await
        .expect_err("tripwire should fail");

        assert!(matches!(
            error,
            crate::errors::AgentsError::InputGuardrailTripwire(_)
        ));
    }

    #[tokio::test]
    async fn raises_on_output_tripwire() {
        let agent = Agent::builder("writer").build();
        let guardrails = vec![output_guardrail(
            "block",
            |_ctx, _agent, _output| async move { Ok(GuardrailFunctionOutput::tripwire(None)) },
        )];

        let error = run_output_guardrails(
            &agent,
            &guardrails,
            &[OutputItem::Text {
                text: "hello".to_owned(),
            }],
            &new_run_context("workflow"),
        )
        .await
        .expect_err("tripwire should fail");

        assert!(matches!(
            error,
            crate::errors::AgentsError::OutputGuardrailTripwire(_)
        ));
    }
}
