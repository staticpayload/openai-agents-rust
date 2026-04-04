use std::env;

pub fn debug_flag_enabled(flag: &str, default: bool) -> bool {
    match env::var(flag) {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "True"),
        Err(_) => default,
    }
}

pub fn load_dont_log_model_data() -> bool {
    debug_flag_enabled("OPENAI_AGENTS_DONT_LOG_MODEL_DATA", true)
}

pub fn load_dont_log_tool_data() -> bool {
    debug_flag_enabled("OPENAI_AGENTS_DONT_LOG_TOOL_DATA", true)
}

pub fn dont_log_model_data() -> bool {
    load_dont_log_model_data()
}

pub fn dont_log_tool_data() -> bool {
    load_dont_log_tool_data()
}
