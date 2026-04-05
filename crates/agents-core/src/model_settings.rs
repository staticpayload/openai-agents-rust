use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Provider-agnostic model tuning parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelSettings {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub tool_choice: Option<String>,
    pub parallel_tool_calls: Option<bool>,
    pub truncation: Option<String>,
    pub store: Option<bool>,
    pub include_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub response_include: Vec<String>,
    pub top_logprobs: Option<u32>,
    pub reasoning: Option<ReasoningSettings>,
    pub verbosity: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_query: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_body: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_headers: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_args: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningSettings {
    pub effort: Option<String>,
    pub summary: Option<String>,
}

impl ModelSettings {
    pub fn resolve(&self, override_settings: Option<&Self>) -> Self {
        let Some(override_settings) = override_settings else {
            return self.clone();
        };

        let mut resolved = self.clone();
        if override_settings.temperature.is_some() {
            resolved.temperature = override_settings.temperature;
        }
        if override_settings.top_p.is_some() {
            resolved.top_p = override_settings.top_p;
        }
        if override_settings.max_output_tokens.is_some() {
            resolved.max_output_tokens = override_settings.max_output_tokens;
        }
        if override_settings.frequency_penalty.is_some() {
            resolved.frequency_penalty = override_settings.frequency_penalty;
        }
        if override_settings.presence_penalty.is_some() {
            resolved.presence_penalty = override_settings.presence_penalty;
        }
        if override_settings.tool_choice.is_some() {
            resolved.tool_choice = override_settings.tool_choice.clone();
        }
        if override_settings.parallel_tool_calls.is_some() {
            resolved.parallel_tool_calls = override_settings.parallel_tool_calls;
        }
        if override_settings.truncation.is_some() {
            resolved.truncation = override_settings.truncation.clone();
        }
        if override_settings.store.is_some() {
            resolved.store = override_settings.store;
        }
        if override_settings.include_usage.is_some() {
            resolved.include_usage = override_settings.include_usage;
        }
        if !override_settings.response_include.is_empty() {
            resolved.response_include = override_settings.response_include.clone();
        }
        if override_settings.top_logprobs.is_some() {
            resolved.top_logprobs = override_settings.top_logprobs;
        }
        if override_settings.reasoning.is_some() {
            resolved.reasoning = override_settings.reasoning.clone();
        }
        if override_settings.verbosity.is_some() {
            resolved.verbosity = override_settings.verbosity.clone();
        }
        if !override_settings.metadata.is_empty() {
            resolved.metadata.extend(override_settings.metadata.clone());
        }
        if !override_settings.extra_query.is_empty() {
            resolved
                .extra_query
                .extend(override_settings.extra_query.clone());
        }
        if !override_settings.extra_body.is_empty() {
            resolved
                .extra_body
                .extend(override_settings.extra_body.clone());
        }
        if !override_settings.extra_headers.is_empty() {
            resolved
                .extra_headers
                .extend(override_settings.extra_headers.clone());
        }
        if !override_settings.extra_args.is_empty() {
            resolved
                .extra_args
                .extend(override_settings.extra_args.clone());
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn overlays_non_empty_model_settings() {
        let base = ModelSettings {
            temperature: Some(0.2),
            top_p: Some(0.9),
            max_output_tokens: Some(128),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.3),
            tool_choice: Some("auto".to_owned()),
            parallel_tool_calls: Some(false),
            truncation: Some("auto".to_owned()),
            store: Some(false),
            include_usage: Some(false),
            response_include: vec!["reasoning".to_owned()],
            top_logprobs: Some(3),
            reasoning: Some(ReasoningSettings {
                effort: Some("low".to_owned()),
                summary: None,
            }),
            verbosity: Some("medium".to_owned()),
            metadata: BTreeMap::from([("tier".to_owned(), json!("base"))]),
            extra_query: BTreeMap::from([("route".to_owned(), json!("slow"))]),
            extra_body: BTreeMap::from([("store".to_owned(), json!(false))]),
            extra_headers: BTreeMap::from([("x-base".to_owned(), json!("1"))]),
            extra_args: BTreeMap::from([("timeout".to_owned(), json!(10))]),
        };
        let override_settings = ModelSettings {
            temperature: Some(0.8),
            top_p: None,
            max_output_tokens: Some(256),
            frequency_penalty: None,
            presence_penalty: Some(0.6),
            tool_choice: Some("required".to_owned()),
            parallel_tool_calls: Some(true),
            truncation: Some("disabled".to_owned()),
            store: Some(true),
            include_usage: Some(true),
            response_include: vec!["file_search_call.results".to_owned()],
            top_logprobs: Some(5),
            reasoning: Some(ReasoningSettings {
                effort: Some("medium".to_owned()),
                summary: Some("auto".to_owned()),
            }),
            verbosity: Some("low".to_owned()),
            metadata: BTreeMap::from([("route".to_owned(), json!("fast"))]),
            extra_query: BTreeMap::from([("region".to_owned(), json!("us"))]),
            extra_body: BTreeMap::from([("parallel_tool_calls".to_owned(), json!(true))]),
            extra_headers: BTreeMap::from([("x-route".to_owned(), json!("fast"))]),
            extra_args: BTreeMap::from([("retry".to_owned(), json!(2))]),
        };

        let resolved = base.resolve(Some(&override_settings));

        assert_eq!(resolved.temperature, Some(0.8));
        assert_eq!(resolved.top_p, Some(0.9));
        assert_eq!(resolved.max_output_tokens, Some(256));
        assert_eq!(resolved.frequency_penalty, Some(0.1));
        assert_eq!(resolved.presence_penalty, Some(0.6));
        assert_eq!(resolved.tool_choice.as_deref(), Some("required"));
        assert_eq!(resolved.parallel_tool_calls, Some(true));
        assert_eq!(resolved.truncation.as_deref(), Some("disabled"));
        assert_eq!(resolved.store, Some(true));
        assert_eq!(resolved.include_usage, Some(true));
        assert_eq!(
            resolved.response_include,
            vec!["file_search_call.results".to_owned()]
        );
        assert_eq!(resolved.top_logprobs, Some(5));
        assert_eq!(
            resolved
                .reasoning
                .as_ref()
                .and_then(|value| value.effort.as_deref()),
            Some("medium")
        );
        assert_eq!(resolved.verbosity.as_deref(), Some("low"));
        assert_eq!(resolved.metadata.get("tier"), Some(&json!("base")));
        assert_eq!(resolved.metadata.get("route"), Some(&json!("fast")));
        assert_eq!(resolved.extra_query.get("route"), Some(&json!("slow")));
        assert_eq!(resolved.extra_query.get("region"), Some(&json!("us")));
        assert_eq!(resolved.extra_body.get("store"), Some(&json!(false)));
        assert_eq!(
            resolved.extra_body.get("parallel_tool_calls"),
            Some(&json!(true))
        );
        assert_eq!(resolved.extra_headers.get("x-base"), Some(&json!("1")));
        assert_eq!(resolved.extra_headers.get("x-route"), Some(&json!("fast")));
        assert_eq!(resolved.extra_args.get("timeout"), Some(&json!(10)));
        assert_eq!(resolved.extra_args.get("retry"), Some(&json!(2)));
    }
}
