use std::fmt;

use thiserror::Error;

/// Collected data attached to runtime failures.
#[derive(Clone, Debug, Default)]
pub struct RunErrorDetails {
    pub message: Option<String>,
    pub agent_name: Option<String>,
    pub turns_completed: usize,
}

impl fmt::Display for RunErrorDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "run error details(agent={:?}, turns_completed={}, message={:?})",
            self.agent_name, self.turns_completed, self.message
        )
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct MaxTurnsExceeded {
    pub message: String,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ModelBehaviorError {
    pub message: String,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct UserError {
    pub message: String,
}

#[derive(Debug, Error)]
#[error("tool `{tool_name}` timed out after {timeout_seconds} seconds")]
pub struct ToolTimeoutError {
    pub tool_name: String,
    pub timeout_seconds: f64,
}

#[derive(Debug, Error)]
#[error("input guardrail triggered: {guardrail_name}")]
pub struct InputGuardrailTripwireTriggered {
    pub guardrail_name: String,
}
