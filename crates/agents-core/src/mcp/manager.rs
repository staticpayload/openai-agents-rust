use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::errors::Result;
use crate::mcp::server::{MCPServer, MCPTool};

/// Manages MCP server lifecycles and exposes only connected servers.
#[derive(Clone, Default)]
pub struct MCPServerManager {
    all_servers: Vec<Arc<dyn MCPServer>>,
    active_servers: Vec<Arc<dyn MCPServer>>,
    connected_server_names: HashSet<String>,
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
            connected_server_names: HashSet::new(),
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

    pub fn active_server_names(&self) -> Vec<String> {
        self.active_servers
            .iter()
            .map(|server| server.name().to_owned())
            .collect()
    }

    async fn connect_server(&mut self, server: &Arc<dyn MCPServer>) -> Result<bool> {
        match server.connect().await {
            Ok(()) => {
                self.connected_server_names.insert(server.name().to_owned());
                Ok(true)
            }
            Err(error) => {
                let name = server.name().to_owned();
                if !self.failed_servers.iter().any(|failed| failed == &name) {
                    self.failed_servers.push(name.clone());
                }
                self.errors.insert(name, error.to_string());
                if self.strict {
                    return Err(error);
                }
                Ok(false)
            }
        }
    }

    pub async fn connect_all(&mut self) -> Result<Vec<Arc<dyn MCPServer>>> {
        let previous_connected_server_names = self.connected_server_names.clone();
        let previous_active_servers = self.active_servers.clone();
        self.failed_servers.clear();
        self.errors.clear();
        self.connected_server_names.clear();

        let servers = self.all_servers.clone();
        let mut active = Vec::new();
        for server in &servers {
            match self.connect_server(server).await {
                Ok(true) => active.push(server.clone()),
                Ok(false) => {
                    if !self.drop_failed_servers {
                        active.push(server.clone());
                    }
                }
                Err(error) => {
                    let _ = self.cleanup_connected_servers(Some(server.clone())).await;
                    self.connected_server_names.clear();
                    self.active_servers = if self.drop_failed_servers {
                        self.all_servers
                            .iter()
                            .filter(|server| {
                                previous_connected_server_names.contains(server.name())
                            })
                            .cloned()
                            .collect()
                    } else {
                        previous_active_servers
                    };
                    return Err(error);
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
        self.failed_servers.clear();
        self.errors.clear();

        let mut retried_active = Vec::new();
        for server in retry_servers {
            if self.connect_server(&server).await? {
                retried_active.push(server);
            }
        }

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

    pub async fn list_tools_for_active(&self) -> Result<Vec<(Arc<dyn MCPServer>, Vec<MCPTool>)>> {
        let mut results = Vec::new();
        for server in &self.active_servers {
            results.push((server.clone(), server.list_tools().await?));
        }
        Ok(results)
    }

    pub async fn cleanup_all(&mut self) -> Result<()> {
        self.cleanup_connected_servers(None).await?;
        self.connected_server_names.clear();
        self.active_servers.clear();
        Ok(())
    }

    async fn cleanup_connected_servers(
        &mut self,
        extra_server: Option<Arc<dyn MCPServer>>,
    ) -> Result<()> {
        let connected = self.connected_server_names.clone();
        let mut cleaned = HashSet::new();

        if let Some(server) = extra_server {
            cleaned.insert(server.name().to_owned());
            server.cleanup().await?;
        }

        for server in self.all_servers.iter().rev() {
            if connected.contains(server.name()) && cleaned.insert(server.name().to_owned()) {
                server.cleanup().await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::errors::{AgentsError, Result};
    use crate::mcp::server::{MCPServer, MCPTool};
    use crate::tool::ToolOutput;

    struct FakeServer {
        name: String,
        fail_connect: bool,
        tools: Vec<MCPTool>,
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
            Ok(self.tools.clone())
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

    struct CountingServer {
        name: String,
        fail_connects_remaining: AtomicUsize,
        cleanup_calls: AtomicUsize,
    }

    #[async_trait]
    impl MCPServer for CountingServer {
        fn name(&self) -> &str {
            &self.name
        }

        async fn connect(&self) -> Result<()> {
            let remaining = self.fail_connects_remaining.load(Ordering::SeqCst);
            if remaining > 0 {
                self.fail_connects_remaining.fetch_sub(1, Ordering::SeqCst);
                Err(AgentsError::message("connect failed"))
            } else {
                Ok(())
            }
        }

        async fn cleanup(&self) -> Result<()> {
            self.cleanup_calls.fetch_add(1, Ordering::SeqCst);
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
                tools: vec![MCPTool::new("lookup")],
            }) as Arc<dyn MCPServer>,
            Arc::new(FakeServer {
                name: "bad".to_owned(),
                fail_connect: true,
                tools: Vec::new(),
            }) as Arc<dyn MCPServer>,
        ]);

        let active = manager.connect_all().await.expect("connect should succeed");

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name(), "ok");
        assert_eq!(manager.failed_servers, vec!["bad".to_owned()]);
    }

    #[tokio::test]
    async fn reconnect_deduplicates_failed_server_names() {
        let mut manager = MCPServerManager::new(vec![Arc::new(FakeServer {
            name: "flaky".to_owned(),
            fail_connect: true,
            tools: Vec::new(),
        }) as Arc<dyn MCPServer>]);

        manager.connect_all().await.expect("connect should succeed");
        manager.reconnect(true).await.expect("retry should succeed");

        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);
    }

    #[tokio::test]
    async fn strict_connect_cleans_up_connected_servers_and_clears_state() {
        let stable = Arc::new(CountingServer {
            name: "stable".to_owned(),
            fail_connects_remaining: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
        });
        let flaky = Arc::new(CountingServer {
            name: "flaky".to_owned(),
            fail_connects_remaining: AtomicUsize::new(1),
            cleanup_calls: AtomicUsize::new(0),
        });
        let mut manager = MCPServerManager::new(vec![
            stable.clone() as Arc<dyn MCPServer>,
            flaky.clone() as Arc<dyn MCPServer>,
        ]);
        manager.strict = true;

        let error = manager
            .connect_all()
            .await
            .err()
            .expect("strict connect should fail");
        assert!(error.to_string().contains("connect failed"));
        assert_eq!(stable.cleanup_calls.load(Ordering::SeqCst), 1);
        assert!(manager.active_servers().is_empty());
        assert!(manager.connected_server_names.is_empty());
    }

    #[tokio::test]
    async fn strict_connect_preserves_existing_active_servers() {
        let stable = Arc::new(CountingServer {
            name: "stable".to_owned(),
            fail_connects_remaining: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
        });
        let flaky = Arc::new(CountingServer {
            name: "flaky".to_owned(),
            fail_connects_remaining: AtomicUsize::new(2),
            cleanup_calls: AtomicUsize::new(0),
        });
        let mut manager = MCPServerManager::new(vec![
            stable.clone() as Arc<dyn MCPServer>,
            flaky.clone() as Arc<dyn MCPServer>,
        ]);

        manager
            .connect_all()
            .await
            .expect("initial connect should succeed");
        assert_eq!(manager.active_server_names(), vec!["stable".to_owned()]);
        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);

        manager.strict = true;
        let error = manager
            .connect_all()
            .await
            .err()
            .expect("strict connect should fail");

        assert!(error.to_string().contains("connect failed"));
        assert_eq!(manager.active_server_names(), vec!["stable".to_owned()]);
        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);
        assert_eq!(stable.cleanup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(flaky.cleanup_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn strict_connect_preserves_existing_failed_active_servers_when_not_dropping_failed() {
        let stable = Arc::new(CountingServer {
            name: "stable".to_owned(),
            fail_connects_remaining: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
        });
        let flaky = Arc::new(CountingServer {
            name: "flaky".to_owned(),
            fail_connects_remaining: AtomicUsize::new(2),
            cleanup_calls: AtomicUsize::new(0),
        });
        let mut manager = MCPServerManager::new(vec![
            stable.clone() as Arc<dyn MCPServer>,
            flaky.clone() as Arc<dyn MCPServer>,
        ]);
        manager.drop_failed_servers = false;

        manager
            .connect_all()
            .await
            .expect("initial connect should succeed");
        assert_eq!(
            manager.active_server_names(),
            vec!["stable".to_owned(), "flaky".to_owned()]
        );
        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);

        manager.strict = true;
        let error = manager
            .connect_all()
            .await
            .err()
            .expect("strict connect should fail");

        assert!(error.to_string().contains("connect failed"));
        assert_eq!(
            manager.active_server_names(),
            vec!["stable".to_owned(), "flaky".to_owned()]
        );
        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);
        assert_eq!(stable.cleanup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(flaky.cleanup_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn strict_connect_restores_full_active_set_on_first_failure_when_not_dropping_failed() {
        let stable = Arc::new(CountingServer {
            name: "stable".to_owned(),
            fail_connects_remaining: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
        });
        let flaky = Arc::new(CountingServer {
            name: "flaky".to_owned(),
            fail_connects_remaining: AtomicUsize::new(1),
            cleanup_calls: AtomicUsize::new(0),
        });
        let mut manager = MCPServerManager::new(vec![
            stable.clone() as Arc<dyn MCPServer>,
            flaky.clone() as Arc<dyn MCPServer>,
        ]);
        manager.drop_failed_servers = false;
        manager.strict = true;

        let error = manager
            .connect_all()
            .await
            .err()
            .expect("strict connect should fail");

        assert!(error.to_string().contains("connect failed"));
        assert_eq!(
            manager.active_server_names(),
            vec!["stable".to_owned(), "flaky".to_owned()]
        );
        assert_eq!(manager.failed_servers, vec!["flaky".to_owned()]);
        assert_eq!(stable.cleanup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(flaky.cleanup_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reconnect_failed_only_retries_failed_subset() {
        let stable = Arc::new(CountingServer {
            name: "stable".to_owned(),
            fail_connects_remaining: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
        });
        let flaky = Arc::new(CountingServer {
            name: "flaky".to_owned(),
            fail_connects_remaining: AtomicUsize::new(1),
            cleanup_calls: AtomicUsize::new(0),
        });
        let mut manager = MCPServerManager::new(vec![
            stable.clone() as Arc<dyn MCPServer>,
            flaky.clone() as Arc<dyn MCPServer>,
        ]);

        manager
            .connect_all()
            .await
            .expect("initial connect should succeed");
        assert_eq!(manager.active_server_names(), vec!["stable".to_owned()]);

        manager
            .reconnect(true)
            .await
            .expect("retry should reconnect only failed server");

        assert_eq!(manager.active_server_names().len(), 2);
        assert_eq!(stable.cleanup_calls.load(Ordering::SeqCst), 0);
        assert_eq!(flaky.cleanup_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cleanup_clears_active_servers_and_connected_names() {
        let mut manager = MCPServerManager::new(vec![Arc::new(FakeServer {
            name: "ok".to_owned(),
            fail_connect: false,
            tools: Vec::new(),
        }) as Arc<dyn MCPServer>]);

        manager.connect_all().await.expect("connect should succeed");
        manager.cleanup_all().await.expect("cleanup should succeed");

        assert!(manager.active_servers().is_empty());
        assert!(manager.connected_server_names.is_empty());
    }

    #[tokio::test]
    async fn lists_tools_for_active_servers() {
        let mut manager = MCPServerManager::new(vec![Arc::new(FakeServer {
            name: "ok".to_owned(),
            fail_connect: false,
            tools: vec![MCPTool::new("lookup")],
        }) as Arc<dyn MCPServer>]);

        manager.connect_all().await.expect("connect should succeed");
        let listed = manager
            .list_tools_for_active()
            .await
            .expect("list tools should succeed");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0.name(), "ok");
        assert_eq!(listed[0].1.len(), 1);
        assert_eq!(listed[0].1[0].name, "lookup");
    }
}
