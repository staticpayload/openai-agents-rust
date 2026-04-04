use std::fmt;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::Result;
use crate::tool::ToolOutput;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequireApprovalToolList {
    #[serde(default)]
    pub tool_names: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequireApprovalObject {
    pub always: Option<RequireApprovalToolList>,
    pub never: Option<RequireApprovalToolList>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MCPToolAnnotations {
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MCPTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub title: Option<String>,
    pub annotations: Option<MCPToolAnnotations>,
    pub meta: Option<Value>,
    pub namespace: Option<String>,
    pub requires_approval: bool,
}

impl MCPTool {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}

#[async_trait]
pub trait MCPServer: Send + Sync {
    fn name(&self) -> &str;

    async fn connect(&self) -> Result<()>;

    async fn cleanup(&self) -> Result<()>;

    async fn list_tools(&self) -> Result<Vec<MCPTool>>;

    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        meta: Option<Value>,
    ) -> Result<ToolOutput>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MCPServerStdioParams {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MCPServerSseParams {
    pub url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MCPServerStreamableHttpParams {
    pub url: String,
}

#[derive(Clone)]
pub struct MCPServerStdio {
    name: String,
    pub params: MCPServerStdioParams,
}

impl fmt::Debug for MCPServerStdio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MCPServerStdio")
            .field("name", &self.name)
            .field("params", &self.params)
            .finish()
    }
}

impl MCPServerStdio {
    pub fn new(name: impl Into<String>, params: MCPServerStdioParams) -> Self {
        Self {
            name: name.into(),
            params,
        }
    }
}

#[async_trait]
impl MCPServer for MCPServerStdio {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<MCPTool>> {
        Ok(Vec::new())
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        _arguments: Value,
        _meta: Option<Value>,
    ) -> Result<ToolOutput> {
        Ok(ToolOutput::from(format!("mcp:{tool_name}")))
    }
}

#[derive(Clone)]
pub struct MCPServerStreamableHttp {
    name: String,
    pub params: MCPServerStreamableHttpParams,
}

impl fmt::Debug for MCPServerStreamableHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MCPServerStreamableHttp")
            .field("name", &self.name)
            .field("params", &self.params)
            .finish()
    }
}

impl MCPServerStreamableHttp {
    pub fn new(name: impl Into<String>, params: MCPServerStreamableHttpParams) -> Self {
        Self {
            name: name.into(),
            params,
        }
    }
}

#[async_trait]
impl MCPServer for MCPServerStreamableHttp {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<MCPTool>> {
        Ok(Vec::new())
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        _arguments: Value,
        _meta: Option<Value>,
    ) -> Result<ToolOutput> {
        Ok(ToolOutput::from(format!("mcp:{tool_name}")))
    }
}
