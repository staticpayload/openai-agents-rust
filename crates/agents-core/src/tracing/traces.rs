use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::tracing::config::TracingConfig;
use crate::tracing::util::{gen_trace_id, time_iso};

/// Trace metadata for an agent run.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Trace {
    pub id: Uuid,
    pub workflow_name: String,
    pub group_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
    pub tracing_api_key: Option<String>,
    pub disabled: bool,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            id: gen_trace_id(),
            workflow_name: String::new(),
            group_id: None,
            metadata: BTreeMap::new(),
            tracing_api_key: None,
            disabled: false,
            started_at: None,
            ended_at: None,
        }
    }
}

impl Trace {
    pub fn new(workflow_name: impl Into<String>) -> Self {
        Self {
            workflow_name: workflow_name.into(),
            ..Self::default()
        }
    }

    pub fn with_config(mut self, config: Option<&TracingConfig>) -> Self {
        if let Some(config) = config {
            self.tracing_api_key = config.api_key.clone();
            self.disabled = config.disabled;
        }
        self
    }

    pub fn start(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(time_iso());
        }
    }

    pub fn finish(&mut self) {
        self.ended_at = Some(time_iso());
    }
}
