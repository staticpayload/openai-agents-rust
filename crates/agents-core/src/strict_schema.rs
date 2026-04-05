use serde_json::{Map, Value};

use crate::exceptions::UserError;

fn empty_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
    })
}

/// Ensure an input JSON schema conforms to the strict subset expected by structured outputs.
pub fn ensure_strict_json_schema(schema: Value) -> std::result::Result<Value, UserError> {
    if schema == Value::Object(Map::new()) {
        return Ok(empty_schema());
    }

    let mut schema = schema;
    let root = schema.clone();
    ensure_node(&mut schema, &root)?;
    Ok(schema)
}

fn ensure_node(node: &mut Value, root: &Value) -> std::result::Result<(), UserError> {
    let Some(object) = node.as_object_mut() else {
        return Ok(());
    };

    if let Some(Value::Object(defs)) = object.get_mut("$defs") {
        for value in defs.values_mut() {
            ensure_node(value, root)?;
        }
    }

    if let Some(Value::Object(definitions)) = object.get_mut("definitions") {
        for value in definitions.values_mut() {
            ensure_node(value, root)?;
        }
    }

    if object.get("type").and_then(Value::as_str) == Some("object") {
        match object.get("additionalProperties") {
            None => {
                object.insert("additionalProperties".to_owned(), Value::Bool(false));
            }
            Some(Value::Bool(true)) => {
                return Err(UserError {
                    message: "additionalProperties must be false for strict object schemas"
                        .to_owned(),
                });
            }
            _ => {}
        }
    }

    if object.get("properties").is_some() {
        let property_names = object
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| properties.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        object.insert(
            "required".to_owned(),
            Value::Array(property_names.into_iter().map(Value::String).collect()),
        );
    }

    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        for value in properties.values_mut() {
            ensure_node(value, root)?;
        }
    }

    if let Some(items) = object.get_mut("items") {
        ensure_node(items, root)?;
    }

    if let Some(Value::Array(any_of)) = object.get_mut("anyOf") {
        for value in any_of {
            ensure_node(value, root)?;
        }
    }

    if let Some(Value::Array(one_of)) = object.remove("oneOf") {
        let mut normalized = Vec::new();
        for mut value in one_of {
            ensure_node(&mut value, root)?;
            normalized.push(value);
        }
        object
            .entry("anyOf".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(Value::Array(any_of)) = object.get_mut("anyOf") {
            any_of.extend(normalized);
        }
    }

    if let Some(Value::Array(all_of)) = object.remove("allOf") {
        if all_of.len() == 1 {
            let mut merged = all_of.into_iter().next().unwrap_or(Value::Null);
            ensure_node(&mut merged, root)?;
            if let Some(merged_object) = merged.as_object() {
                for (key, value) in merged_object {
                    object.insert(key.clone(), value.clone());
                }
            }
            ensure_node(node, root)?;
            return Ok(());
        }

        let mut normalized = Vec::new();
        for mut value in all_of {
            ensure_node(&mut value, root)?;
            normalized.push(value);
        }
        object.insert("allOf".to_owned(), Value::Array(normalized));
    }

    if matches!(object.get("default"), Some(Value::Null)) {
        object.remove("default");
    }

    if let Some(Value::String(reference)) = object.get("$ref").cloned() {
        if object.len() > 1 {
            let resolved = resolve_ref(root, &reference).ok_or_else(|| UserError {
                message: format!("could not resolve schema reference `{reference}`"),
            })?;
            if let Some(resolved_object) = resolved.as_object() {
                let mut merged = resolved_object.clone();
                for (key, value) in object.clone() {
                    if key != "$ref" {
                        merged.insert(key, value);
                    }
                }
                *object = merged;
                object.remove("$ref");
                ensure_node(node, root)?;
            }
        }
    }

    Ok(())
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let path = reference.strip_prefix("#/")?;
    let mut current = root;
    for segment in path.split('/') {
        current = current.get(segment)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn makes_object_schema_strict() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let strict = ensure_strict_json_schema(schema).expect("schema should normalize");
        assert_eq!(strict["additionalProperties"], Value::Bool(false));
        assert_eq!(strict["required"], json!(["name"]));
    }

    #[test]
    fn merges_single_all_of_entry_into_parent() {
        let schema = json!({
            "type": "object",
            "allOf": [
                {
                    "properties": {
                        "enabled": { "type": "boolean" }
                    }
                }
            ]
        });

        let strict = ensure_strict_json_schema(schema).expect("schema should normalize");

        assert!(strict.get("allOf").is_none());
        assert_eq!(strict["additionalProperties"], Value::Bool(false));
        assert_eq!(strict["required"], json!(["enabled"]));
        assert_eq!(strict["properties"]["enabled"]["type"], json!("boolean"));
    }

    #[test]
    fn expands_definition_refs_when_ref_has_siblings() {
        let schema = json!({
            "definitions": {
                "refObj": {
                    "type": "string",
                    "default": null
                }
            },
            "type": "object",
            "properties": {
                "value": {
                    "$ref": "#/definitions/refObj",
                    "description": "merged"
                }
            }
        });

        let strict = ensure_strict_json_schema(schema).expect("schema should normalize");

        assert_eq!(strict["properties"]["value"]["type"], json!("string"));
        assert_eq!(
            strict["properties"]["value"]["description"],
            json!("merged")
        );
        assert!(strict["properties"]["value"].get("default").is_none());
    }
}
