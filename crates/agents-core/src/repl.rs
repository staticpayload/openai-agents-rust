use std::io::{self, Write};

use futures::StreamExt;

use crate::agent::Agent;
use crate::errors::Result;
use crate::items::InputItem;
use crate::result::RunResultStreaming;
use crate::run::Runner;
use crate::run_config::DEFAULT_MAX_TURNS;
use crate::stream_events::StreamEvent;

pub async fn run_demo_loop(agent: &Agent, stream: bool, max_turns: usize) -> Result<()> {
    let mut current_agent = agent.clone();
    let mut input_items: Vec<InputItem> = Vec::new();

    loop {
        print!(" > ");
        io::stdout().flush().ok();
        let mut user_input = String::new();
        if io::stdin().read_line(&mut user_input).is_err() {
            break;
        }
        let user_input = user_input.trim_end().to_owned();
        if matches!(user_input.to_lowercase().as_str(), "exit" | "quit") {
            break;
        }
        if user_input.is_empty() {
            continue;
        }

        input_items.push(InputItem::Json {
            value: serde_json::json!({
                "role": "user",
                "content": user_input,
            }),
        });

        if stream {
            let result = Runner::new()
                .with_config(crate::run_config::RunConfig {
                    max_turns,
                    ..crate::run_config::RunConfig::default()
                })
                .run_items_streamed(&current_agent, input_items.clone())
                .await?;
            consume_streamed_result(&result).await;
            let final_result = result.wait_for_completion().await?;
            current_agent = final_result
                .last_agent
                .clone()
                .unwrap_or_else(|| current_agent.clone());
            input_items = final_result.to_input_list();
        } else {
            let result = Runner::new()
                .with_config(crate::run_config::RunConfig {
                    max_turns,
                    ..crate::run_config::RunConfig::default()
                })
                .run_items(&current_agent, input_items.clone())
                .await?;
            if let Some(final_output) = result.final_output.as_deref() {
                println!("{final_output}");
            }
            current_agent = result
                .last_agent
                .clone()
                .unwrap_or_else(|| current_agent.clone());
            input_items = result.to_input_list();
        }
    }

    Ok(())
}

async fn consume_streamed_result(result: &RunResultStreaming) {
    let mut stream = Box::pin(result.stream_events());
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::RunItemEvent(event) => match event.item {
                crate::items::RunItem::ToolCall { .. } => println!("[tool called]"),
                crate::items::RunItem::ToolCallOutput { output, .. } => {
                    println!("[tool output: {:?}]", output)
                }
                _ => {}
            },
            StreamEvent::AgentUpdated(event) => {
                println!("[Agent updated: {}]", event.new_agent.name)
            }
            StreamEvent::RawResponseEvent(_) | StreamEvent::Lifecycle(_) => {}
        }
    }

    if let Ok(final_result) = result.wait_for_completion().await {
        if let Some(final_output) = &final_result.final_output {
            println!("{final_output}");
        }
    }
}

#[allow(dead_code)]
const _: usize = DEFAULT_MAX_TURNS;
