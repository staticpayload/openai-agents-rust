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

pub fn add_trace_processor(processor: Arc<dyn TracingProcessor>) {
    register_processor(processor);
}

pub fn set_trace_processors(processors: Vec<Arc<dyn TracingProcessor>>) {
    get_trace_provider().set_processors(processors);
}

pub fn set_tracing_disabled(disabled: bool) {
    get_trace_provider().set_disabled(disabled);
}

pub fn flush_traces() {
    get_trace_provider().force_flush();
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, OnceLock};

    use super::*;
    use crate::tracing::TracingProcessor;
    use crate::tracing::provider::TraceProvider;

    #[derive(Default)]
    struct DummyProvider {
        shutdown_calls: Mutex<usize>,
        force_flush_calls: Mutex<usize>,
        disabled_values: Mutex<Vec<bool>>,
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
        fn set_disabled(&self, disabled: bool) {
            self.disabled_values
                .lock()
                .expect("disabled values lock")
                .push(disabled);
        }
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

        fn force_flush(&self) {
            *self.force_flush_calls.lock().expect("force flush lock") += 1;
        }
    }

    fn provider_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn replaces_global_provider() {
        let _guard = provider_test_lock().lock().expect("provider test lock");
        let provider = Arc::new(DummyProvider::default());
        let provider_trait: Arc<dyn TraceProvider> = provider.clone();
        set_trace_provider(provider_trait);

        let current = get_trace_provider();
        current.shutdown();

        assert_eq!(*provider.shutdown_calls.lock().expect("shutdown lock"), 1);
    }

    #[test]
    fn forwards_disable_and_flush_to_provider() {
        let _guard = provider_test_lock().lock().expect("provider test lock");
        let provider = Arc::new(DummyProvider::default());
        let provider_trait: Arc<dyn TraceProvider> = provider.clone();
        set_trace_provider(provider_trait);

        set_tracing_disabled(true);
        flush_traces();

        assert_eq!(
            *provider.force_flush_calls.lock().expect("force flush lock"),
            1
        );
        assert_eq!(
            provider
                .disabled_values
                .lock()
                .expect("disabled values lock")
                .as_slice(),
            &[true]
        );
    }
}
