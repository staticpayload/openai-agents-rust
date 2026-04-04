use thiserror::Error;

use crate::exceptions::{
    InputGuardrailTripwireTriggered, MaxTurnsExceeded, ModelBehaviorError,
    OutputGuardrailTripwireTriggered, ToolInputGuardrailTripwireTriggered,
    ToolOutputGuardrailTripwireTriggered, ToolTimeoutError, UserError,
};

/// Errors produced by the Rust Agents SDK scaffold.
#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("model provider is not configured")]
    ModelProviderNotConfigured,
    #[error("model provider resolved no model")]
    ModelUnavailable,
    #[error("{message}")]
    Message { message: String },
    #[error(transparent)]
    MaxTurnsExceeded(#[from] MaxTurnsExceeded),
    #[error(transparent)]
    ModelBehavior(#[from] ModelBehaviorError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    ToolTimeout(#[from] ToolTimeoutError),
    #[error(transparent)]
    InputGuardrailTripwire(#[from] InputGuardrailTripwireTriggered),
    #[error(transparent)]
    OutputGuardrailTripwire(#[from] OutputGuardrailTripwireTriggered),
    #[error(transparent)]
    ToolInputGuardrailTripwire(#[from] ToolInputGuardrailTripwireTriggered),
    #[error(transparent)]
    ToolOutputGuardrailTripwire(#[from] ToolOutputGuardrailTripwireTriggered),
}

impl AgentsError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AgentsError>;
