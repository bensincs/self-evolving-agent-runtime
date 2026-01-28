// crates/host/src/agents/planner/mod.rs

//! Planner agent that orchestrates the mutation process.

mod prompts;
mod tool_defs;
mod tool_handler;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest};

use super::capability_ops::CapabilityOps;
use super::log::{self, Agent as LogAgent};
use super::MutationResult;

/// MutationAgent orchestrates capability creation using planner/coder/tester sub-agents.
pub struct MutationAgent<'a, C: AiClient + Sync> {
    client: &'a C,
    capabilities_root: &'a str,
}

impl<'a, C: AiClient + Sync> MutationAgent<'a, C> {
    pub fn new(client: &'a C, capabilities_root: &'a str) -> Self {
        Self {
            client,
            capabilities_root,
        }
    }

    /// Create a new capability by mutating an existing one.
    pub fn mutate_capability(&self, task: &str, parent_id: &str) -> Result<MutationResult> {
        log::info(format!(
            "Creating capability from '{}': {}",
            parent_id,
            &task[..task.len().min(80)]
        ));

        // Generate unique ID
        let new_id = generate_capability_id(parent_id);
        log::info(format!("New capability ID: {}", new_id));

        // Copy parent capability
        let cap_ops = CapabilityOps::new(self.capabilities_root);
        cap_ops.copy_capability(parent_id, &new_id)?;

        // Run the planner agent
        self.run_planner(&new_id, parent_id, task, 30)
    }

    fn run_planner(
        &self,
        new_id: &str,
        parent_id: &str,
        task: &str,
        max_steps: usize,
    ) -> Result<MutationResult> {
        let cap_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(new_id);

        let main_rs = fs::read_to_string(cap_path.join("src/main.rs"))?;
        let system_prompt = prompts::build_planner_prompt(task, parent_id, &main_rs);
        let tools = tool_defs::planner_tool_definitions();

        let mut messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": "Create plan.json, delegate to coder/tester, run test, and complete."}),
        ];

        let mut handler = tool_handler::PlannerToolHandler::new(
            self.client,
            self.capabilities_root,
            new_id,
            parent_id,
            task,
            max_steps,
        );

        for step in 0..max_steps {
            log::agent_step(LogAgent::Planner, step + 1);

            let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
            let response = self.client.chat(request)?;
            let choice = response.choices.into_iter().next().context("no choices")?;
            let msg = choice.message;

            if let Some(tool_calls) = msg.tool_calls.clone() {
                if let Some(ref content) = msg.content {
                    if !content.trim().is_empty() {
                        log::agent_message(LogAgent::Planner, content);
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
                    log::tool_call(LogAgent::Planner, &tc.function.name, &tc.function.arguments);

                    match handler.handle(&tc)? {
                        tool_handler::PlannerResult::Continue(result_msg) => {
                            if result_msg.starts_with("ERROR") {
                                log::tool_error(LogAgent::Planner, &result_msg);
                            } else {
                                log::tool_success(LogAgent::Planner, &result_msg);
                            }
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tc.id,
                                "name": tc.function.name,
                                "content": result_msg
                            }));
                        }
                        tool_handler::PlannerResult::Complete(mutation_result) => {
                            log::success(format!(
                                "Capability '{}' created: {}",
                                mutation_result.capability_id, mutation_result.summary
                            ));
                            return Ok(mutation_result);
                        }
                    }
                }
                continue;
            }

            if let Some(content) = msg.content.clone() {
                log::agent_message(LogAgent::Planner, &content);

                // Fallback: if model says DONE/complete without calling tool, force completion
                let content_upper = content.to_uppercase();
                if content_upper.contains("DONE")
                    || content_upper.contains("COMPLETE")
                    || content_upper.contains("SUCCESSFULLY")
                {
                    // Check if tests actually pass before auto-completing
                    let (test_passed, _) =
                        super::common::handle_test(self.capabilities_root, new_id)?;
                    if test_passed {
                        log::info("Auto-completing: model indicated done and tests pass");
                        let result = MutationResult {
                            capability_id: new_id.to_string(),
                            summary: "Capability created successfully".to_string(),
                        };
                        return Ok(result);
                    }
                }
            }
            messages.push(json!({"role": "assistant", "content": msg.content.unwrap_or_default()}));
            messages
                .push(json!({"role": "user", "content": "Call the complete() tool to finish."}));
        }

        log::error("Planner reached max steps without completing");
        anyhow::bail!("Planner reached max steps without completing")
    }
}

/// Generate a unique capability ID based on parent.
fn generate_capability_id(parent_id: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}_v{}", parent_id, timestamp % 10000)
}
