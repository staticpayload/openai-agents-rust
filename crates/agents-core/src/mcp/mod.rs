//! Model Context Protocol support.

pub mod manager;
pub mod server;
pub mod util;

pub use manager::MCPServerManager;
pub use server::{
    MCPServer, MCPServerSseParams, MCPServerStdio, MCPServerStdioParams, MCPServerStreamableHttp,
    MCPServerStreamableHttpParams, MCPTool, MCPToolAnnotations, RequireApprovalObject,
    RequireApprovalToolList,
};
pub use util::{
    MCPToolMetaContext, MCPToolMetaResolver, MCPUtil, ToolFilter, ToolFilterCallable,
    ToolFilterContext, ToolFilterStatic, create_static_tool_filter,
};
