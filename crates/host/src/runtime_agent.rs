// crates/host/src/runtime_agent.rs

//! Runtime agent that orchestrates capabilities.

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, InputItem, ResponseItem, Tool};
use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::embedding::Embedder;

use crate::coding_agent::CodingAgent;
use crate::log::{self, Agent};
use crate::store::CapabilityStore;

/// The runtime agent that handles user tasks.
pub struct RuntimeAgent<'a, C: AiClient, E: Embedder> {
    client: &'a C,
    store: &'a mut CapabilityStore,
    runner: &'a CapabilityRunner,
    embedder: &'a E,
    capabilities_root: &'a str,
    max_steps: usize,
}

impl<'a, C: AiClient, E: Embedder> RuntimeAgent<'a, C, E> {
    pub fn new(
        client: &'a C,
        store: &'a mut CapabilityStore,
        runner: &'a CapabilityRunner,
        embedder: &'a E,
        capabilities_root: &'a str,
    ) -> Self {
        Self {
            client,
            store,
            runner,
            embedder,
            capabilities_root,
            max_steps: 12,
        }
    }

    /// Run a task using available capabilities.
    pub fn run_task(&mut self, task: &str, capabilities_summary: &str) -> Result<String> {
        let instructions = build_instructions(capabilities_summary);
        let tools = runtime_tools();

        let mut input = vec![InputItem::user(task)];

        for step in 0..self.max_steps {
            log::step(Agent::Runtime, step + 1, input.len());

            // Small delay between steps to avoid rate limits
            if step > 0 {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            // Truncate old context - keep first message + last 6 items
            if input.len() > 8 {
                let first = input.remove(0);
                let drain_count = input.len().saturating_sub(6);
                input.drain(0..drain_count);
                input.insert(0, first);
                log::info(&format!("Truncated context to {} items", input.len()));
            }

            let response = self.client.respond(&instructions, input.clone(), &tools)?;

            if response.has_function_calls() {
                for item in &response.items {
                    if let ResponseItem::FunctionCall {
                        call_id,
                        name,
                        arguments,
                    } = item
                    {
                        log::tool_call(Agent::Runtime, name, arguments);
                        input.push(InputItem::function_call(call_id, name, arguments));

                        let result = self.handle_tool(name, arguments)?;
                        log::tool_result(Agent::Runtime, name, &result, false);
                        input.push(InputItem::function_output(call_id, result));
                    }
                }
            } else if let Some(text) = response.text() {
                log::response(Agent::Runtime, text);
                log::done(Agent::Runtime, "Task completed");
                return Ok(text.to_string());
            }
        }

        log::error(Agent::Runtime, "Reached max steps without answer");
        anyhow::bail!("Reached max steps without answer")
    }

    fn handle_tool(&mut self, name: &str, arguments: &str) -> Result<String> {
        match name {
            "run_capability" => {
                let args: serde_json::Value = serde_json::from_str(arguments)?;
                let cap_id = args["capability_id"]
                    .as_str()
                    .context("missing capability_id")?;
                let input_json = args["input_json"].as_str().context("missing input_json")?;

                let cap = self
                    .store
                    .get_capability(cap_id)
                    .context("capability not found")?
                    .clone();

                match self.runner.run_capability(&cap, input_json) {
                    Ok(output) => Ok(output),
                    Err(e) => Ok(format!("ERROR: {}", e)),
                }
            }
            "mutate_capability" => {
                let args: serde_json::Value = serde_json::from_str(arguments)?;
                let task = args["task_description"]
                    .as_str()
                    .context("missing task_description")?;

                // Get nearest capabilities for the coding agent to reference
                let nearest_caps: Vec<String> = self
                    .store
                    .capabilities()
                    .iter()
                    .take(3)
                    .map(|c| c.id.clone())
                    .collect();

                let agent = CodingAgent::new(self.client, self.capabilities_root);
                let result = agent.create_capability(task, &nearest_caps)?;

                // Reload capabilities
                self.store.reload(self.capabilities_root, self.embedder)?;

                Ok(format!(
                    "SUCCESS: New capability created.\n\nCapability ID: {}\nSummary: {}\n\nIMPORTANT: Use EXACTLY this ID in run_capability: {}",
                    result.capability_id, result.summary, result.capability_id
                ))
            }
            _ => Ok(format!("Unknown tool: {}", name)),
        }
    }
}

fn build_instructions(capabilities_summary: &str) -> String {
    format!(
        r#"You solve tasks using capabilities. NEVER answer directly - always use tools.

Available capabilities:
{capabilities_summary}

Rules:
1. If a capability matches the task: run_capability(id, json)
2. If NO capability matches: mutate_capability(task_description) to create one, then run it
3. ALWAYS use a tool. Never say "I can't" or answer without tools.
"#
    )
}

fn runtime_tools() -> Vec<Tool> {
    vec![
        Tool::function(
            "run_capability",
            "Execute an existing capability with JSON input.",
            json!({
                "type": "object",
                "properties": {
                    "capability_id": { "type": "string" },
                    "input_json": { "type": "string" }
                },
                "required": ["capability_id", "input_json"]
            }),
        ),
        Tool::function(
            "mutate_capability",
            "Create a NEW capability when none exist for the task.",
            json!({
                "type": "object",
                "properties": {
                    "task_description": { "type": "string", "description": "What the new capability should do" }
                },
                "required": ["task_description"]
            }),
        ),
    ]
}
