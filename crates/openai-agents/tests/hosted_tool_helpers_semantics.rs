use openai_agents::{
    code_interpreter_tool, file_search_tool, image_generation_tool, tool_search_tool,
    web_search_tool,
};

#[test]
fn facade_hosted_tool_helpers_are_constructible() {
    let tool_names = vec![
        code_interpreter_tool().definition.name,
        file_search_tool().definition.name,
        image_generation_tool().definition.name,
        tool_search_tool().definition.name,
        web_search_tool().definition.name,
    ];

    assert_eq!(
        tool_names,
        vec![
            "code_interpreter",
            "file_search",
            "image_generation",
            "tool_search",
            "web_search",
        ]
    );
}
