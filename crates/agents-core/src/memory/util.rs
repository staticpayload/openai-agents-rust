use crate::errors::{AgentsError, Result};
use crate::items::InputItem;

pub fn apply_session_limit(items: &[InputItem], limit: Option<usize>) -> Vec<InputItem> {
    match limit {
        Some(limit) if items.len() > limit => items[items.len() - limit..].to_vec(),
        _ => items.to_vec(),
    }
}

pub fn validate_sql_identifier(identifier: &str) -> Result<()> {
    let valid = !identifier.is_empty()
        && identifier
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if valid {
        Ok(())
    } else {
        Err(AgentsError::message(format!(
            "invalid SQLite identifier `{identifier}`"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_latest_item_limit_in_chronological_order() {
        let items = vec![
            InputItem::from("a"),
            InputItem::from("b"),
            InputItem::from("c"),
        ];

        let limited = apply_session_limit(&items, Some(2));

        assert_eq!(limited[0].as_text(), Some("b"));
        assert_eq!(limited[1].as_text(), Some("c"));
    }

    #[test]
    fn rejects_invalid_sql_identifier() {
        let error = validate_sql_identifier("bad-name").expect_err("identifier should fail");

        assert!(error.to_string().contains("invalid SQLite identifier"));
    }
}
