use crate::tracing::{Span, Trace};

pub trait TracingProcessor: Send + Sync {
    fn on_trace_start(&self, trace: &Trace);
    fn on_trace_end(&self, trace: &Trace);
    fn on_span_start(&self, span: &Span);
    fn on_span_end(&self, span: &Span);
    fn shutdown(&self) {}
    fn force_flush(&self) {}
}

pub trait TracingExporter: Send + Sync {
    fn export_trace(&self, trace: &Trace);
    fn export_span(&self, span: &Span);
}
