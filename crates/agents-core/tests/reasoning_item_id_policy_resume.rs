use std::sync::{Arc, Mutex};

use agents_core::{
    Agent, InputItem, Model, ModelProvider, ModelRequest, ModelResponse, OutputItem,
    ReasoningItemIdPolicy, RunContext, RunContextWrapper, RunItem, RunState, Runner,
};
use async_trait::async_trait;

#[derive(Default)]
struct ResumeCaptureModel {
    seen_inputs: Mutex<Vec<Vec<InputItem>>>,
}

#[async_trait]
impl Model for ResumeCaptureModel {
    async fn generate(&self, request: ModelRequest) -> agents_core::Result<ModelResponse> {
        self.seen_inputs
            .lock()
            .expect("resume capture inputs lock")
            .push(request.input.clone());
        Ok(ModelResponse {
            model: request.model,
            output: vec![OutputItem::Text {
                text: "resumed".to_owned(),
            }],
            usage: Default::default(),
            response_id: None,
            request_id: Some("req-resume".to_owned()),
        })
    }
}

struct ResumeCaptureProvider {
    model: Arc<ResumeCaptureModel>,
}

impl ModelProvider for ResumeCaptureProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[tokio::test]
async fn reasoning_item_id_policy_omit_survives_resume() {
    let model = Arc::new(ResumeCaptureModel::default());
    let provider = Arc::new(ResumeCaptureProvider {
        model: model.clone(),
    });
    let agent = Agent::builder("assistant").build();
    let context = RunContextWrapper::new(RunContext::default());
    let mut state = RunState::new(&context, vec![InputItem::from("start")], agent.clone(), 2)
        .expect("run state should build");
    state.normalized_input = Some(vec![InputItem::from("normalized-start")]);
    state.reasoning_item_id_policy = ReasoningItemIdPolicy::Omit;
    state.push_generated_item(RunItem::Reasoning {
        text: "internal".to_owned(),
    });

    let serialized = state
        .to_json_string()
        .expect("run state should serialize with the omit policy");
    let restored = RunState::from_json_str(&serialized)
        .expect("run state should deserialize with the omit policy");

    let result = Runner::new()
        .with_model_provider(provider)
        .resume(&restored)
        .await
        .expect("resume should succeed");

    assert_eq!(result.final_output.as_deref(), Some("resumed"));
    assert_eq!(result.reasoning_item_id_policy, ReasoningItemIdPolicy::Omit);
    assert_eq!(
        model
            .seen_inputs
            .lock()
            .expect("resume capture inputs lock")
            .as_slice(),
        &[vec![InputItem::from("normalized-start")]]
    );
    assert_eq!(
        result.to_input_list_mode(agents_core::ToInputListMode::Normalized),
        vec![
            InputItem::from("normalized-start"),
            InputItem::from("resumed")
        ]
    );
}
