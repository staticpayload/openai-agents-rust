//! Session persistence abstractions and core backends.

pub mod session;
pub mod session_settings;
pub mod sqlite_session;
pub mod util;

pub use session::{
    MemorySession, OpenAIResponsesCompactionArgs, OpenAIResponsesCompactionAwareSession, Session,
    is_openai_responses_compaction_aware_session,
};
pub use session_settings::{SessionSettings, resolve_session_limit};
pub use sqlite_session::SQLiteSession;
