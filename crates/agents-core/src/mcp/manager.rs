use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::errors::Result;
use crate::mcp::server::MCPServer;

/// Manages MCP server lifecycles and exposes only connected servers.
#[derive(Clone, Default)]
pub struct MCPServerManager {
    all_servers: Vec<Arc<dyn MCPServer>>,
    active_servers: Vec<Arc<dyn MCPServer>>,
    pub failed_servers: Vec<String>,
    pub errors: HashMap<String, String>,
    pub drop_failed_servers: bool,
    pub strict: bool,
}

impl MCPServerManager {
    pub fn new(servers: impl IntoIterator<Item = Arc<dyn MCPServer>>) -> Self {
        let all_servers = servers.into_iter().collect::<Vec<_>>();
        Self {
            active_servers: all_servers.clone(),
            all_servers,
            failed_servers: Vec::new(),
            errors: HashMap::new(),
            drop_failed_servers: true,
            strict: false,
        }
    }

    pub fn active_servers(&self) -> Vec<Arc<dyn MCPServer>> {
        self.active_servers.clone()
    }

    pub fn all_servers(&self) -> Vec<Arc<dyn MCPServer>> {
        self.all_servers.clone()
    }

    pub async fn connect_all(&mut self) -> Result<Vec<Arc<dyn MCPServer>>> {
        self.failed_servers.clear();
        self.errors.clear();

        let mut active = Vec::new();
        for server in &self.all_servers {
            match server.connect().await {
                Ok(()) => active.push(server.clone()),
                Err(error) => {
                    let name = server.name().to_owned();
                    self.failed_servers.push(name.clone());
                    self.errors.insert(name, error.to_string());
                    if self.strict {
                        return Err(error);
                    }
                    if !self.drop_failed_servers {
                        active.push(server.clone());
                    }
                }
            }
        }

        self.active_servers = active;
        Ok(self.active_servers())
    }

    pub async fn reconnect(&mut self, failed_only: bool) -> Result<Vec<Arc<dyn MCPServer>>> {
        if !failed_only {
            self.cleanup_all().await?;
            return self.connect_all().await;
        }

        let failed = self.failed_servers.iter().cloned().collect::<HashSet<_>>();
        let retry_servers = self
            .all_servers
            .iter()
            .filter(|server| failed.contains(server.name()))
            .cloned()
            .collect::<Vec<_>>();
        let mut retry_manager = Self::new(retry_servers);
        retry_manager.drop_failed_servers = self.drop_failed_servers;
        retry_manager.strict = self.strict;
        let retried_active = retry_manager.connect_all().await?;

        self.failed_servers = retry_manager.failed_servers;
        self.errors = retry_manager.errors;

        let mut active = self
            .active_servers
            .iter()
            .filter(|server| !failed.contains(server.name()))
            .cloned()
            .collect::<Vec<_>>();
        active.extend(retried_active);
        self.active_servers = active;
        Ok(self.active_servers())
    }

    pub async fn cleanup_all(&self) -> Result<()> {
        for server in self.all_servers.iter().rev() {
            server.cleanup().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::Value;

    use super::*;
    use crate::errors::{AgentsError, Result};
    use crate::mcp::server::{MCPServer, MCPTool};
    use crate::tool::ToolOutput;

    struct FakeServer {
        name: String,
        fail_connect: bool,
    }

    #[async_trait]
    impl MCPServer for FakeServer {
        fn name(&self) -> &str {
            &self.name
        }

        async fn connect(&self) -> Result<()> {
            if self.fail_connect {
                Err(AgentsError::message("connect failed"))
            } else {
                Ok(())
            }
        }

        async fn cleanup(&self) -> Result<()> {
            Ok(())
        }

        async fn list_tools(&self) -> Result<Vec<MCPTool>> {
            Ok(Vec::new())
        }

        async fn call_tool(
            &self,
            _tool_name: &str,
            _arguments: Value,
            _meta: Option<Value>,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput::from("ok"))
        }
    }

    #[tokio::test]
    async fn drops_failed_servers_when_requested() {
        let mut manager = MCPServerManager::new(vec![
            Arc::new(FakeServer {
                name: "ok".to_owned(),
                fail_connect: false,
            }) as Arc<dyn MCPServer>,
            Arc::new(FakeServer {
                name: "bad".to_owned(),
                fail_connect: true,
            }) as Arc<dyn MCPServer>,
        ]);

        let active = manager.connect_all().await.expect("connect should succeed");

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name(), "ok");
        assert_eq!(manager.failed_servers, vec!["bad".to_owned()]);
    }
}
