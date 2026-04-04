use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Session configuration settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSettings {
    pub limit: Option<usize>,
}

impl SessionSettings {
    pub fn resolve(&self, override_settings: Option<&Self>) -> Self {
        let Some(override_settings) = override_settings else {
            return self.clone();
        };

        Self {
            limit: override_settings.limit.or(self.limit),
        }
    }
}

pub fn resolve_session_limit(
    explicit_limit: Option<usize>,
    settings: Option<&SessionSettings>,
) -> Option<usize> {
    explicit_limit.or_else(|| settings.and_then(|settings| settings.limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_session_limit_with_override_precedence() {
        let settings = SessionSettings { limit: Some(4) };

        assert_eq!(resolve_session_limit(Some(2), Some(&settings)), Some(2));
        assert_eq!(resolve_session_limit(None, Some(&settings)), Some(4));
        assert_eq!(resolve_session_limit(None, None), None);
    }
}
