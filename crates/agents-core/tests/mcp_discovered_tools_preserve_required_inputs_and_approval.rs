use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agents_core::{
    Agent, MCPServer, MCPTool, MCPToolAnnotations, MCPUtil, Model, ModelProvider, ModelRequest,
    ModelResponse, OutputItem, Result, RunContext, RunContextWrapper, RunInterruptionKind, RunItem,
    RunResult, RunState, Runner, Tool, ToolOutput, Usage,
};
use async_trait::async_trait;
use serde_json::{Value, json};

#[derive(Default)]
struct FakeServerState {
    tool_calls: AtomicUsize,
}

struct FakeServer {
    state: Arc<FakeServerState>,
}

#[async_trait]
impl MCPServer for FakeServer {
    fn name(&self) -> &str {
        "docs"
    }

    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<MCPTool>> {
        Ok(vec![MCPTool {
            name: "lookup".to_owned(),
            description: Some("Lookup MCP data".to_owned()),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "q": {"type": "string"}
                },
                "required": ["q"]
            })),
            title: None,
            annotations: Some(MCPToolAnnotations {
                title: Some("Lookup".to_owned()),
            }),
            meta: None,
            namespace: Some("mcp".to_owned()),
            requires_approval: true,
        }])
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        _meta: Option<Value>,
    ) -> Result<ToolOutput> {
        self.state.tool_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::from(format!("{tool_name}:{arguments}")))
    }
}

#[derive(Clone, Default)]
struct DiscoveryModel {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Model for DiscoveryModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: "call-1".to_owned(),
                    tool_name: "lookup".to_owned(),
                    arguments: json!({ "q": "rust" }),
                    namespace: Some("mcp".to_owned()),
                }],
                usage: Usage::default(),
                response_id: Some("resp-1".to_owned()),
                request_id: Some("req-1".to_owned()),
            });
        }

        let tool_output = request
            .input
            .iter()
            .filter_map(|item| match item {
                agents_core::InputItem::Json { value } => value
                    .get("type")
                    .and_then(Value::as_str)
                    .filter(|kind| *kind == "tool_call_output")
                    .and_then(|_| value.get("output"))
                    .and_then(|output| output.get("text"))
                    .and_then(Value::as_str),
                agents_core::InputItem::Text { .. } => None,
            })
            .find(|text| text.starts_with("lookup:"))
            .expect("tool output should be present");

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: format!("done:{tool_output}"),
            }],
            usage: Usage::default(),
            response_id: Some("resp-2".to_owned()),
            request_id: Some("req-2".to_owned()),
        })
    }
}

#[derive(Clone)]
struct DiscoveryProvider {
    model: Arc<DiscoveryModel>,
}

impl ModelProvider for DiscoveryProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

fn build_agent(server: Arc<dyn MCPServer>) -> Agent {
    Agent::builder("assistant").mcp_server(server).build()
}

fn approve_lookup(mut state: RunState) -> RunState {
    state.approve_for_tool(
        "call-1",
        Some("lookup".to_owned()),
        Some("approved".to_owned()),
    );
    state
}

async fn run_after_approval(
    agent: &Agent,
    provider: Arc<DiscoveryProvider>,
    state: RunState,
) -> RunResult {
    Runner::new()
        .with_model_provider(provider)
        .resume_with_agent(&state, agent)
        .await
        .expect("approved resume should succeed")
}

#[tokio::test]
async fn mcp_discovered_tools_preserve_required_inputs_and_approval() {
    let state = Arc::new(FakeServerState::default());
    let server = Arc::new(FakeServer {
        state: state.clone(),
    }) as Arc<dyn MCPServer>;
    let agent = build_agent(server.clone());

    let tools = MCPUtil::get_function_tools_connected(
        server,
        None,
        RunContextWrapper::new(RunContext::default()),
        agent.clone(),
        None,
    )
    .await
    .expect("discovered tools should load");

    assert_eq!(tools.len(), 1);
    assert!(tools[0].needs_approval);
    assert_eq!(tools[0].definition.namespace.as_deref(), Some("mcp"));
    assert_eq!(
        tools[0]
            .definition
            .input_json_schema
            .as_ref()
            .and_then(|schema| schema.get("required")),
        Some(&json!(["q"]))
    );

    let missing_required = tools[0]
        .invoke(
            agents_core::ToolContext::new(
                RunContextWrapper::new(RunContext::default()),
                "lookup",
                "call-missing",
                "{}",
            ),
            json!({}),
        )
        .await
        .expect_err("missing required arguments should be rejected locally");
    assert!(
        missing_required
            .to_string()
            .contains("missing required argument(s): q")
    );

    let non_object = tools[0]
        .invoke(
            agents_core::ToolContext::new(
                RunContextWrapper::new(RunContext::default()),
                "lookup",
                "call-non-object",
                "\"hello\"",
            ),
            json!("hello"),
        )
        .await
        .expect_err("non-object arguments should be rejected locally");
    assert!(
        non_object
            .to_string()
            .contains("requires object arguments that match its input schema")
    );
    assert_eq!(state.tool_calls.load(Ordering::SeqCst), 0);

    let provider = Arc::new(DiscoveryProvider {
        model: Arc::new(DiscoveryModel::default()),
    });
    let initial = Runner::new()
        .with_model_provider(provider.clone())
        .run(&agent, "hello")
        .await
        .expect("initial run should interrupt");

    assert!(initial.final_output.is_none());
    assert_eq!(initial.interruptions.len(), 1);
    assert!(matches!(
        initial
            .interruptions
            .first()
            .and_then(|step| step.kind.clone()),
        Some(RunInterruptionKind::ToolApproval)
    ));
    assert_eq!(state.tool_calls.load(Ordering::SeqCst), 0);

    let resumed = run_after_approval(
        &agent,
        provider,
        approve_lookup(
            initial
                .durable_state()
                .cloned()
                .expect("interrupted run should have durable state"),
        ),
    )
    .await;

    assert_eq!(
        resumed.final_output.as_deref(),
        Some("done:lookup:{\"q\":\"rust\"}")
    );
    assert!(resumed.new_items.iter().any(|item| {
        matches!(
            item,
            RunItem::ToolCallOutput {
                tool_name,
                namespace,
                ..
            } if tool_name == "lookup" && namespace.as_deref() == Some("mcp")
        )
    }));
    assert_eq!(state.tool_calls.load(Ordering::SeqCst), 1);
}
