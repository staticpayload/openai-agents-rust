use agents_core::ModelInputData;
use futures::FutureExt;
use std::sync::{Arc, Mutex, OnceLock};

use agents_core::OutputSchemaDefinition;
use openai_agents::{
    Agent, AgentAsToolInput, AgentAsToolOptions, AgentRunner, AgentsError,
    LocalShellCommandRequest, Model, ModelProvider, ModelRequest, ModelResponse, OutputItem,
    RunConfig, RunContext, RunContextWrapper, Runner, Tool, ToolContext, ToolOutput, Usage,
    drop_agent_tool_run_result, function_tool, get_default_agent_runner, run, run_sync,
    set_default_agent_runner,
};
use serde_json::json;

fn default_runner_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

struct DefaultRunnerReset(AgentRunner);

impl Drop for DefaultRunnerReset {
    fn drop(&mut self) {
        set_default_agent_runner(Some(self.0.clone()));
    }
}

#[derive(Clone, Default)]
struct StructuredOutputLoopModel {
    calls: Arc<Mutex<usize>>,
    output_schemas: Arc<Mutex<Vec<Option<OutputSchemaDefinition>>>>,
}

#[async_trait::async_trait]
impl Model for StructuredOutputLoopModel {
    async fn generate(&self, request: ModelRequest) -> openai_agents::Result<ModelResponse> {
        let mut calls = self
            .calls
            .lock()
            .expect("structured output loop calls lock");
        self.output_schemas
            .lock()
            .expect("structured output loop schema lock")
            .push(request.output_schema.clone());
        *calls += 1;

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::ToolCall {
                call_id: format!("call-{calls}"),
                tool_name: "search".to_owned(),
                arguments: json!({"query":"rust"}),
                namespace: None,
            }],
            usage: Usage::default(),
            response_id: Some(format!("resp-{calls}")),
            request_id: None,
        })
    }
}

struct StructuredOutputLoopProvider {
    model: Arc<StructuredOutputLoopModel>,
}

impl ModelProvider for StructuredOutputLoopProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[tokio::test]
async fn facade_run_uses_default_runner() {
    let _guard = default_runner_lock().lock().await;
    let original_runner = get_default_agent_runner();
    let _reset = DefaultRunnerReset(original_runner.clone());
    set_default_agent_runner(Some(Runner::new().with_config(RunConfig {
        model: Some("gpt-facade-default".to_owned()),
        ..RunConfig::default()
    })));

    let agent = Agent::builder("assistant").build();
    let result = run(&agent, "hello")
        .await
        .expect("facade run should succeed");

    assert_eq!(
        result
            .raw_responses
            .first()
            .and_then(|response| response.model.as_deref()),
        Some("gpt-facade-default")
    );
}

#[tokio::test]
async fn facade_run_sync_rejects_active_runtime() {
    let agent = Agent::builder("assistant").build();

    let error = run_sync(&agent, "hello").expect_err("run_sync should reject active runtimes");

    assert!(error.to_string().contains("event loop is already running"));
}

#[tokio::test]
async fn facade_agent_as_tool_runs_nested_agent() {
    let agent = Agent::builder("nested").build();
    let tool = agent
        .as_tool::<AgentAsToolInput>(
            Some("nested_tool"),
            Some("Invoke the nested agent"),
            AgentAsToolOptions::default(),
        )
        .expect("agent tool should build");

    let call_id = "call-facade-nested";
    let output = tool
        .invoke(
            ToolContext::new(
                RunContextWrapper::new(RunContext::default()),
                "nested_tool",
                call_id,
                "{\"input\":\"hello\"}",
            ),
            json!({"input":"hello"}),
        )
        .await
        .expect("agent tool should execute");

    assert_eq!(output, ToolOutput::from("hello"));
    let stored = openai_agents::peek_agent_tool_run_result(call_id, None)
        .expect("nested run result should be recorded");
    assert_eq!(stored.final_output.as_deref(), Some("hello"));
    drop_agent_tool_run_result(call_id, None);
}

#[tokio::test]
async fn facade_agent_as_tool_preserves_nested_state_and_structured_input() {
    let agent = Agent::builder("translator").build();
    let tool = agent
        .as_tool::<LocalShellCommandRequest>(
            Some("translate"),
            Some("Translate text"),
            AgentAsToolOptions::default(),
        )
        .expect("agent tool should build");
    let mut run_context = RunContextWrapper::new(RunContext::default());
    openai_agents::set_agent_tool_state_scope(&mut run_context, Some("scope-facade".to_owned()));
    run_context.tool_input = Some(json!({"stale": true}));

    tool.invoke(
        ToolContext::new(
            run_context,
            "translate",
            "call-facade-translate",
            "{\"command\":\"echo hola\",\"cwd\":\"/tmp\",\"env\":{\"LANG\":\"en_US.UTF-8\"}}",
        ),
        json!({"command":"echo hola","cwd":"/tmp","env":{"LANG":"en_US.UTF-8"}}),
    )
    .await
    .expect("agent tool should execute");

    let stored = openai_agents::peek_agent_tool_run_result(
        "call-facade-translate",
        Some("scope-facade".to_owned()),
    )
    .expect("structured nested run result should be recorded");
    assert_eq!(
        stored.context_snapshot.agent_tool_state_scope.as_deref(),
        Some("scope-facade")
    );
    assert_eq!(
        stored.context_snapshot.tool_input,
        Some(json!({"command":"echo hola","cwd":"/tmp","env":{"LANG":"en_US.UTF-8"}}))
    );

    drop_agent_tool_run_result("call-facade-translate", Some("scope-facade".to_owned()));
}

#[tokio::test]
async fn facade_call_model_input_filter_rewrites_non_streamed_history() {
    let agent = Agent::builder("assistant").build();
    let result = Runner::new()
        .with_config(RunConfig {
            call_model_input_filter: Some(std::sync::Arc::new(|mut data| {
                async move {
                    data.model_data.input = vec![openai_agents::InputItem::from("filtered")];
                    Ok::<_, openai_agents::AgentsError>(data.model_data)
                }
                .boxed()
            })),
            ..RunConfig::default()
        })
        .run(&agent, "hello")
        .await
        .expect("filtered run should succeed");

    assert_eq!(result.final_output.as_deref(), Some("filtered"));
    assert_eq!(
        result.normalized_input,
        Some(vec![openai_agents::InputItem::from("filtered")])
    );
}

#[tokio::test]
async fn facade_call_model_input_filter_rewrites_streamed_history() {
    let agent = Agent::builder("assistant").build();
    let streamed = Runner::new()
        .with_config(RunConfig {
            call_model_input_filter: Some(std::sync::Arc::new(|_data| {
                async move {
                    Ok::<_, openai_agents::AgentsError>(ModelInputData {
                        input: vec![openai_agents::InputItem::from("stream-filtered")],
                        instructions: None,
                    })
                }
                .boxed()
            })),
            ..RunConfig::default()
        })
        .run_streamed(&agent, "hello")
        .await
        .expect("filtered streamed run should start");

    let result = streamed
        .wait_for_completion()
        .await
        .expect("filtered streamed run should complete");

    assert_eq!(result.final_output.as_deref(), Some("stream-filtered"));
    assert_eq!(
        result.normalized_input,
        Some(vec![openai_agents::InputItem::from("stream-filtered")])
    );
}

#[tokio::test]
async fn facade_structured_output_run_exhausts_max_turns_with_runtime_schema_plumbing() {
    let model = Arc::new(StructuredOutputLoopModel::default());
    let provider = Arc::new(StructuredOutputLoopProvider {
        model: model.clone(),
    });
    let output_schema = OutputSchemaDefinition::new(
        "StructuredSummary",
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "answer": { "type": "string" }
            },
            "required": ["answer"]
        }),
        true,
    );
    let search_tool = function_tool(
        "search",
        "Search docs",
        |_ctx, _args: serde_json::Value| async { Ok::<_, AgentsError>("tool-result".to_owned()) },
    )
    .expect("function tool should build");
    let agent = Agent::builder("assistant")
        .output_schema(output_schema.clone())
        .function_tool(search_tool)
        .build();
    let runner = Runner::new()
        .with_model_provider(provider)
        .with_config(RunConfig {
            max_turns: 2,
            ..RunConfig::default()
        });

    let error = runner
        .run(&agent, "hello")
        .await
        .expect_err("non-streamed structured-output run should exhaust max turns");
    assert!(error.to_string().contains("max_turns (2)"));
    assert_eq!(
        model
            .output_schemas
            .lock()
            .expect("structured output schema lock")
            .clone(),
        vec![Some(output_schema.clone()), Some(output_schema.clone())]
    );

    let streamed = runner
        .run_streamed(&agent, "hello")
        .await
        .expect("streamed structured-output run should start");
    let error = streamed
        .wait_for_completion()
        .await
        .expect_err("streamed structured-output run should exhaust max turns");
    assert!(error.to_string().contains("max_turns (2)"));

    let schemas = model
        .output_schemas
        .lock()
        .expect("structured output schema lock")
        .clone();
    assert_eq!(
        schemas,
        vec![
            Some(output_schema.clone()),
            Some(output_schema.clone()),
            Some(output_schema.clone()),
            Some(output_schema),
        ]
    );
}
