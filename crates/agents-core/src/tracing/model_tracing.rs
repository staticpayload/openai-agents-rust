use crate::tracing::{Span, generation_span};

pub fn get_model_tracing_impl(model_name: Option<&str>) -> Span {
    generation_span(model_name.map(ToOwned::to_owned), None)
}
