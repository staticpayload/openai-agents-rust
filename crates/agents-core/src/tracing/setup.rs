use std::sync::{Arc, OnceLock, RwLock};

use crate::tracing::processor_interface::TracingProcessor;
use crate::tracing::processors::default_processor;
use crate::tracing::provider::{DefaultTraceProvider, TraceProvider};

static GLOBAL_TRACE_PROVIDER: OnceLock<RwLock<Arc<dyn TraceProvider>>> = OnceLock::new();

fn provider_cell() -> &'static RwLock<Arc<dyn TraceProvider>> {
    GLOBAL_TRACE_PROVIDER.get_or_init(|| {
        let provider = Arc::new(DefaultTraceProvider::default()) as Arc<dyn TraceProvider>;
        provider.register_processor(default_processor());
        RwLock::new(provider)
    })
}

pub fn set_trace_provider(provider: Arc<dyn TraceProvider>) {
    *provider_cell().write().expect("trace provider lock") = provider;
}

pub fn get_trace_provider() -> Arc<dyn TraceProvider> {
    provider_cell().read().expect("trace provider lock").clone()
}

pub fn register_processor(processor: Arc<dyn TracingProcessor>) {
    get_trace_provider().register_processor(processor);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::tracing::TracingProcessor;
    use crate::tracing::provider::TraceProvider;

    #[derive(Default)]
    struct DummyProvider {
        shutdown_calls: Mutex<usize>,
    }

    impl TraceProvider for DummyProvider {
        fn register_processor(&self, _processor: Arc<dyn TracingProcessor>) {}
        fn set_processors(&self, _processors: Vec<Arc<dyn TracingProcessor>>) {}
        fn get_current_trace(&self) -> Option<crate::tracing::Trace> {
            None
        }
        fn get_current_span(&self) -> Option<crate::tracing::Span> {
            None
        }
        fn set_disabled(&self, _disabled: bool) {}
        fn time_iso(&self) -> String {
            String::new()
        }
        fn gen_trace_id(&self) -> uuid::Uuid {
            uuid::Uuid::new_v4()
        }
        fn gen_span_id(&self) -> uuid::Uuid {
            uuid::Uuid::new_v4()
        }
        fn gen_group_id(&self) -> String {
            String::new()
        }
        fn create_trace(
            &self,
            _name: &str,
            _trace_id: Option<uuid::Uuid>,
            _group_id: Option<String>,
            _metadata: Option<std::collections::BTreeMap<String, serde_json::Value>>,
            _tracing: Option<&crate::tracing::TracingConfig>,
            _disabled: bool,
        ) -> crate::tracing::Trace {
            crate::tracing::Trace::new("dummy")
        }
        fn create_span(
            &self,
            _name: &str,
            _span_data: crate::tracing::SpanData,
            _span_id: Option<uuid::Uuid>,
            _parent: Option<&crate::tracing::Span>,
            _trace: Option<&crate::tracing::Trace>,
            _disabled: bool,
        ) -> crate::tracing::Span {
            crate::tracing::Span::new(uuid::Uuid::new_v4(), "dummy")
        }
        fn start_trace(&self, _trace: &mut crate::tracing::Trace, _mark_as_current: bool) {}
        fn finish_trace(&self, _trace: &mut crate::tracing::Trace, _reset_current: bool) {}
        fn start_span(&self, _span: &mut crate::tracing::Span, _mark_as_current: bool) {}
        fn finish_span(&self, _span: &mut crate::tracing::Span, _reset_current: bool) {}
        fn shutdown(&self) {
            *self.shutdown_calls.lock().expect("shutdown lock") += 1;
        }
    }

    #[test]
    fn replaces_global_provider() {
        let provider = Arc::new(DummyProvider::default());
        let provider_trait: Arc<dyn TraceProvider> = provider.clone();
        set_trace_provider(provider_trait);

        let current = get_trace_provider();
        current.shutdown();

        assert_eq!(*provider.shutdown_calls.lock().expect("shutdown lock"), 1);
    }
}
