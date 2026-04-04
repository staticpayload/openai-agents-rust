use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::exceptions::UserError;
use crate::tool::ToolDefinition;
use crate::tool_context::ToolCall;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FunctionToolLookupKey {
    Bare { name: String },
    Namespaced { namespace: String, name: String },
    DeferredTopLevel { name: String },
}

pub fn is_reserved_synthetic_tool_namespace(
    name: impl AsRef<str>,
    namespace: Option<&str>,
) -> bool {
    let name = name.as_ref().trim();
    let Some(namespace) = namespace.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };

    !name.is_empty() && namespace == name
}

pub fn tool_qualified_name(name: impl AsRef<str>, namespace: Option<&str>) -> Option<String> {
    let name = name.as_ref().trim();
    if name.is_empty() {
        return None;
    }

    match namespace.map(str::trim).filter(|value| !value.is_empty()) {
        Some(namespace) => Some(format!("{namespace}.{name}")),
        None => Some(name.to_owned()),
    }
}

pub fn tool_trace_name(name: impl AsRef<str>, namespace: Option<&str>) -> Option<String> {
    let name = name.as_ref().trim();
    if name.is_empty() {
        return None;
    }
    if is_reserved_synthetic_tool_namespace(name, namespace) {
        return Some(name.to_owned());
    }
    tool_qualified_name(name, namespace)
}

pub fn get_tool_call_namespace(tool_call: &ToolCall) -> Option<&str> {
    tool_call
        .namespace
        .as_deref()
        .filter(|value| !value.is_empty())
}

pub fn get_tool_call_name(tool_call: &ToolCall) -> Option<&str> {
    if tool_call.name.trim().is_empty() {
        None
    } else {
        Some(tool_call.name.as_str())
    }
}

pub fn get_tool_call_qualified_name(tool_call: &ToolCall) -> Option<String> {
    tool_qualified_name(
        get_tool_call_name(tool_call)?,
        get_tool_call_namespace(tool_call),
    )
}

pub fn get_tool_call_trace_name(tool_call: &ToolCall) -> Option<String> {
    tool_trace_name(
        get_tool_call_name(tool_call)?,
        get_tool_call_namespace(tool_call),
    )
}

pub fn get_function_tool_lookup_key(
    name: impl AsRef<str>,
    namespace: Option<&str>,
) -> Option<FunctionToolLookupKey> {
    let name = name.as_ref().trim();
    if name.is_empty() {
        return None;
    }
    if is_reserved_synthetic_tool_namespace(name, namespace) {
        return Some(FunctionToolLookupKey::DeferredTopLevel {
            name: name.to_owned(),
        });
    }
    match namespace.map(str::trim).filter(|value| !value.is_empty()) {
        Some(namespace) => Some(FunctionToolLookupKey::Namespaced {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        }),
        None => Some(FunctionToolLookupKey::Bare {
            name: name.to_owned(),
        }),
    }
}

pub fn get_function_tool_lookup_key_for_call(
    tool_call: &ToolCall,
) -> Option<FunctionToolLookupKey> {
    get_function_tool_lookup_key(
        get_tool_call_name(tool_call)?,
        get_tool_call_namespace(tool_call),
    )
}

pub fn get_function_tool_lookup_key_for_definition(
    definition: &ToolDefinition,
) -> Option<FunctionToolLookupKey> {
    if definition.defer_loading && definition.namespace.is_none() {
        return Some(FunctionToolLookupKey::DeferredTopLevel {
            name: definition.name.clone(),
        });
    }
    get_function_tool_lookup_key(&definition.name, definition.namespace.as_deref())
}

pub fn get_function_tool_lookup_keys(definition: &ToolDefinition) -> Vec<FunctionToolLookupKey> {
    let mut keys = Vec::new();
    if let Some(key) =
        get_function_tool_lookup_key(&definition.name, definition.namespace.as_deref())
    {
        if !matches!(key, FunctionToolLookupKey::DeferredTopLevel { .. }) {
            keys.push(key);
        }
    }

    if definition.defer_loading && definition.namespace.is_none() {
        keys.push(FunctionToolLookupKey::DeferredTopLevel {
            name: definition.name.clone(),
        });
    }

    keys
}

pub fn get_function_tool_qualified_name(definition: &ToolDefinition) -> Option<String> {
    tool_qualified_name(&definition.name, definition.namespace.as_deref())
}

pub fn get_function_tool_trace_name(definition: &ToolDefinition) -> Option<String> {
    tool_trace_name(&definition.name, definition.namespace.as_deref())
}

pub fn get_function_tool_approval_keys(
    name: impl AsRef<str>,
    namespace: Option<&str>,
    allow_bare_name_alias: bool,
    lookup_key: Option<&FunctionToolLookupKey>,
) -> Vec<String> {
    let name = name.as_ref().trim();
    if name.is_empty() {
        return Vec::new();
    }

    let mut approval_keys = Vec::new();
    if allow_bare_name_alias {
        approval_keys.push(name.to_owned());
    }

    let resolved_lookup_key = lookup_key
        .cloned()
        .or_else(|| get_function_tool_lookup_key(name, namespace));

    if let Some(key) = resolved_lookup_key {
        let key_value = match key {
            FunctionToolLookupKey::Bare { name } => name,
            FunctionToolLookupKey::Namespaced { namespace, name } => {
                tool_qualified_name(&name, Some(namespace.as_str())).unwrap_or(name)
            }
            FunctionToolLookupKey::DeferredTopLevel { name } => {
                format!("deferred_top_level:{name}")
            }
        };
        if !approval_keys.contains(&key_value) {
            approval_keys.push(key_value);
        }
    }

    if approval_keys.is_empty() {
        approval_keys.push(name.to_owned());
    }

    approval_keys
}

pub fn validate_function_tool_namespace_shape(
    name: impl AsRef<str>,
    namespace: Option<&str>,
) -> std::result::Result<(), UserError> {
    let name = name.as_ref().trim();
    if !is_reserved_synthetic_tool_namespace(name, namespace) {
        return Ok(());
    }

    let reserved_key =
        tool_qualified_name(name, namespace).unwrap_or_else(|| "unknown_tool".to_owned());
    Err(UserError {
        message: format!(
            "responses tool-search reserves the synthetic namespace `{reserved_key}` for deferred top-level function tools"
        ),
    })
}

pub fn build_function_tool_lookup_map<'a, I>(
    tools: I,
) -> std::result::Result<HashMap<FunctionToolLookupKey, &'a ToolDefinition>, UserError>
where
    I: IntoIterator<Item = &'a ToolDefinition>,
{
    let mut tool_map = HashMap::new();
    for definition in tools {
        validate_function_tool_namespace_shape(&definition.name, definition.namespace.as_deref())?;
        for key in get_function_tool_lookup_keys(definition) {
            tool_map.insert(key, definition);
        }
    }
    Ok(tool_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_namespaced_and_deferred_lookup_keys() {
        assert_eq!(
            get_function_tool_lookup_key("search", Some("knowledge")),
            Some(FunctionToolLookupKey::Namespaced {
                namespace: "knowledge".to_owned(),
                name: "search".to_owned(),
            })
        );
        assert_eq!(
            get_function_tool_lookup_key("search", Some("search")),
            Some(FunctionToolLookupKey::DeferredTopLevel {
                name: "search".to_owned(),
            })
        );
    }

    #[test]
    fn builds_lookup_map_for_deferred_tools() {
        let tool = ToolDefinition::new("search", "Search").with_defer_loading(true);
        let map = build_function_tool_lookup_map([&tool]).expect("lookup map should build");

        assert!(map.contains_key(&FunctionToolLookupKey::DeferredTopLevel {
            name: "search".to_owned(),
        }));
    }

    #[test]
    fn computes_approval_keys() {
        let keys = get_function_tool_approval_keys(
            "search",
            Some("knowledge"),
            true,
            Some(&FunctionToolLookupKey::Namespaced {
                namespace: "knowledge".to_owned(),
                name: "search".to_owned(),
            }),
        );

        assert_eq!(
            keys,
            vec!["search".to_owned(), "knowledge.search".to_owned()]
        );
    }
}
