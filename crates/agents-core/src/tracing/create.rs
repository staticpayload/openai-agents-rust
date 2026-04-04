use std::collections::BTreeMap;

use serde_json::Value;

use crate::tracing::config::TracingConfig;
use crate::tracing::setup::get_trace_provider;
use crate::tracing::span_data::{
    AgentSpanData, CustomSpanData, FunctionSpanData, GenerationSpanData, GuardrailSpanData,
    HandoffSpanData, MCPListToolsSpanData, ResponseSpanData, SpanData,
};
use crate::tracing::{Span, Trace};

pub fn trace(
    workflow_name: &str,
    trace_id: Option<uuid::Uuid>,
    group_id: Option<String>,
    metadata: Option<BTreeMap<String, Value>>,
    tracing: Option<&TracingConfig>,
    disabled: bool,
) -> Trace {
    get_trace_provider().create_trace(
        workflow_name,
        trace_id,
        group_id,
        metadata,
        tracing,
        disabled,
    )
}

pub fn get_current_trace() -> Option<Trace> {
    get_trace_provider().get_current_trace()
}

pub fn get_current_span() -> Option<Span> {
    get_trace_provider().get_current_span()
}

pub fn agent_span(name: &str, handoffs: Option<Vec<String>>, tools: Option<Vec<String>>) -> Span {
    get_trace_provider().create_span(
        name,
        SpanData::Agent(AgentSpanData {
            name: name.to_owned(),
            handoffs,
            tools,
            output_type: None,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn function_span(name: &str, input: Option<String>, output: Option<String>) -> Span {
    get_trace_provider().create_span(
        name,
        SpanData::Function(FunctionSpanData {
            name: name.to_owned(),
            input,
            output,
            mcp_data: None,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn generation_span(model: Option<String>, usage: Option<Value>) -> Span {
    get_trace_provider().create_span(
        "generation",
        SpanData::Generation(GenerationSpanData {
            input: None,
            output: None,
            model,
            model_config: None,
            usage,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn response_span(response_id: Option<String>) -> Span {
    get_trace_provider().create_span(
        "response",
        SpanData::Response(ResponseSpanData { response_id }),
        None,
        None,
        None,
        false,
    )
}

pub fn handoff_span(from_agent: Option<String>, to_agent: Option<String>) -> Span {
    get_trace_provider().create_span(
        "handoff",
        SpanData::Handoff(HandoffSpanData {
            from_agent,
            to_agent,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn custom_span(name: &str, data: BTreeMap<String, Value>) -> Span {
    get_trace_provider().create_span(
        name,
        SpanData::Custom(CustomSpanData {
            name: name.to_owned(),
            data,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn guardrail_span(name: &str, triggered: bool) -> Span {
    get_trace_provider().create_span(
        name,
        SpanData::Guardrail(GuardrailSpanData {
            name: name.to_owned(),
            triggered,
        }),
        None,
        None,
        None,
        false,
    )
}

pub fn mcp_tools_span(server: &str, tools: Vec<String>) -> Span {
    get_trace_provider().create_span(
        "mcp_list_tools",
        SpanData::MpcListTools(MCPListToolsSpanData {
            server: server.to_owned(),
            tools,
        }),
        None,
        None,
        None,
        false,
    )
}
