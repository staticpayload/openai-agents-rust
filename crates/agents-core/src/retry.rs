use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelRetryBackoffSettings {
    pub initial_delay: Option<f64>,
    pub max_delay: Option<f64>,
    pub multiplier: Option<f64>,
    pub jitter: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelRetryNormalizedError {
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub request_id: Option<String>,
    pub retry_after: Option<f64>,
    pub is_abort: bool,
    pub is_network_error: bool,
    pub is_timeout: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelRetryAdvice {
    pub suggested: Option<bool>,
    pub retry_after: Option<f64>,
    pub replay_safety: Option<String>,
    pub reason: Option<String>,
    pub normalized: Option<ModelRetryNormalizedError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelRetryAdviceRequest {
    pub attempt: usize,
    pub stream: bool,
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RetryDecision {
    pub retry: bool,
    pub delay: Option<f64>,
    pub reason: Option<String>,
}

impl RetryDecision {
    pub fn retry(delay: Option<f64>, reason: impl Into<String>) -> Self {
        Self {
            retry: true,
            delay,
            reason: Some(reason.into()),
        }
    }

    pub fn stop(reason: impl Into<String>) -> Self {
        Self {
            retry: false,
            delay: None,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RetryPolicyContext {
    pub attempt: usize,
    pub max_retries: usize,
    pub stream: bool,
    pub normalized: ModelRetryNormalizedError,
    pub provider_advice: Option<ModelRetryAdvice>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelRetrySettings {
    pub max_retries: Option<usize>,
    pub backoff: Option<ModelRetryBackoffSettings>,
}
