use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use futures::StreamExt;
use openai_agents::{
    Agent, MCPReadResourceResult, MCPResource, MCPResourceContents, MCPResourceTemplate, MCPServer,
    MCPServerManager, MCPServerStreamableHttp, MCPServerStreamableHttpParams,
    MCPTextResourceContents, MCPTool, Model, ModelProvider, ModelRequest, ModelResponse,
    OutputItem, Result, RunItem, Runner, ToolOutput,
};
use serde_json::{Value, json};

#[derive(Default)]
struct FakeMcpServerState {
    connect_calls: AtomicUsize,
    cleanup_calls: AtomicUsize,
    list_calls: AtomicUsize,
    tool_calls: AtomicUsize,
    fail_first_connect: bool,
}

struct FakeMcpServer {
    name: String,
    state: Arc<FakeMcpServerState>,
}

impl FakeMcpServer {
    fn new(name: &str) -> (Arc<Self>, Arc<FakeMcpServerState>) {
        let state = Arc::new(FakeMcpServerState::default());
        (
            Arc::new(Self {
                name: name.to_owned(),
                state: state.clone(),
            }),
            state,
        )
    }

    fn flaky(name: &str) -> (Arc<Self>, Arc<FakeMcpServerState>) {
        let state = Arc::new(FakeMcpServerState {
            fail_first_connect: true,
            ..FakeMcpServerState::default()
        });
        (
            Arc::new(Self {
                name: name.to_owned(),
                state: state.clone(),
            }),
            state,
        )
    }
}

#[async_trait]
impl MCPServer for FakeMcpServer {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect(&self) -> Result<()> {
        let count = self.state.connect_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.state.fail_first_connect && count == 1 {
            return Err(openai_agents::AgentsError::message(
                "temporary connect failure",
            ));
        }
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        self.state.cleanup_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<MCPTool>> {
        self.state.list_calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![MCPTool {
            name: "lookup".to_owned(),
            description: Some("Lookup MCP data".to_owned()),
            input_schema: Some(json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            })),
            namespace: Some(self.name.clone()),
            ..MCPTool::default()
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
struct FakeMcpModel {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Model for FakeMcpModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: "call-mcp-1".to_owned(),
                    tool_name: "lookup".to_owned(),
                    arguments: json!({ "query": "rust" }),
                    namespace: Some("docs".to_owned()),
                }],
                usage: Default::default(),
                response_id: Some("resp-mcp-1".to_owned()),
                request_id: Some("req-mcp-1".to_owned()),
            });
        }

        let tool_result = request
            .input
            .iter()
            .filter_map(|item| match item {
                openai_agents::InputItem::Json { value } => Some(value),
                openai_agents::InputItem::Text { .. } => None,
            })
            .find_map(|value| {
                value
                    .get("type")
                    .and_then(Value::as_str)
                    .filter(|kind| *kind == "tool_call_output")
                    .and_then(|_| value.get("output"))
                    .and_then(|output| output.get("text"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "missing".to_owned());

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: format!("done:{tool_result}"),
            }],
            usage: Default::default(),
            response_id: Some("resp-mcp-2".to_owned()),
            request_id: Some("req-mcp-2".to_owned()),
        })
    }
}

#[derive(Clone)]
struct FakeMcpProvider {
    model: Arc<dyn Model>,
}

impl ModelProvider for FakeMcpProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[tokio::test]
async fn runner_calls_mcp_tools_in_non_streamed_runs() {
    let (server, server_state) = FakeMcpServer::new("docs");
    let provider = FakeMcpProvider {
        model: Arc::new(FakeMcpModel::default()),
    };
    let agent = Agent::builder("assistant")
        .mcp_server(server as Arc<dyn MCPServer>)
        .build();

    let result = Runner::new()
        .with_model_provider(Arc::new(provider))
        .run(&agent, "hello")
        .await
        .expect("run should succeed");

    assert_eq!(
        result.final_output.as_deref(),
        Some("done:lookup:{\"query\":\"rust\"}")
    );
    assert!(server_state.list_calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(server_state.tool_calls.load(Ordering::SeqCst), 1);
    assert!(result.new_items.iter().any(|item| {
        matches!(
            item,
            RunItem::ToolCallOutput {
                tool_name,
                call_id,
                namespace,
                ..
            } if tool_name == "lookup"
                && call_id.as_deref() == Some("call-mcp-1")
                && namespace.as_deref() == Some("docs")
        )
    }));
}

#[tokio::test]
async fn runner_calls_mcp_tools_in_streamed_runs() {
    let (server, server_state) = FakeMcpServer::new("docs");
    let provider = FakeMcpProvider {
        model: Arc::new(FakeMcpModel::default()),
    };
    let agent = Agent::builder("assistant")
        .mcp_server(server as Arc<dyn MCPServer>)
        .build();

    let streamed = Runner::new()
        .with_model_provider(Arc::new(provider))
        .run_streamed(&agent, "hello")
        .await
        .expect("streamed run should start");

    let events = streamed.stream_events().collect::<Vec<_>>().await;
    let result = streamed
        .wait_for_completion()
        .await
        .expect("streamed run should complete");

    assert_eq!(
        result.final_output.as_deref(),
        Some("done:lookup:{\"query\":\"rust\"}")
    );
    assert!(!events.is_empty());
    assert_eq!(server_state.tool_calls.load(Ordering::SeqCst), 1);
    assert!(result.new_items.iter().any(|item| {
        matches!(
            item,
            RunItem::ToolCallOutput {
                tool_name,
                call_id,
                namespace,
                ..
            } if tool_name == "lookup"
                && call_id.as_deref() == Some("call-mcp-1")
                && namespace.as_deref() == Some("docs")
        )
    }));
}

#[tokio::test]
async fn mcp_server_manager_reconnects_failed_servers_and_cleans_up() {
    let (stable, stable_state) = FakeMcpServer::new("stable");
    let (flaky, flaky_state) = FakeMcpServer::flaky("flaky");
    let mut manager = MCPServerManager::new(vec![
        stable as Arc<dyn MCPServer>,
        flaky as Arc<dyn MCPServer>,
    ]);

    let first_active = manager
        .connect_all()
        .await
        .expect("initial connect should succeed");
    assert_eq!(first_active.len(), 1);
    assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);

    let retried_active = manager
        .reconnect(true)
        .await
        .expect("retry should reconnect failed server");
    assert_eq!(retried_active.len(), 2);

    manager.cleanup_all().await.expect("cleanup should succeed");

    assert_eq!(stable_state.cleanup_calls.load(Ordering::SeqCst), 1);
    assert_eq!(flaky_state.cleanup_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn streamable_http_server_exposes_resources_once_connected() {
    let server = MCPServerStreamableHttp::new(
        "docs",
        MCPServerStreamableHttpParams {
            url: "http://localhost:8000/mcp".to_owned(),
        },
    )
    .with_resources(vec![MCPResource {
        uri: "file:///readme.md".to_owned(),
        name: "readme.md".to_owned(),
        mime_type: Some("text/markdown".to_owned()),
        ..MCPResource::default()
    }])
    .with_resource_templates(vec![MCPResourceTemplate {
        uri_template: "file:///{path}".to_owned(),
        name: "file".to_owned(),
        ..MCPResourceTemplate::default()
    }])
    .with_resource_content(
        "file:///readme.md",
        MCPReadResourceResult {
            contents: vec![MCPResourceContents::Text(MCPTextResourceContents {
                uri: "file:///readme.md".to_owned(),
                text: "# Hello".to_owned(),
                mime_type: Some("text/markdown".to_owned()),
            })],
        },
    );

    let error = server
        .list_resources(None)
        .await
        .expect_err("resources should require connection");
    assert!(matches!(error, openai_agents::AgentsError::User(_)));

    server.connect().await.expect("connect should succeed");
    let resources = server
        .list_resources(None)
        .await
        .expect("resources should load");
    let templates = server
        .list_resource_templates(None)
        .await
        .expect("resource templates should load");
    let content = server
        .read_resource("file:///readme.md")
        .await
        .expect("resource contents should load");

    assert_eq!(resources.resources.len(), 1);
    assert_eq!(templates.resource_templates.len(), 1);
    assert_eq!(content.contents.len(), 1);
}
