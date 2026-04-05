use std::sync::Arc;

use async_trait::async_trait;
use openai_agents::{
    Agent, Model, ModelProvider, ModelRequest, ModelResponse, OpenAIConversationsSession,
    OpenAIResponsesCompactionMode, OpenAIResponsesCompactionSession, OutputItem, Runner, Session,
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
