use std::collections::{HashMap, HashSet};

use crate::agent::Agent;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentToolUseTracker {
    agent_to_tools: HashMap<String, HashSet<String>>,
}

#[allow(dead_code)]
impl AgentToolUseTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_tool_use(
        &mut self,
        agent: &Agent,
        tool_names: impl IntoIterator<Item = String>,
    ) {
        let entry = self.agent_to_tools.entry(agent.name.clone()).or_default();
        entry.extend(tool_names);
    }

    pub(crate) fn has_used_tools(&self, agent: &Agent) -> bool {
        self.agent_to_tools
            .get(&agent.name)
            .map(|tools| !tools.is_empty())
            .unwrap_or(false)
    }

    pub(crate) fn as_serializable(&self) -> HashMap<String, Vec<String>> {
        self.agent_to_tools
            .iter()
            .map(|(name, tools)| {
                let mut values = tools.iter().cloned().collect::<Vec<_>>();
                values.sort();
                (name.clone(), values)
            })
            .collect()
    }

    pub(crate) fn from_serializable(snapshot: HashMap<String, Vec<String>>) -> Self {
        let agent_to_tools = snapshot
            .into_iter()
            .map(|(name, tools)| (name, tools.into_iter().collect()))
            .collect();
        Self { agent_to_tools }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_tools_by_agent_name() {
        let mut tracker = AgentToolUseTracker::new();
        let agent = Agent::builder("router").build();

        tracker.add_tool_use(&agent, ["search".to_owned(), "lookup".to_owned()]);

        assert!(tracker.has_used_tools(&agent));
        assert_eq!(
            tracker.as_serializable().get("router"),
            Some(&vec!["lookup".to_owned(), "search".to_owned()])
        );
    }
}
