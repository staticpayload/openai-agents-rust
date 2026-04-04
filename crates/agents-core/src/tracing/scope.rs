use std::cell::RefCell;

use crate::tracing::{Span, Trace};

thread_local! {
    static CURRENT_TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
    static CURRENT_SPAN: RefCell<Option<Span>> = const { RefCell::new(None) };
}

pub struct Scope;

impl Scope {
    pub fn get_current_trace() -> Option<Trace> {
        CURRENT_TRACE.with(|trace| trace.borrow().clone())
    }

    pub fn set_current_trace(trace: Option<Trace>) {
        CURRENT_TRACE.with(|current| *current.borrow_mut() = trace);
    }

    pub fn get_current_span() -> Option<Span> {
        CURRENT_SPAN.with(|span| span.borrow().clone())
    }

    pub fn set_current_span(span: Option<Span>) {
        CURRENT_SPAN.with(|current| *current.borrow_mut() = span);
    }
}
