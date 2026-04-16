use std::sync::{Arc, Mutex};

use agents_core::{
    Agent, AgentsError, InputItem, MemorySession, Model, ModelProvider, ModelRequest,
    ModelResponse, OutputItem, Result, RunConfig, RunOptions, Runner, Session, SessionSettings,
    Usage, function_tool,
};
use async_trait::async_trait;
use futures::FutureExt;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchArgs {
    query: String,
}

#[derive(Clone, Default)]
struct ConversationDeltaCaptureModel {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

#[async_trait]
impl Model for ConversationDeltaCaptureModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let mut requests = self.requests.lock().expect("conversation requests lock");
        requests.push(request.clone());

        if requests.len() == 1 {
            return Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: "call-1".to_owned(),
                    tool_name: "search".to_owned(),
                    arguments: json!({"query":"rust"}),
                    namespace: None,
                }],
                usage: Usage::default(),
                response_id: Some("resp-1".to_owned()),
                request_id: Some("req-1".to_owned()),
            });
        }

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: "done".to_owned(),
            }],
            usage: Usage::default(),
            response_id: Some("resp-2".to_owned()),
            request_id: Some("req-2".to_owned()),
        })
    }
}

#[derive(Clone)]
struct StaticProvider<M> {
    model: Arc<M>,
}

impl<M> ModelProvider for StaticProvider<M>
where
    M: Model + 'static,
{
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[derive(Clone, Default)]
struct AutoPreviousResponseModel {
    calls: Arc<Mutex<usize>>,
    seen_previous_response_ids: Arc<Mutex<Vec<Option<String>>>>,
}

#[async_trait]
impl Model for AutoPreviousResponseModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        self.seen_previous_response_ids
            .lock()
            .expect("previous response ids lock")
            .push(request.previous_response_id.clone());

        let mut calls = self.calls.lock().expect("auto previous calls lock");
        *calls += 1;

        if *calls == 1 {
            return Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: "call-1".to_owned(),
                    tool_name: "search".to_owned(),
                    arguments: json!({"query":"rust"}),
                    namespace: None,
                }],
                usage: Usage::default(),
                response_id: Some("resp-auto-1".to_owned()),
                request_id: Some("req-auto-1".to_owned()),
            });
        }

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: "done".to_owned(),
            }],
            usage: Usage::default(),
            response_id: Some("resp-auto-2".to_owned()),
            request_id: Some("req-auto-2".to_owned()),
        })
    }
}

#[derive(Clone, Default)]
struct SessionCaptureModel {
    seen_inputs: Arc<Mutex<Vec<Vec<InputItem>>>>,
}

#[async_trait]
impl Model for SessionCaptureModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        self.seen_inputs
            .lock()
            .expect("session inputs lock")
            .push(request.input.clone());

        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: "session-ok".to_owned(),
            }],
            usage: Usage::default(),
            response_id: None,
            request_id: None,
        })
    }
}

#[tokio::test]
async fn conversation_id_only_sends_new_items_after_first_turn() {
    let model = Arc::new(ConversationDeltaCaptureModel::default());
    let provider = Arc::new(StaticProvider {
        model: model.clone(),
    });
    let search_tool =
        function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
    let runner = Runner::new()
        .with_model_provider(provider)
        .with_config(RunConfig {
            conversation_id: Some("conv-delta".to_owned()),
            ..RunConfig::default()
        });
    let agent = Agent::builder("assistant")
        .function_tool(search_tool)
        .build();

    let result = runner
        .run(&agent, "hello")
        .await
        .expect("conversation-backed run should succeed");

    assert_eq!(result.final_output.as_deref(), Some("done"));

    let requests = model.requests.lock().expect("requests lock").clone();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].conversation_id.as_deref(), Some("conv-delta"));
    assert_eq!(requests[1].conversation_id.as_deref(), Some("conv-delta"));
    assert_eq!(
        requests[0]
            .input
            .iter()
            .filter_map(InputItem::as_text)
            .collect::<Vec<_>>(),
        vec!["hello"]
    );
    assert!(
        requests[1]
            .input
            .iter()
            .all(|item| item.as_text() != Some("hello"))
    );
    assert!(requests[1].input.iter().any(|item| matches!(
        item,
        InputItem::Json { value }
            if value.get("type").and_then(serde_json::Value::as_str) == Some("tool_call_output")
    )));
    assert_eq!(result.conversation_id(), Some("conv-delta"));
}

#[tokio::test]
async fn auto_previous_response_id_chains_across_turns() {
    let model = Arc::new(AutoPreviousResponseModel::default());
    let provider = Arc::new(StaticProvider {
        model: model.clone(),
    });
    let search_tool =
        function_tool(
            "search",
            "Search documents",
            |_ctx, args: SearchArgs| async move {
                Ok::<_, AgentsError>(format!("result:{}", args.query))
            },
        )
        .expect("function tool should build");
    let agent = Agent::builder("assistant")
        .function_tool(search_tool)
        .build();

    let result = Runner::new()
        .with_model_provider(provider)
        .with_config(RunConfig {
            auto_previous_response_id: true,
            ..RunConfig::default()
        })
        .run(&agent, "hello")
        .await
        .expect("run should succeed");

    assert_eq!(result.final_output.as_deref(), Some("done"));
    assert_eq!(
        model
            .seen_previous_response_ids
            .lock()
            .expect("previous response ids lock")
            .as_slice(),
        &[None, Some("resp-auto-1".to_owned())]
    );
    assert_eq!(result.previous_response_id(), Some("resp-auto-2"));
}

#[tokio::test]
async fn session_persistence_rejects_server_managed_state() {
    let agent = Agent::builder("assistant").build();
    let session: Arc<dyn Session + Sync> = Arc::new(MemorySession::new("session"));

    for runner in [
        Runner::new().with_config(RunConfig {
            conversation_id: Some("conv_123".to_owned()),
            ..RunConfig::default()
        }),
        Runner::new().with_config(RunConfig {
            previous_response_id: Some("resp_123".to_owned()),
            ..RunConfig::default()
        }),
        Runner::new().with_config(RunConfig {
            auto_previous_response_id: true,
            ..RunConfig::default()
        }),
    ] {
        let error = runner
            .run_with_session(&agent, "hello", session.as_ref())
            .await
            .expect_err("session-backed runs should reject server-managed state");
        assert!(matches!(error, AgentsError::User(_)));
        assert!(
            error
                .to_string()
                .contains("Session persistence cannot be combined")
        );
    }

    let streamed_error = Runner::new()
        .with_config(RunConfig {
            conversation_id: Some("conv_streamed".to_owned()),
            ..RunConfig::default()
        })
        .run_streamed_with_options(
            &agent,
            vec![InputItem::from("hello")],
            RunOptions {
                session: Some(session.clone()),
                ..RunOptions::default()
            },
        )
        .await
        .expect("streamed run should start")
        .wait_for_completion()
        .await
        .expect_err("streamed run should reject server-managed state");
    assert!(matches!(streamed_error, AgentsError::User(_)));
    assert!(
        streamed_error
            .to_string()
            .contains("Session persistence cannot be combined")
    );
}

#[tokio::test]
async fn session_settings_limit_bounds_injected_history() {
    let agent = Agent::builder("assistant").build();

    for (label, session, expected) in [
        (
            "limit-zero",
            MemorySession::new("session-zero").with_settings(SessionSettings { limit: Some(0) }),
            vec!["hello"],
        ),
        (
            "limit-two",
            MemorySession::new("session-two").with_settings(SessionSettings { limit: Some(2) }),
            vec!["b", "c", "hello"],
        ),
        (
            "limit-none",
            MemorySession::new("session-none").with_settings(SessionSettings { limit: None }),
            vec!["a", "b", "c", "hello"],
        ),
    ] {
        let model = Arc::new(SessionCaptureModel::default());
        let provider = Arc::new(StaticProvider {
            model: model.clone(),
        });
        session
            .add_items(vec![
                InputItem::from("a"),
                InputItem::from("b"),
                InputItem::from("c"),
            ])
            .await
            .expect("history should be stored");

        let result = Runner::new()
            .with_model_provider(provider)
            .run_with_session(&agent, "hello", &session)
            .await
            .expect("session-backed run should succeed");

        assert_eq!(
            result.final_output.as_deref(),
            Some("session-ok"),
            "{label}"
        );
        assert_eq!(
            model
                .seen_inputs
                .lock()
                .expect("session inputs lock")
                .first()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(InputItem::as_text)
                .collect::<Vec<_>>(),
            expected,
            "{label}"
        );
    }
}

#[tokio::test]
async fn runner_uses_session_input_callback_to_prepare_history() {
    let model = Arc::new(SessionCaptureModel::default());
    let provider = Arc::new(StaticProvider {
        model: model.clone(),
    });
    let session = MemorySession::new("session");
    session
        .add_items(vec![
            InputItem::from("history-1"),
            InputItem::from("history-2"),
        ])
        .await
        .expect("history should be stored");
    let agent = Agent::builder("assistant").build();

    let result = Runner::new()
        .with_model_provider(provider)
        .with_config(RunConfig {
            session_input_callback: Some(Arc::new(|history, new_items| {
                async move {
                    Ok(vec![
                        history
                            .last()
                            .cloned()
                            .unwrap_or_else(|| InputItem::from("")),
                        new_items
                            .first()
                            .cloned()
                            .unwrap_or_else(|| InputItem::from("")),
                    ])
                }
                .boxed()
            })),
            ..RunConfig::default()
        })
        .run_with_session(&agent, "hello", &session)
        .await
        .expect("session-backed run should succeed");

    let seen_inputs = model.seen_inputs.lock().expect("session inputs lock");
    assert_eq!(seen_inputs.len(), 1);
    assert_eq!(seen_inputs[0].len(), 2);
    assert_eq!(seen_inputs[0][0].as_text(), Some("history-2"));
    assert_eq!(seen_inputs[0][1].as_text(), Some("hello"));
    drop(seen_inputs);

    let persisted = session
        .get_items()
        .await
        .expect("session items should load");
    assert_eq!(persisted.len(), 4);
    assert_eq!(persisted[0].as_text(), Some("history-1"));
    assert_eq!(persisted[1].as_text(), Some("history-2"));
    assert_eq!(persisted[2].as_text(), Some("hello"));
    assert_eq!(persisted[3].as_text(), Some("session-ok"));
    assert_eq!(result.final_output.as_deref(), Some("session-ok"));
}
