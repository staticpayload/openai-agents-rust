use serde::{Deserialize, Serialize};

use crate::agent::Agent;

/// Declarative handoff metadata for routing between agents.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Handoff {
    pub target: String,
    pub description: Option<String>,
    #[serde(skip, default)]
    pub agent: Option<Box<Agent>>,
}

impl Handoff {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            description: None,
            agent: None,
        }
    }

    pub fn to_agent(agent: Agent) -> Self {
        Self {
            target: agent.name.clone(),
            description: None,
            agent: Some(Box::new(agent)),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_agent(mut self, agent: Agent) -> Self {
        self.target = agent.name.clone();
        self.agent = Some(Box::new(agent));
        self
    }

    pub fn runtime_agent(&self) -> Option<&Agent> {
        self.agent.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_runtime_agent_target() {
        let specialist = Agent::builder("specialist").build();
        let handoff = Handoff::to_agent(specialist.clone());

        assert_eq!(handoff.target, "specialist");
        assert_eq!(
            handoff.runtime_agent().map(|agent| agent.name.as_str()),
            Some("specialist")
        );
    }
}
