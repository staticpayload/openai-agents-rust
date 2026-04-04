use std::marker::PhantomData;

use schemars::{JsonSchema, schema_for};
use serde_json::Value;

use crate::exceptions::UserError;
use crate::strict_schema::ensure_strict_json_schema;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DocstringStyle {
    #[default]
    Google,
    Numpy,
    Sphinx,
}

#[derive(Clone, Debug)]
pub struct FunctionSchema<TArgs> {
    pub name: String,
    pub description: Option<String>,
    pub params_json_schema: Value,
    pub strict_json_schema: bool,
    _marker: PhantomData<TArgs>,
}

impl<TArgs> FunctionSchema<TArgs>
where
    TArgs: JsonSchema,
{
    pub fn from_type(
        name: impl Into<String>,
        description: Option<String>,
        strict_json_schema: bool,
    ) -> Result<Self, UserError> {
        let schema = serde_json::to_value(schema_for!(TArgs)).map_err(|error| UserError {
            message: error.to_string(),
        })?;
        let params_json_schema = if strict_json_schema {
            ensure_strict_json_schema(schema)?
        } else {
            schema
        };

        Ok(Self {
            name: name.into(),
            description,
            params_json_schema,
            strict_json_schema,
            _marker: PhantomData,
        })
    }
}
