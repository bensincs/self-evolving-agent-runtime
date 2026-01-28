// crates/host/src/agents/coder/mod.rs

//! Coder agent that implements capability code.

mod prompts;
mod tool_defs;
mod tool_handler;

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest};

use super::log::{self, Agent as LogAgent};

/// Run the coder agent loop.
pub fn run_coder_agent<C: AiClient + Sync>(
    client: &C,
    capabilities_root: &str,
    new_id: &str,
    cap_path: &Path,
    main_rs: &str,
    task: &str,
    max_steps: usize,
) -> Result<()> {
    let system_prompt =
        prompts::build_coder_prompt(capabilities_root, new_id, cap_path, main_rs, task);
    let tools = tool_defs::coder_tool_definitions();

    let mut messages = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": "Read tests first, then write src/lib.rs and src/main.rs to make them compile and pass. Reply DONE when all tests pass and WASM build succeeds."}),
    ];

    let handler = tool_handler::CoderToolHandler::new(capabilities_root, new_id, cap_path);

    for step in 0..max_steps {
        log::agent_step(LogAgent::Coder, step + 1);

        let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
        let response = client.chat(request)?;
        let choice = response.choices.into_iter().next().context("no choices")?;
        let msg = choice.message;

        if let Some(tool_calls) = msg.tool_calls.clone() {
            if let Some(ref content) = msg.content {
                if !content.trim().is_empty() {
                    log::agent_message(LogAgent::Coder, content);
                }
            }

            messages.push(json!({
                "role": "assistant",
                "content": msg.content.clone(),
                "tool_calls": tool_calls.iter().map(|tc| json!({
                    "id": tc.id,
                    "type": tc.call_type,
                    "function": {"name": tc.function.name, "arguments": tc.function.arguments}
                })).collect::<Vec<_>>()
            }));

            for tc in tool_calls {
                log::tool_call(LogAgent::Coder, &tc.function.name, &tc.function.arguments);

                let result = handler.handle(&tc)?;
                if result.starts_with("ERROR") {
                    log::tool_error(LogAgent::Coder, &result);
                } else {
                    log::tool_success(LogAgent::Coder, &result);
                }
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "name": tc.function.name,
                    "content": result
                }));
            }
            continue;
        }

        if let Some(content) = msg.content.clone() {
            log::agent_message(LogAgent::Coder, &content);
            if content.to_uppercase().contains("DONE") {
                log::agent_done(LogAgent::Coder);
                return Ok(());
            }
        }
        messages.push(json!({"role": "assistant", "content": msg.content.unwrap_or_default()}));
        messages.push(json!({"role": "user", "content": "Continue. Reply DONE when finished."}));
    }

    log::error("Coder agent reached max steps without DONE");
    anyhow::bail!("Coder agent reached max steps without DONE")
}
