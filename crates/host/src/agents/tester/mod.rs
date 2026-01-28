// crates/host/src/agents/tester/mod.rs

//! Tester agent that writes tests based on the plan.

mod prompts;
mod tool_defs;
mod tool_handler;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest};

use super::log::{self, Agent as LogAgent};

/// Backup the src directory contents.
fn backup_src(cap_path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let src_path = cap_path.join("src");
    let mut backup = HashMap::new();

    if src_path.exists() {
        for entry in fs::read_dir(&src_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let content = fs::read(&path)?;
                backup.insert(name, content);
            }
        }
    }

    Ok(backup)
}

/// Restore the src directory from backup.
fn restore_src(cap_path: &Path, backup: &HashMap<String, Vec<u8>>) -> Result<()> {
    let src_path = cap_path.join("src");

    // Remove any files the tester created
    if src_path.exists() {
        for entry in fs::read_dir(&src_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            }
        }
    }

    // Restore original files
    for (name, content) in backup {
        let path = src_path.join(name);
        fs::write(&path, content)?;
    }

    Ok(())
}

/// Run the tester agent loop.
pub fn run_tester_agent<C: AiClient + Sync>(
    client: &C,
    capabilities_root: &str,
    new_id: &str,
    cap_path: &Path,
    max_steps: usize,
) -> Result<()> {
    // Backup src before tester runs - will be restored after
    let src_backup = backup_src(cap_path)?;
    log::info(format!(
        "Backed up {} src files (will restore after tester)",
        src_backup.len()
    ));

    // Run the agent and restore src regardless of outcome
    let result = run_tester_agent_inner(client, capabilities_root, new_id, cap_path, max_steps);

    // Restore src to original state
    if let Err(e) = restore_src(cap_path, &src_backup) {
        log::error(format!("Failed to restore src: {}", e));
    } else {
        log::info("Restored src to original state");
    }

    result
}

/// Inner tester agent loop.
fn run_tester_agent_inner<C: AiClient + Sync>(
    client: &C,
    capabilities_root: &str,
    new_id: &str,
    cap_path: &Path,
    max_steps: usize,
) -> Result<()> {
    let system_prompt = prompts::build_tester_prompt(capabilities_root, new_id, cap_path);
    let tools = tool_defs::tester_tool_definitions();

    let mut messages = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": "Write tests based on the plan. Use write_file. Reply DONE when finished."}),
    ];

    let mut handler = tool_handler::TesterToolHandler::new(capabilities_root, new_id, cap_path);

    for step in 0..max_steps {
        log::agent_step(LogAgent::Tester, step + 1);

        let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
        let response = client.chat(request)?;
        let choice = response.choices.into_iter().next().context("no choices")?;
        let msg = choice.message;

        if let Some(tool_calls) = msg.tool_calls.clone() {
            if let Some(ref content) = msg.content {
                if !content.trim().is_empty() {
                    log::agent_message(LogAgent::Tester, content);
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
                log::tool_call(LogAgent::Tester, &tc.function.name, &tc.function.arguments);

                let result = handler.handle(&tc)?;
                if result.starts_with("ERROR") {
                    log::tool_error(LogAgent::Tester, &result);
                } else {
                    log::tool_success(LogAgent::Tester, &result);
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
            log::agent_message(LogAgent::Tester, &content);
            if content.to_uppercase().contains("DONE") {
                log::agent_done(LogAgent::Tester);
                return Ok(());
            }
        }
        messages.push(json!({"role": "assistant", "content": msg.content.unwrap_or_default()}));
        messages.push(json!({"role": "user", "content": "Continue. Reply DONE when finished."}));
    }

    log::error("Tester agent reached max steps without DONE");
    anyhow::bail!("Tester agent reached max steps without DONE")
}
