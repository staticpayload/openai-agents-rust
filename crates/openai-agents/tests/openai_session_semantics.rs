use std::sync::Arc;

use async_trait::async_trait;
use openai_agents::{
    Agent, InputItem, MemorySession, Model, ModelProvider, ModelRequest, ModelResponse,
    OpenAIConversationsSession, OpenAIResponsesCompactionMode, OpenAIResponsesCompactionSession,
    OutputItem, RunConfig, RunOptions, Runner, Session,
};
use tokio::sync::Mutex;

#[derive(Clone)]
struct CapturingModel {
    requests: Arc<Mutex<Vec<(Option<String>, Option<String>)>>>,
    response_id: String,
}

#[async_trait]
impl Model for CapturingModel {
    async fn generate(&self, request: ModelRequest) -> openai_agents::Result<ModelResponse> {
        self.requests.lock().await.push((
            request.previous_response_id.clone(),
            request.conversation_id.clone(),
        ));
        Ok(ModelResponse {
            model: request.model.clone(),
            output: vec![OutputItem::Text {
                text: request
                    .input
                    .last()
                    .and_then(|item| item.as_text())
                    .unwrap_or_default()
                    .to_owned(),
            }],
            usage: Default::default(),
            response_id: Some(self.response_id.clone()),
            request_id: Some("req-1".to_owned()),
        })
    }
}

#[derive(Clone)]
struct CapturingProvider {
    model: Arc<dyn Model>,
}

impl ModelProvider for CapturingProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[derive(Clone)]
struct SequencedCapturingModel {
    requests: Arc<Mutex<Vec<(Option<String>, Option<String>)>>>,
    response_ids: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Model for SequencedCapturingModel {
    async fn generate(&self, request: ModelRequest) -> openai_agents::Result<ModelResponse> {
        self.requests.lock().await.push((
            request.previous_response_id.clone(),
            request.conversation_id.clone(),
        ));
        let response_id = self.response_ids.lock().await.remove(0);
        Ok(ModelResponse {
            model: request.model.clone(),
            output: vec![OutputItem::Text {
                text: request
                    .input
                    .last()
                    .and_then(|item| item.as_text())
                    .unwrap_or_default()
                    .to_owned(),
            }],
            usage: Default::default(),
            response_id: Some(response_id),
            request_id: Some("req-sequenced".to_owned()),
        })
    }
}

#[tokio::test]
async fn runner_uses_and_persists_openai_conversation_session_state() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingProvider {
        model: Arc::new(CapturingModel {
            requests: requests.clone(),
            response_id: "resp-next".to_owned(),
        }),
    };
    let session = OpenAIConversationsSession::new("conv-1");
    session.set_last_response_id("resp-prev").await;
    let runner = Runner::new().with_model_provider(Arc::new(provider));
    let agent = Agent::builder("assistant").build();

    let result = runner
        .run_with_session(&agent, "hello", &session)
        .await
        .expect("run should succeed");

    let captured = requests.lock().await.clone();
    assert_eq!(
        captured,
        vec![(Some("resp-prev".to_owned()), Some("conv-1".to_owned()))]
    );
    assert_eq!(result.previous_response_id(), Some("resp-next"));
    assert_eq!(
        session.last_response_id().await.as_deref(),
        Some("resp-next")
    );
}

#[tokio::test]
async fn session_input_callback_preserves_duplicate_json_provenance_without_public_fields() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingProvider {
        model: Arc::new(CapturingModel {
            requests: requests.clone(),
            response_id: "resp-session".to_owned(),
        }),
    };
    let session = MemorySession::new("session");
    session
        .add_items(vec![InputItem::Json {
            value: serde_json::json!({
                "kind": "duplicate",
                "value": 42,
            }),
        }])
        .await
        .expect("seed items should be stored");

    let runner = Runner::new()
        .with_model_provider(Arc::new(provider))
        .with_config(RunConfig {
            session_input_callback: Some(Arc::new(|_history, mut new_items| {
                Box::pin(async move { Ok(vec![new_items.remove(0)]) })
            })),
            ..RunConfig::default()
        });
    let agent = Agent::builder("assistant").build();

    runner
        .run_items_with_session(
            &agent,
            vec![InputItem::Json {
                value: serde_json::json!({
                    "kind": "duplicate",
                    "value": 42,
                }),
            }],
            &session,
        )
        .await
        .expect("run should succeed");

    let items = session
        .get_items()
        .await
        .expect("session items should load");
    assert_eq!(items.len(), 3);
    assert!(matches!(
        &items[0],
        InputItem::Json { value }
            if value == &serde_json::json!({
                "kind": "duplicate",
                "value": 42,
            })
    ));
    assert!(matches!(
        &items[1],
        InputItem::Json { value }
            if value == &serde_json::json!({
                "kind": "duplicate",
                "value": 42,
            })
    ));
    assert_eq!(items[2].as_text(), Some(""));

    let captured = requests.lock().await.clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0, None);
    assert_eq!(captured[0].1, None);
}

#[tokio::test]
async fn runner_triggers_auto_compaction_for_compaction_sessions() {
    let provider = CapturingProvider {
        model: Arc::new(CapturingModel {
            requests: Arc::new(Mutex::new(Vec::new())),
            response_id: "resp-compacted".to_owned(),
        }),
    };
    let session = OpenAIResponsesCompactionSession::new("session")
        .with_mode(OpenAIResponsesCompactionMode::Auto)
        .with_compaction_threshold(1);
    session
        .add_items(vec![openai_agents::InputItem::Json {
            value: serde_json::json!({
                "type": "tool_call_output",
                "call_id": "call-1",
            }),
        }])
        .await
        .expect("seed items should be stored");
    let runner = Runner::new().with_model_provider(Arc::new(provider));
    let agent = Agent::builder("assistant").build();

    let result = runner
        .run_with_session(&agent, "hello", &session)
        .await
        .expect("run should succeed");

    assert_eq!(result.previous_response_id(), Some("resp-compacted"));
    let items = session
        .get_items()
        .await
        .expect("session items should load");
    assert!(items.iter().any(|item| matches!(
        item,
        openai_agents::InputItem::Json { value }
            if value.get("type").and_then(serde_json::Value::as_str) == Some("compaction")
    )));
}

#[tokio::test]
async fn run_options_override_conversation_tracking_for_one_call_only() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingProvider {
        model: Arc::new(CapturingModel {
            requests: requests.clone(),
            response_id: "resp-next".to_owned(),
        }),
    };
    let runner = Runner::new()
        .with_model_provider(Arc::new(provider))
        .with_config(RunConfig {
            conversation_id: Some("conv-base".to_owned()),
            previous_response_id: Some("resp-base".to_owned()),
            ..RunConfig::default()
        });
    let agent = Agent::builder("assistant").build();

    runner
        .run_with_options(
            &agent,
            vec![InputItem::from("override")],
            RunOptions {
                conversation_id: Some("conv-override".to_owned()),
                previous_response_id: Some("resp-override".to_owned()),
                ..RunOptions::default()
            },
        )
        .await
        .expect("override run should succeed");

    runner
        .run(&agent, "base")
        .await
        .expect("base run should succeed");

    assert_eq!(
        requests.lock().await.clone(),
        vec![
            (
                Some("resp-override".to_owned()),
                Some("conv-override".to_owned())
            ),
            (Some("resp-base".to_owned()), Some("conv-base".to_owned())),
        ]
    );
}

#[tokio::test]
async fn openai_conversation_session_advances_previous_response_id_across_turns() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingProvider {
        model: Arc::new(SequencedCapturingModel {
            requests: requests.clone(),
            response_ids: Arc::new(Mutex::new(vec![
                "resp-first".to_owned(),
                "resp-second".to_owned(),
            ])),
        }),
    };
    let session = OpenAIConversationsSession::new("conv-1");
    let runner = Runner::new().with_model_provider(Arc::new(provider));
    let agent = Agent::builder("assistant").build();

    let first = runner
        .run_with_session(&agent, "hello", &session)
        .await
        .expect("first run should succeed");
    let second = runner
        .run_with_session(&agent, "again", &session)
        .await
        .expect("second run should succeed");

    assert_eq!(first.previous_response_id(), Some("resp-first"));
    assert_eq!(second.previous_response_id(), Some("resp-second"));
    assert_eq!(
        requests.lock().await.clone(),
        vec![
            (None, Some("conv-1".to_owned())),
            (Some("resp-first".to_owned()), Some("conv-1".to_owned())),
        ]
    );
    assert_eq!(
        session.last_response_id().await.as_deref(),
        Some("resp-second")
    );
}
