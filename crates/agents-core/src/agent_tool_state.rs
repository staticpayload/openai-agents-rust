use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::result::RunResult;
use crate::run_context::RunContextWrapper;

type ScopedKey = (Option<String>, String);

fn storage() -> &'static Mutex<HashMap<ScopedKey, RunResult>> {
    static STORAGE: OnceLock<Mutex<HashMap<ScopedKey, RunResult>>> = OnceLock::new();
    STORAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_agent_tool_state_scope<TContext>(
    context: &RunContextWrapper<TContext>,
) -> Option<String> {
    context.agent_tool_state_scope.clone()
}

pub fn set_agent_tool_state_scope<TContext>(
    context: &mut RunContextWrapper<TContext>,
    scope_id: Option<String>,
) {
    context.agent_tool_state_scope = scope_id;
}

pub fn record_agent_tool_run_result(
    tool_call_id: impl Into<String>,
    run_result: RunResult,
    scope_id: Option<String>,
) {
    storage()
        .lock()
        .expect("agent tool state storage")
        .insert((scope_id, tool_call_id.into()), run_result);
}

pub fn consume_agent_tool_run_result(
    tool_call_id: &str,
    scope_id: Option<String>,
) -> Option<RunResult> {
    storage()
        .lock()
        .expect("agent tool state storage")
        .remove(&(scope_id, tool_call_id.to_owned()))
}

pub fn peek_agent_tool_run_result(
    tool_call_id: &str,
    scope_id: Option<String>,
) -> Option<RunResult> {
    storage()
        .lock()
        .expect("agent tool state storage")
        .get(&(scope_id, tool_call_id.to_owned()))
        .cloned()
}

pub fn drop_agent_tool_run_result(tool_call_id: &str, scope_id: Option<String>) {
    let _ = storage()
        .lock()
        .expect("agent tool state storage")
        .remove(&(scope_id, tool_call_id.to_owned()));
}

#[cfg(test)]
mod tests {
    use crate::{Agent, OutputItem, RunResult};

    use super::*;

    #[test]
    fn stores_and_consumes_scoped_results() {
        let result = RunResult {
            agent_name: Agent::builder("assistant").build().name,
            output: vec![OutputItem::Text {
                text: "ok".to_owned(),
            }],
            ..RunResult::default()
        };
        record_agent_tool_run_result("call-1", result.clone(), Some("scope-a".to_owned()));
        assert!(peek_agent_tool_run_result("call-1", Some("scope-a".to_owned())).is_some());
        assert!(consume_agent_tool_run_result("call-1", Some("scope-a".to_owned())).is_some());
        assert!(peek_agent_tool_run_result("call-1", Some("scope-a".to_owned())).is_none());
    }
}
