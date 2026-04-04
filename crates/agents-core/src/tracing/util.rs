use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

pub fn time_iso() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", now.as_secs(), now.subsec_millis())
}

pub fn gen_trace_id() -> Uuid {
    Uuid::new_v4()
}

pub fn gen_span_id() -> Uuid {
    Uuid::new_v4()
}

pub fn gen_group_id() -> String {
    Uuid::new_v4().to_string()
}
