use serde::{Deserialize, Serialize};

use crate::guardrail::{InputGuardrail, OutputGuardrail};
use crate::handoff::Handoff;
use crate::tool::{FunctionTool, StaticTool};

/// High-level agent definition.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub instructions: Option<String>,
    pub tools: Vec<StaticTool>,
    #[serde(skip, default)]
    pub function_tools: Vec<FunctionTool>,
    pub handoffs: Vec<Handoff>,
    pub input_guardrails: Vec<InputGuardrail>,
    pub output_guardrails: Vec<OutputGuardrail>,
    pub model: Option<String>,
}

impl Agent {
    pub fn builder(name: impl Into<String>) -> AgentBuilder {
        AgentBuilder::new(name)
    }

    pub fn tool_definitions(&self) -> Vec<crate::tool::ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    pub fn find_function_tool(&self, name: &str, namespace: Option<&str>) -> Option<&FunctionTool> {
        self.function_tools.iter().find(|tool| {
            tool.definition.name == name && tool.definition.namespace.as_deref() == namespace
        })
    }

    pub fn find_handoff(&self, target: &str) -> Option<&Handoff> {
        self.handoffs
            .iter()
            .find(|handoff| handoff.target == target)
    }
}

/// Builder for [`Agent`].
#[derive(Clone, Debug)]
pub struct AgentBuilder {
    agent: Agent,
}

impl AgentBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            agent: Agent {
                name: name.into(),
                ..Agent::default()
            },
        }
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.agent.instructions = Some(instructions.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.agent.model = Some(model.into());
        self
    }

    pub fn tool(mut self, tool: StaticTool) -> Self {
        self.agent.tools.push(tool);
        self
    }

    pub fn function_tool(mut self, tool: FunctionTool) -> Self {
        self.agent.tools.push(StaticTool {
            definition: tool.definition.clone(),
        });
        self.agent.function_tools.push(tool);
        self
    }

    pub fn handoff(mut self, handoff: Handoff) -> Self {
        self.agent.handoffs.push(handoff);
        self
    }

    pub fn handoff_to_agent(mut self, agent: Agent) -> Self {
        self.agent.handoffs.push(Handoff::to_agent(agent));
        self
    }

    pub fn input_guardrail(mut self, guardrail: InputGuardrail) -> Self {
        self.agent.input_guardrails.push(guardrail);
        self
    }

    pub fn output_guardrail(mut self, guardrail: OutputGuardrail) -> Self {
        self.agent.output_guardrails.push(guardrail);
        self
    }

    pub fn build(self) -> Agent {
        self.agent
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::Deserialize;

    use crate::tool::function_tool;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct SearchArgs {
        query: String,
    }

    #[test]
    fn stores_runtime_function_tools_and_serialized_definitions() {
        let tool = function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, crate::errors::AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");

        let agent = Agent::builder("assistant").function_tool(tool).build();

        assert_eq!(agent.tools.len(), 1);
        assert_eq!(agent.function_tools.len(), 1);
        assert!(agent.find_function_tool("search", None).is_some());
    }
}
