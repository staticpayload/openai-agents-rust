use std::marker::PhantomData;

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::exceptions::{ModelBehaviorError, UserError};
use crate::strict_schema::ensure_strict_json_schema;

pub trait AgentOutputSchemaBase: Send + Sync {
    fn is_plain_text(&self) -> bool;
    fn name(&self) -> &str;
    fn json_schema(&self) -> std::result::Result<Value, UserError>;
    fn is_strict_json_schema(&self) -> bool;
    fn validate_json(&self, json: &str) -> std::result::Result<Value, ModelBehaviorError>;
}

#[derive(Clone, Debug)]
pub struct AgentOutputSchema<T> {
    name: String,
    strict_json_schema: bool,
    _marker: PhantomData<T>,
}

impl<T> AgentOutputSchema<T>
where
    T: JsonSchema + DeserializeOwned + Serialize + Send + Sync + 'static,
{
    pub fn new(strict_json_schema: bool) -> Self {
        Self {
            name: std::any::type_name::<T>().to_owned(),
            strict_json_schema,
            _marker: PhantomData,
        }
    }
}

impl<T> AgentOutputSchemaBase for AgentOutputSchema<T>
where
    T: JsonSchema + DeserializeOwned + Serialize + Send + Sync + 'static,
{
    fn is_plain_text(&self) -> bool {
        self.name == "alloc::string::String" || self.name == "std::string::String"
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn json_schema(&self) -> std::result::Result<Value, UserError> {
        if self.is_plain_text() {
            return Err(UserError {
                message: "plain text output has no JSON schema".to_owned(),
            });
        }
        let schema = serde_json::to_value(schema_for!(T)).map_err(|error| UserError {
            message: error.to_string(),
        })?;
        if self.strict_json_schema {
            ensure_strict_json_schema(schema)
        } else {
            Ok(schema)
        }
    }

    fn is_strict_json_schema(&self) -> bool {
        self.strict_json_schema
    }

    fn validate_json(&self, json: &str) -> std::result::Result<Value, ModelBehaviorError> {
        let parsed: T = serde_json::from_str(json).map_err(|error| ModelBehaviorError {
            message: error.to_string(),
        })?;
        serde_json::to_value(parsed).map_err(|error| ModelBehaviorError {
            message: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct ExampleOutput {
        answer: String,
    }

    #[test]
    fn validates_structured_output() {
        let schema = AgentOutputSchema::<ExampleOutput>::new(true);
        let parsed = schema
            .validate_json(r#"{"answer":"ok"}"#)
            .expect("output should parse");
        assert_eq!(parsed["answer"], Value::String("ok".to_owned()));
    }
}
