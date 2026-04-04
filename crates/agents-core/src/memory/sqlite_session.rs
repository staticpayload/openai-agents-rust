use std::fmt;
use std::path::Path;

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use crate::errors::{AgentsError, Result};
use crate::items::InputItem;
use crate::memory::session::{
    OpenAIResponsesCompactionArgs, OpenAIResponsesCompactionAwareSession, Session,
};
use crate::memory::session_settings::{SessionSettings, resolve_session_limit};
use crate::memory::util::validate_sql_identifier;

/// SQLite-backed implementation of session storage.
#[derive(Clone)]
pub struct SQLiteSession {
    session_id: String,
    session_settings: Option<SessionSettings>,
    pool: SqlitePool,
    sessions_table: String,
    messages_table: String,
}

impl fmt::Debug for SQLiteSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SQLiteSession")
            .field("session_id", &self.session_id)
            .field("session_settings", &self.session_settings)
            .field("sessions_table", &self.sessions_table)
            .field("messages_table", &self.messages_table)
            .finish()
    }
}

impl SQLiteSession {
    pub async fn open(session_id: impl Into<String>, db_path: impl AsRef<Path>) -> Result<Self> {
        let db_url = format!("sqlite://{}", db_path.as_ref().display());
        Self::open_with_url(session_id, &db_url).await
    }

    pub async fn open_in_memory(session_id: impl Into<String>) -> Result<Self> {
        Self::open_with_url(session_id, "sqlite::memory:").await
    }

    pub async fn open_with_url(session_id: impl Into<String>, database_url: &str) -> Result<Self> {
        Self::open_with_options(
            session_id,
            database_url,
            "agent_sessions",
            "agent_messages",
            Some(SessionSettings::default()),
        )
        .await
    }

    pub async fn open_with_options(
        session_id: impl Into<String>,
        database_url: &str,
        sessions_table: impl Into<String>,
        messages_table: impl Into<String>,
        session_settings: Option<SessionSettings>,
    ) -> Result<Self> {
        let sessions_table = sessions_table.into();
        let messages_table = messages_table.into();
        validate_sql_identifier(&sessions_table)?;
        validate_sql_identifier(&messages_table)?;

        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|error| AgentsError::message(error.to_string()))?;
        let session = Self {
            session_id: session_id.into(),
            session_settings,
            pool,
            sessions_table,
            messages_table,
        };
        session.init_schema().await?;
        Ok(session)
    }

    async fn init_schema(&self) -> Result<()> {
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
                session_id TEXT PRIMARY KEY,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            self.sessions_table
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                message_data TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES {} (session_id) ON DELETE CASCADE
            )",
            self.messages_table, self.sessions_table
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        sqlx::query(&format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_session_id ON {} (session_id, id)",
            self.messages_table, self.messages_table
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Session for SQLiteSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn session_settings(&self) -> Option<&SessionSettings> {
        self.session_settings.as_ref()
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>> {
        let resolved_limit = resolve_session_limit(limit, self.session_settings());
        let rows = if let Some(limit) = resolved_limit {
            sqlx::query(&format!(
                "SELECT message_data FROM {} WHERE session_id = ? ORDER BY id DESC LIMIT ?",
                self.messages_table
            ))
            .bind(&self.session_id)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AgentsError::message(error.to_string()))?
        } else {
            sqlx::query(&format!(
                "SELECT message_data FROM {} WHERE session_id = ? ORDER BY id ASC",
                self.messages_table
            ))
            .bind(&self.session_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AgentsError::message(error.to_string()))?
        };

        let mut items = rows
            .into_iter()
            .filter_map(|row| row.try_get::<String, _>("message_data").ok())
            .filter_map(|value| serde_json::from_str::<InputItem>(&value).ok())
            .collect::<Vec<_>>();
        if resolved_limit.is_some() {
            items.reverse();
        }
        Ok(items)
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        sqlx::query(&format!(
            "INSERT OR IGNORE INTO {} (session_id) VALUES (?)",
            self.sessions_table
        ))
        .bind(&self.session_id)
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        for item in items {
            let payload = serde_json::to_string(&item)
                .map_err(|error| AgentsError::message(error.to_string()))?;
            sqlx::query(&format!(
                "INSERT INTO {} (session_id, message_data) VALUES (?, ?)",
                self.messages_table
            ))
            .bind(&self.session_id)
            .bind(payload)
            .execute(&self.pool)
            .await
            .map_err(|error| AgentsError::message(error.to_string()))?;
        }

        sqlx::query(&format!(
            "UPDATE {} SET updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            self.sessions_table
        ))
        .bind(&self.session_id)
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        Ok(())
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        let row = sqlx::query(&format!(
            "SELECT id, message_data FROM {} WHERE session_id = ? ORDER BY id DESC LIMIT 1",
            self.messages_table
        ))
        .bind(&self.session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let id = row
            .try_get::<i64, _>("id")
            .map_err(|error| AgentsError::message(error.to_string()))?;
        let message_data = row
            .try_get::<String, _>("message_data")
            .map_err(|error| AgentsError::message(error.to_string()))?;

        sqlx::query(&format!("DELETE FROM {} WHERE id = ?", self.messages_table))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|error| AgentsError::message(error.to_string()))?;

        let item = serde_json::from_str::<InputItem>(&message_data)
            .map_err(|error| AgentsError::message(error.to_string()))?;
        Ok(Some(item))
    }

    async fn clear_session(&self) -> Result<()> {
        sqlx::query(&format!(
            "DELETE FROM {} WHERE session_id = ?",
            self.messages_table
        ))
        .bind(&self.session_id)
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        sqlx::query(&format!(
            "DELETE FROM {} WHERE session_id = ?",
            self.sessions_table
        ))
        .bind(&self.session_id)
        .execute(&self.pool)
        .await
        .map_err(|error| AgentsError::message(error.to_string()))?;

        Ok(())
    }

    fn compaction_session(&self) -> Option<&dyn OpenAIResponsesCompactionAwareSession> {
        Some(self)
    }
}

#[async_trait]
impl OpenAIResponsesCompactionAwareSession for SQLiteSession {
    async fn run_compaction(&self, _args: Option<OpenAIResponsesCompactionArgs>) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_session_round_trips_items_and_limit() {
        let session = SQLiteSession::open_in_memory("session")
            .await
            .expect("sqlite session should open");
        session
            .add_items(vec![
                InputItem::from("hello"),
                InputItem::from("world"),
                InputItem::from("again"),
            ])
            .await
            .expect("items should save");

        let items = session
            .get_items_with_limit(Some(2))
            .await
            .expect("items should load");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_text(), Some("world"));
        assert_eq!(items[1].as_text(), Some("again"));
    }

    #[tokio::test]
    async fn sqlite_session_supports_pop_and_clear() {
        let session = SQLiteSession::open_in_memory("session")
            .await
            .expect("sqlite session should open");
        session
            .add_items(vec![InputItem::from("one"), InputItem::from("two")])
            .await
            .expect("items should save");

        let popped = session.pop_item().await.expect("item should pop");
        assert_eq!(
            popped.and_then(|item| item.as_text().map(ToOwned::to_owned)),
            Some("two".to_owned())
        );

        session.clear_session().await.expect("session should clear");
        let items = session.get_items().await.expect("items should load");
        assert!(items.is_empty());
    }
}
