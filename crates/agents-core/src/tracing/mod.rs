//! Tracing primitives and global provider setup.

pub mod config;
pub mod context;
pub mod create;
pub mod logger;
pub mod model_tracing;
pub mod processor_interface;
pub mod processors;
pub mod provider;
pub mod scope;
pub mod setup;
pub mod span_data;
pub mod spans;
pub mod traces;
pub mod util;

pub use config::TracingConfig;
pub use context::{TraceCtxManager, create_trace_for_run};
pub use create::{
    agent_span, custom_span, function_span, generation_span, get_current_span, get_current_trace,
    guardrail_span, handoff_span, mcp_tools_span, response_span, trace,
};
pub use model_tracing::get_model_tracing_impl;
pub use processor_interface::{TracingExporter, TracingProcessor};
pub use processors::{
    BatchTraceProcessor, ConsoleSpanExporter, default_exporter, default_processor,
};
pub use provider::{DefaultTraceProvider, SynchronousMultiTracingProcessor, TraceProvider};
pub use setup::{get_trace_provider, set_trace_provider};
pub use span_data::{
    AgentSpanData, CustomSpanData, FunctionSpanData, GenerationSpanData, GuardrailSpanData,
    HandoffSpanData, MCPListToolsSpanData, ResponseSpanData, SpanData,
};
pub use spans::{Span, SpanError};
pub use traces::Trace;
