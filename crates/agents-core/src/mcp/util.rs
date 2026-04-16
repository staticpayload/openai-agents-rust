use std::sync::Arc;

use serde_json::Value;

use crate::_mcp_tool_metadata::{resolve_mcp_tool_description_for_model, resolve_mcp_tool_title};
use crate::agent::Agent;
use crate::errors::{AgentsError, Result};
use crate::exceptions::UserError;
use crate::mcp::server::{MCPServer, MCPTool};
use crate::run_context::{RunContext, RunContextWrapper};
use crate::tool::{FunctionTool, ToolDefinition};

#[derive(Clone)]
pub struct ToolFilterContext {
    pub run_context: RunContextWrapper<RunContext>,
    pub agent: Agent,
    pub server_name: String,
}

pub type ToolFilterCallable =
    Arc<dyn Fn(ToolFilterContext, &MCPTool) -> bool + Send + Sync + 'static>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolFilterStatic {
    pub allowed_tool_names: Option<Vec<String>>,
    pub blocked_tool_names: Option<Vec<String>>,
}

#[derive(Clone)]
pub enum ToolFilter {
    Callable(ToolFilterCallable),
    Static(ToolFilterStatic),
}

#[derive(Clone)]
pub struct MCPToolMetaContext {
    pub run_context: RunContextWrapper<RunContext>,
    pub server_name: String,
    pub tool_name: String,
    pub arguments: Option<Value>,
}

pub type MCPToolMetaResolver =
    Arc<dyn Fn(MCPToolMetaContext) -> Option<Value> + Send + Sync + 'static>;

pub fn create_static_tool_filter(
    allowed_tool_names: Option<Vec<String>>,
    blocked_tool_names: Option<Vec<String>>,
) -> Option<ToolFilterStatic> {
    if allowed_tool_names.is_none() && blocked_tool_names.is_none() {
        None
    } else {
        Some(ToolFilterStatic {
            allowed_tool_names,
            blocked_tool_names,
        })
    }
}

pub struct MCPUtil;

impl MCPUtil {
    fn validate_tool_arguments(
        tool_name: &str,
        schema: Option<&Value>,
        args: &Value,
    ) -> Result<()> {
        let Some(schema) = schema else {
            return Ok(());
        };

        let Value::Object(arguments) = args else {
            return Err(AgentsError::User(UserError {
                message: format!(
                    "tool `{tool_name}` requires object arguments that match its input schema"
                ),
            }));
        };

        let missing_required = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|required| !arguments.contains_key(*required))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        if missing_required.is_empty() {
            return Ok(());
        }

        Err(AgentsError::User(UserError {
            message: format!(
                "tool `{tool_name}` is missing required argument(s): {}",
                missing_required.join(", ")
            ),
        }))
    }

    pub fn tool_allowed(
        filter: Option<&ToolFilter>,
        context: ToolFilterContext,
        tool: &MCPTool,
    ) -> bool {
        match filter {
            None => true,
            Some(ToolFilter::Callable(filter)) => filter(context, tool),
            Some(ToolFilter::Static(filter)) => {
                if let Some(allowed) = &filter.allowed_tool_names {
                    if !allowed.iter().any(|name| name == &tool.name) {
                        return false;
                    }
                }
                if let Some(blocked) = &filter.blocked_tool_names {
                    if blocked.iter().any(|name| name == &tool.name) {
                        return false;
                    }
                }
                true
            }
        }
    }

    pub async fn list_tools_filtered(
        server: Arc<dyn MCPServer>,
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
    ) -> Result<Vec<MCPTool>> {
        server.connect().await?;
        let tools =
            Self::list_tools_filtered_connected(server.clone(), filter, run_context, agent).await;
        server.cleanup().await?;
        tools
    }

    pub async fn list_tools_filtered_connected(
        server: Arc<dyn MCPServer>,
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
    ) -> Result<Vec<MCPTool>> {
        let server_name = server.name().to_owned();
        let tools = server.list_tools().await?;
        Ok(tools
            .into_iter()
            .filter(|tool| {
                Self::tool_allowed(
                    filter,
                    ToolFilterContext {
                        run_context: run_context.clone(),
                        agent: agent.clone(),
                        server_name: server_name.clone(),
                    },
                    tool,
                )
            })
            .collect())
    }

    pub fn to_function_tool(
        server: Arc<dyn MCPServer>,
        tool: &MCPTool,
        meta_resolver: Option<MCPToolMetaResolver>,
        run_context: RunContextWrapper<RunContext>,
    ) -> Result<FunctionTool> {
        let tool_value =
            serde_json::to_value(tool).map_err(|error| AgentsError::message(error.to_string()))?;
        let title = resolve_mcp_tool_title(&tool_value);
        let description = resolve_mcp_tool_description_for_model(&tool_value);

        let mut definition = ToolDefinition::new(&tool.name, description);
        if let Some(title) = title {
            definition.description = format!("{title}: {}", definition.description);
        }
        if let Some(schema) = &tool.input_schema {
            let strict = crate::strict_schema::ensure_strict_json_schema(schema.clone())?;
            definition = definition.with_input_json_schema(strict);
        }
        if let Some(namespace) = &tool.namespace {
            definition = definition.with_namespace(namespace.clone());
        }

        let tool_name = tool.name.clone();
        let server_name = server.name().to_owned();
        let needs_approval = tool.requires_approval;
        let input_schema = tool.input_schema.clone();
        let function_tool = FunctionTool::new(
            definition,
            Arc::new(move |_tool_context, args| {
                let server = server.clone();
                let tool_name = tool_name.clone();
                let meta_resolver = meta_resolver.clone();
                let run_context = run_context.clone();
                let server_name = server_name.clone();
                let input_schema = input_schema.clone();
                Box::pin(async move {
                    Self::validate_tool_arguments(&tool_name, input_schema.as_ref(), &args)?;
                    server.connect().await?;
                    let meta = meta_resolver.and_then(|resolver| {
                        resolver(MCPToolMetaContext {
                            run_context,
                            server_name,
                            tool_name: tool_name.clone(),
                            arguments: Some(args.clone()),
                        })
                    });
                    let result = server.call_tool(&tool_name, args, meta).await;
                    server.cleanup().await?;
                    result
                })
            }),
        )
        .with_needs_approval(needs_approval);

        Ok(function_tool)
    }

    pub fn to_function_tool_connected(
        server: Arc<dyn MCPServer>,
        tool: &MCPTool,
        meta_resolver: Option<MCPToolMetaResolver>,
        run_context: RunContextWrapper<RunContext>,
    ) -> Result<FunctionTool> {
        Self::to_function_tool(server, tool, meta_resolver, run_context)
    }

    pub async fn get_function_tools(
        server: Arc<dyn MCPServer>,
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
        meta_resolver: Option<MCPToolMetaResolver>,
    ) -> Result<Vec<FunctionTool>> {
        let tools =
            Self::list_tools_filtered(server.clone(), filter, run_context.clone(), agent).await?;
        tools
            .iter()
            .map(|tool| {
                Self::to_function_tool(
                    server.clone(),
                    tool,
                    meta_resolver.clone(),
                    run_context.clone(),
                )
            })
            .collect::<Result<Vec<_>>>()
    }

    pub async fn get_function_tools_connected(
        server: Arc<dyn MCPServer>,
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
        meta_resolver: Option<MCPToolMetaResolver>,
    ) -> Result<Vec<FunctionTool>> {
        let tools =
            Self::list_tools_filtered_connected(server.clone(), filter, run_context.clone(), agent)
                .await?;
        tools
            .iter()
            .map(|tool| {
                Self::to_function_tool_connected(
                    server.clone(),
                    tool,
                    meta_resolver.clone(),
                    run_context.clone(),
                )
            })
            .collect::<Result<Vec<_>>>()
    }

    pub async fn get_all_function_tools(
        servers: &[Arc<dyn MCPServer>],
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
        meta_resolver: Option<MCPToolMetaResolver>,
    ) -> Result<Vec<FunctionTool>> {
        Self::get_all_function_tools_connected(servers, filter, run_context, agent, meta_resolver)
            .await
    }

    pub async fn get_all_function_tools_connected(
        servers: &[Arc<dyn MCPServer>],
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
        meta_resolver: Option<MCPToolMetaResolver>,
    ) -> Result<Vec<FunctionTool>> {
        let mut tools = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for server in servers {
            for tool in Self::get_function_tools_connected(
                server.clone(),
                filter,
                run_context.clone(),
                agent.clone(),
                meta_resolver.clone(),
            )
            .await?
            {
                if !seen.insert(tool.definition.name.clone()) {
                    return Err(AgentsError::message(format!(
                        "duplicate MCP tool name `{}` across servers",
                        tool.definition.name
                    )));
                }
                tools.push(tool);
            }
        }

        Ok(tools)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::internal::tool_execution::execute_local_function_tools;
    use crate::items::RunItem;
    use crate::mcp::server::MCPToolAnnotations;
    use crate::run_config::RunConfig;
    use crate::tool::{Tool, ToolOutput};
    use crate::tool_context::ToolCall;

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
            "test-server"
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
                description: None,
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

    #[tokio::test]
    async fn converts_mcp_tools_into_function_tools() {
        let server = Arc::new(FakeServer {
            state: Arc::new(FakeServerState::default()),
        }) as Arc<dyn MCPServer>;
        let tools = MCPUtil::get_function_tools(
            server,
            None,
            RunContextWrapper::new(RunContext::default()),
            Agent::builder("assistant").build(),
            None,
        )
        .await
        .expect("tools should load");

        assert_eq!(tools.len(), 1);
        assert!(tools[0].needs_approval);
        assert_eq!(tools[0].definition.namespace.as_deref(), Some("mcp"));

        let output = tools[0]
            .invoke(
                crate::tool_context::ToolContext::new(
                    RunContextWrapper::new(RunContext::default()),
                    "lookup",
                    "call-1",
                    "{\"q\":\"hello\"}",
                ),
                json!({"q":"hello"}),
            )
            .await
            .expect("tool should run");
        assert!(matches!(output, ToolOutput::Text(_)));
    }

    #[tokio::test]
    async fn applies_static_and_callable_tool_filters() {
        let server = Arc::new(FakeServer {
            state: Arc::new(FakeServerState::default()),
        }) as Arc<dyn MCPServer>;
        let run_context = RunContextWrapper::new(RunContext::default());
        let agent = Agent::builder("assistant").build();

        let static_tools = MCPUtil::list_tools_filtered_connected(
            server.clone(),
            Some(&ToolFilter::Static(ToolFilterStatic {
                allowed_tool_names: Some(vec!["lookup".to_owned()]),
                blocked_tool_names: Some(vec!["other".to_owned()]),
            })),
            run_context.clone(),
            agent.clone(),
        )
        .await
        .expect("static filter should succeed");
        assert_eq!(static_tools.len(), 1);

        let callable_filter: ToolFilterCallable =
            Arc::new(|context, tool| context.server_name == "test-server" && tool.name == "lookup");
        let callable_tools = MCPUtil::list_tools_filtered_connected(
            server,
            Some(&ToolFilter::Callable(callable_filter)),
            run_context,
            agent,
        )
        .await
        .expect("callable filter should succeed");
        assert_eq!(callable_tools.len(), 1);
    }

    #[tokio::test]
    async fn metadata_resolver_and_approval_mapping_flow_into_function_tool() {
        let server = Arc::new(FakeServer {
            state: Arc::new(FakeServerState::default()),
        }) as Arc<dyn MCPServer>;
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let resolver_captured = captured.clone();
        let resolver: MCPToolMetaResolver = Arc::new(move |context| {
            resolver_captured
                .lock()
                .expect("capture mutex")
                .push((context.server_name, context.tool_name));
            Some(json!({"request_id":"req-123"}))
        });
        let run_context = RunContextWrapper::new(RunContext::default());
        let agent = Agent::builder("assistant").build();
        let tools = MCPUtil::get_function_tools_connected(
            server,
            None,
            run_context.clone(),
            agent,
            Some(resolver),
        )
        .await
        .expect("tools should load");

        assert_eq!(tools.len(), 1);
        assert!(tools[0].needs_approval);
        assert_eq!(tools[0].definition.namespace.as_deref(), Some("mcp"));

        let output = tools[0]
            .invoke(
                crate::tool_context::ToolContext::new(
                    run_context,
                    "lookup",
                    "call-1",
                    "{\"q\":\"hello\"}",
                ),
                json!({"q":"hello"}),
            )
            .await
            .expect("tool should run");

        assert!(matches!(output, ToolOutput::Text(_)));
        assert_eq!(
            captured.lock().expect("capture mutex").as_slice(),
            [("test-server".to_owned(), "lookup".to_owned())]
        );
    }

    #[tokio::test]
    async fn mcp_discovered_tools_preserve_required_inputs_and_approval() {
        let state = Arc::new(FakeServerState::default());
        let server = Arc::new(FakeServer {
            state: state.clone(),
        }) as Arc<dyn MCPServer>;
        let run_context = RunContextWrapper::new(RunContext::default());
        let agent = Agent::builder("assistant")
            .mcp_server(server.clone())
            .build();
        let tools = MCPUtil::get_function_tools_connected(
            server.clone(),
            None,
            run_context.clone(),
            agent.clone(),
            None,
        )
        .await
        .expect("tools should load");

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
                crate::tool_context::ToolContext::new(
                    run_context.clone(),
                    "lookup",
                    "call-missing",
                    "{}",
                ),
                json!({}),
            )
            .await
            .expect_err("missing required arguments should fail locally");
        assert!(
            missing_required
                .to_string()
                .contains("missing required argument(s): q")
        );

        let non_object = tools[0]
            .invoke(
                crate::tool_context::ToolContext::new(
                    run_context.clone(),
                    "lookup",
                    "call-non-object",
                    "\"hello\"",
                ),
                json!("hello"),
            )
            .await
            .expect_err("non-object arguments should fail locally");
        assert!(
            non_object
                .to_string()
                .contains("requires object arguments that match its input schema")
        );
        assert_eq!(state.tool_calls.load(Ordering::SeqCst), 0);

        let outcome = execute_local_function_tools(
            &agent,
            &RunConfig::default(),
            &run_context,
            vec![ToolCall {
                id: "call-approval".to_owned(),
                name: "lookup".to_owned(),
                arguments: serde_json::to_string(&json!({"q":"rust"})).expect("json"),
                namespace: Some("mcp".to_owned()),
            }],
            None,
            None,
        )
        .await
        .expect("approval-gated discovery should interrupt");

        assert_eq!(outcome.interruptions.len(), 1);
        assert!(outcome.new_items.is_empty());
        assert!(matches!(
            outcome.interruptions[0].kind,
            Some(crate::run_state::RunInterruptionKind::ToolApproval)
        ));
        assert_eq!(state.tool_calls.load(Ordering::SeqCst), 0);

        let approved = execute_local_function_tools(
            &agent,
            &RunConfig::default(),
            &run_context,
            vec![ToolCall {
                id: "call-approved".to_owned(),
                name: "lookup".to_owned(),
                arguments: serde_json::to_string(&json!({"q":"rust"})).expect("json"),
                namespace: Some("mcp".to_owned()),
            }],
            None,
            Some((
                &crate::run_state::RunInterruption {
                    kind: Some(crate::run_state::RunInterruptionKind::ToolApproval),
                    approval_id: Some("approval-1".to_owned()),
                    call_id: Some("call-approved".to_owned()),
                    tool_name: Some("lookup".to_owned()),
                    namespace: Some("mcp".to_owned()),
                    reason: Some("tool approval required".to_owned()),
                },
                &crate::run_context::ApprovalRecord {
                    approved: true,
                    reason: Some("approved".to_owned()),
                    approval_id: Some("approval-1".to_owned()),
                    call_id: Some("call-approved".to_owned()),
                    tool_name: Some("lookup".to_owned()),
                    namespace: Some("mcp".to_owned()),
                },
            )),
        )
        .await
        .expect("approved execution should succeed");

        assert!(approved.interruptions.is_empty());
        assert!(approved.new_items.iter().any(|item| {
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
}
