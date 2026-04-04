use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::tracing::span_data::SpanData;
use crate::tracing::util::{gen_span_id, time_iso};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpanError {
    pub message: String,
    pub data: Option<Value>,
}

/// Span metadata attached to a trace.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<SpanError>,
    pub data: SpanData,
    pub disabled: bool,
}

impl Span {
    pub fn new(trace_id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id: gen_span_id(),
            trace_id,
            parent_id: None,
            name: name.into(),
            started_at: None,
            ended_at: None,
            error: None,
            data: SpanData::Custom(crate::tracing::span_data::CustomSpanData::default()),
            disabled: false,
        }
    }

    pub fn with_parent(mut self, parent_id: Option<Uuid>) -> Self {
        self.parent_id = parent_id;
        self
    }

    pub fn with_data(mut self, data: SpanData) -> Self {
        self.data = data;
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

    pub fn set_error(&mut self, message: impl Into<String>, data: Option<Value>) {
        self.error = Some(SpanError {
            message: message.into(),
            data,
        });
    }
}
