use std::collections::BTreeMap;
use std::env;
use std::sync::{Mutex, OnceLock};

use std::sync::Arc;

use agents_core::{ModelProvider, ModelRequest, MultiProvider};
use agents_openai::{
    OPENAI_AGENT_HARNESS_ID_ENV_VAR, OPENAI_HARNESS_ID_TRACE_METADATA_KEY,
    OpenAIAgentRegistrationConfig, OpenAIProvider, get_default_openai_agent_registration,
    set_default_openai_agent_registration, set_default_openai_harness,
};
use serde_json::json;

fn harness_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct DefaultHarnessReset(Option<OpenAIAgentRegistrationConfig>);

impl Drop for DefaultHarnessReset {
    fn drop(&mut self) {
        set_default_openai_agent_registration(self.0.clone());
    }
}

struct EnvHarnessReset(Option<String>);

impl Drop for EnvHarnessReset {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => unsafe { env::set_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR, value) },
            None => unsafe { env::remove_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR) },
        }
    }
}

#[test]
fn harness_registration_metadata_follows_override_rules() {
    let _guard = harness_test_lock().lock().expect("test lock");
    let _default_reset = DefaultHarnessReset(get_default_openai_agent_registration());
    let _env_reset = EnvHarnessReset(env::var(OPENAI_AGENT_HARNESS_ID_ENV_VAR).ok());

    unsafe { env::set_var(OPENAI_AGENT_HARNESS_ID_ENV_VAR, "env-harness") };
    set_default_openai_harness(Some("default-harness"));

    let default_provider = OpenAIProvider::new();
    assert_eq!(
        default_provider.effective_harness_id().as_deref(),
        Some("default-harness")
    );
    assert_eq!(
        default_provider
            .resolve_trace_metadata(None, None)
            .and_then(|metadata| metadata.get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY).cloned()),
        Some(json!("default-harness"))
    );
    assert_eq!(
        default_provider
            .prepare_request(ModelRequest::default())
            .settings
            .metadata
            .get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY),
        Some(&json!("default-harness"))
    );

    set_default_openai_agent_registration(None);
    let env_provider = OpenAIProvider::new();
    assert_eq!(
        env_provider.effective_harness_id().as_deref(),
        Some("env-harness")
    );

    let provider = OpenAIProvider::new().with_agent_registration(OpenAIAgentRegistrationConfig {
        harness_id: Some("provider-harness".to_owned()),
    });
    assert_eq!(
        provider.effective_harness_id().as_deref(),
        Some("provider-harness")
    );
    assert_eq!(
        provider
            .resolve_trace_metadata(
                None,
                Some(&BTreeMap::from([
                    (
                        OPENAI_HARNESS_ID_TRACE_METADATA_KEY.to_owned(),
                        json!("explicit-harness"),
                    ),
                    ("source".to_owned(), json!("test")),
                ])),
            )
            .and_then(|metadata| metadata.get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY).cloned()),
        Some(json!("explicit-harness"))
    );
    assert_eq!(
        provider
            .prepare_request(ModelRequest {
                settings: agents_core::ModelSettings {
                    metadata: BTreeMap::from([(
                        OPENAI_HARNESS_ID_TRACE_METADATA_KEY.to_owned(),
                        json!("explicit-harness"),
                    )]),
                    ..Default::default()
                },
                ..Default::default()
            })
            .settings
            .metadata
            .get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY),
        Some(&json!("explicit-harness"))
    );

    let routed = MultiProvider::new(Arc::new(provider));
    assert_eq!(
        routed
            .resolve_trace_metadata(
                Some("openai/gpt-5"),
                Some(&BTreeMap::from([(
                    "source".to_owned(),
                    json!("multiprovider")
                )])),
            )
            .and_then(|metadata| metadata.get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY).cloned()),
        Some(json!("provider-harness"))
    );
    assert_eq!(
        routed
            .prepare_request(ModelRequest {
                model: Some("openai/gpt-5".to_owned()),
                ..Default::default()
            })
            .settings
            .metadata
            .get(OPENAI_HARNESS_ID_TRACE_METADATA_KEY),
        Some(&json!("provider-harness"))
    );
}
