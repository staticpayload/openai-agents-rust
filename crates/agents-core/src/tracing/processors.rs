use std::sync::Arc;

use crate::tracing::processor_interface::{TracingExporter, TracingProcessor};
use crate::tracing::{Span, Trace};

#[derive(Default)]
pub struct ConsoleSpanExporter;

impl TracingExporter for ConsoleSpanExporter {
    fn export_trace(&self, _trace: &Trace) {}

    fn export_span(&self, _span: &Span) {}
}

pub struct BatchTraceProcessor {
    exporter: Arc<dyn TracingExporter>,
}

impl BatchTraceProcessor {
    pub fn new(exporter: Arc<dyn TracingExporter>) -> Self {
        Self { exporter }
    }
}

impl TracingProcessor for BatchTraceProcessor {
    fn on_trace_start(&self, _trace: &Trace) {}

    fn on_trace_end(&self, trace: &Trace) {
        self.exporter.export_trace(trace);
    }

    fn on_span_start(&self, _span: &Span) {}

    fn on_span_end(&self, span: &Span) {
        self.exporter.export_span(span);
    }
}

pub fn default_exporter() -> Arc<dyn TracingExporter> {
    Arc::new(ConsoleSpanExporter)
}

pub fn default_processor() -> Arc<dyn TracingProcessor> {
    Arc::new(BatchTraceProcessor::new(default_exporter()))
}
