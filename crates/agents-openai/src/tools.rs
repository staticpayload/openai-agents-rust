use agents_core::StaticTool;

pub fn web_search_tool() -> StaticTool {
    StaticTool::new(
        "web_search",
        "Search the public web via OpenAI hosted search.",
    )
}

pub fn file_search_tool() -> StaticTool {
    StaticTool::new(
        "file_search",
        "Search indexed files through the OpenAI file search tool.",
    )
}

pub fn code_interpreter_tool() -> StaticTool {
    StaticTool::new(
        "code_interpreter",
        "Run short code snippets in the hosted OpenAI code interpreter.",
    )
}

pub fn tool_search_tool() -> StaticTool {
    StaticTool::new(
        "tool_search",
        "Search tools available to the OpenAI runtime.",
    )
}

pub fn image_generation_tool() -> StaticTool {
    StaticTool::new(
        "image_generation",
        "Generate or edit images with OpenAI hosted tooling.",
    )
}
