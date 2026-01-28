// crates/host/src/agents/runtime/mod.rs

//! Top-level Runtime agent that orchestrates capabilities.

mod prompts;
mod tool_defs;
mod tool_handler;

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest, ChatToolCall};
use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::embedding::Embedder;

use super::log::{self, Agent as LogAgent};
use super::planner::MutationAgent;
use crate::store::CapabilityStore;

/// The Runtime agent orchestrates the agentic loop: sending tasks to the LLM,
/// handling tool calls, and returning a final answer.
pub struct Agent<'a, C: AiClient, M: AiClient, E: Embedder> {
    client: &'a C,
    mutation_client: &'a M,
    store: &'a mut CapabilityStore,
    runner: &'a CapabilityRunner,
    embedder: &'a E,
    capabilities_root: &'a str,
    max_steps: usize,
    failure_counts: std::collections::HashMap<String, usize>,
}

impl<'a, C: AiClient, M: AiClient + Sync, E: Embedder> Agent<'a, C, M, E> {
    pub fn new(
        client: &'a C,
        mutation_client: &'a M,
        store: &'a mut CapabilityStore,
        runner: &'a CapabilityRunner,
        embedder: &'a E,
        capabilities_root: &'a str,
    ) -> Self {
        Self {
            client,
            mutation_client,
            store,
            runner,
            embedder,
            capabilities_root,
            max_steps: 12,
            failure_counts: std::collections::HashMap::new(),
        }
    }

    /// Run the agentic loop for a given task.
    pub fn run_task(&mut self, task: &str, capabilities_summary: &str) -> Result<String> {
        let tools = tool_defs::runtime_tool_definitions();
        let system_prompt = prompts::build_runtime_prompt(capabilities_summary);

        log::info(format!("System prompt: {} chars", system_prompt.len()));

        let mut messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": task }),
        ];

        for step in 0..self.max_steps {
            log::agent_step(LogAgent::Runtime, step + 1);

            let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
            let response = self.client.chat(request)?;

            let choice = response
                .choices
                .into_iter()
                .next()
                .context("no choices in chat response")?;

            let msg = choice.message;

            if let Some(tool_calls) = msg.tool_calls.clone() {
                // Log any thinking/content before tool calls
                if let Some(ref content) = msg.content {
                    if !content.trim().is_empty() {
                        log::agent_message(LogAgent::Runtime, content);
                    }
                }

                let assistant_msg = json!({
                    "role": "assistant",
                    "content": msg.content.clone(),
                    "tool_calls": tool_calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": tc.call_type,
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments,
                            }
                        })
                    }).collect::<Vec<_>>()
                });
                messages.push(assistant_msg);

                for tc in tool_calls {
                    log::tool_call(LogAgent::Runtime, &tc.function.name, &tc.function.arguments);
                    let result = self.handle_tool_call(&tc)?;
                    if result.starts_with("ERROR") {
                        log::tool_error(LogAgent::Runtime, &result);
                    } else {
                        log::tool_success(LogAgent::Runtime, &result);
                    }
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "name": tc.function.name,
                        "content": result,
                    }));
                }

                continue;
            }

            let content = msg.content.unwrap_or_else(|| "<no content>".to_string());
            log::agent_done(LogAgent::Runtime);
            log::success(format!("Final answer: {}", &content[..content.len().min(100)]));
            return Ok(content);
        }

        anyhow::bail!("Agentic loop reached max_steps without a final answer")
    }

    fn handle_tool_call(&mut self, tc: &ChatToolCall) -> Result<String> {
        match tc.function.name.as_str() {
            "run_capability" => self.handle_run_capability(tc),
            "mutate_capability" => self.handle_mutate_capability(tc),
            other => anyhow::bail!("Unknown tool: {}", other),
        }
    }

    fn handle_run_capability(&mut self, tc: &ChatToolCall) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
            .context("failed to parse run_capability.arguments as JSON")?;

        let capability_id = args
            .get("capability_id")
            .and_then(|v| v.as_str())
            .context("run_capability.arguments missing 'capability_id'")?;

        let input_json = args
            .get("input_json")
            .and_then(|v| v.as_str())
            .context("run_capability.arguments missing 'input_json'")?;

        let cap = self
            .store
            .get_capability(capability_id)
            .with_context(|| format!("Requested capability_id '{}' not found", capability_id))?
            .clone();

        match self.runner.run_capability(&cap, input_json) {
            Ok(output) => {
                self.failure_counts.remove(capability_id);
                Ok(output)
            }
            Err(e) => {
                let error_msg = format!("{}", e);

                let count = self
                    .failure_counts
                    .entry(capability_id.to_string())
                    .or_insert(0);
                *count += 1;

                if *count >= 2 {
                    let deprecation_reason =
                        format!("Failed {} times. Last error: {}", count, error_msg);
                    if let Err(dep_err) = self.store.mark_deprecated(
                        self.capabilities_root,
                        capability_id,
                        &deprecation_reason,
                    ) {
                        log::error(format!(
                            "Failed to mark capability as deprecated: {}",
                            dep_err
                        ));
                    }
                }

                Ok(format!(
                    "ERROR: Capability '{}' failed: {}. Failures: {}/2 before deprecation.",
                    capability_id, error_msg, count
                ))
            }
        }
    }

    fn handle_mutate_capability(&mut self, tc: &ChatToolCall) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
            .context("failed to parse mutate_capability.arguments as JSON")?;

        let task_description = args
            .get("task_description")
            .and_then(|v| v.as_str())
            .context("mutate_capability.arguments missing 'task_description'")?;

        let parent_id = args
            .get("parent_capability_id")
            .and_then(|v| v.as_str())
            .context("mutate_capability.arguments missing 'parent_capability_id'")?;

        let mutation_agent = MutationAgent::new(self.mutation_client, self.capabilities_root);
        let result = mutation_agent.mutate_capability(task_description, parent_id)?;

        log::info("Reloading capability store...");
        self.store.reload(self.capabilities_root, self.embedder)?;

        let output = format!(
            "Created new capability:\n  id: {}\n  summary: {}\n\nYou can now use run_capability with id '{}'.",
            result.capability_id, result.summary, result.capability_id
        );

        Ok(output)
    }
}
