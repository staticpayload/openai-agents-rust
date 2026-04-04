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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
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
        if !override_settings.metadata.is_empty() {
            resolved.metadata.extend(override_settings.metadata.clone());
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
            metadata: BTreeMap::from([("tier".to_owned(), json!("base"))]),
        };
        let override_settings = ModelSettings {
            temperature: Some(0.8),
            top_p: None,
            max_output_tokens: Some(256),
            metadata: BTreeMap::from([("route".to_owned(), json!("fast"))]),
        };

        let resolved = base.resolve(Some(&override_settings));

        assert_eq!(resolved.temperature, Some(0.8));
        assert_eq!(resolved.top_p, Some(0.9));
        assert_eq!(resolved.max_output_tokens, Some(256));
        assert_eq!(resolved.metadata.get("tier"), Some(&json!("base")));
        assert_eq!(resolved.metadata.get("route"), Some(&json!("fast")));
    }
}
