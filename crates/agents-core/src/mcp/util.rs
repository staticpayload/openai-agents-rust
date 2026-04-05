use std::sync::Arc;

use serde_json::Value;

use crate::_mcp_tool_metadata::{resolve_mcp_tool_description_for_model, resolve_mcp_tool_title};
use crate::agent::Agent;
use crate::errors::{AgentsError, Result};
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
        let server_name = server.name().to_owned();
        server.connect().await?;
        let tools = server.list_tools().await;
        server.cleanup().await?;
        let tools = tools?;
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
        let function_tool = FunctionTool::new(
            definition,
            Arc::new(move |_tool_context, args| {
                let server = server.clone();
                let tool_name = tool_name.clone();
                let meta_resolver = meta_resolver.clone();
                let run_context = run_context.clone();
                let server_name = server_name.clone();
                Box::pin(async move {
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

    pub async fn get_all_function_tools(
        servers: &[Arc<dyn MCPServer>],
        filter: Option<&ToolFilter>,
        run_context: RunContextWrapper<RunContext>,
        agent: Agent,
        meta_resolver: Option<MCPToolMetaResolver>,
    ) -> Result<Vec<FunctionTool>> {
        let mut tools = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for server in servers {
            for tool in Self::get_function_tools(
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

    use super::*;
    use crate::mcp::server::MCPToolAnnotations;
    use crate::tool::{Tool, ToolOutput};

    struct FakeServer;

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
            Ok(ToolOutput::from(format!("{tool_name}:{arguments}")))
        }
    }

    #[tokio::test]
    async fn converts_mcp_tools_into_function_tools() {
        let server = Arc::new(FakeServer) as Arc<dyn MCPServer>;
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
}
