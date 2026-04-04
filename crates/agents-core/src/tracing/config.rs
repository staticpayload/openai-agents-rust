use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for tracing export.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TracingConfig {
    pub api_key: Option<String>,
    pub disabled: bool,
}
