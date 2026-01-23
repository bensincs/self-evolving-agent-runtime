// crates/host/src/agent.rs

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest, ChatToolCall};
use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::embedding::Embedder;

use crate::mutation_agent::MutationAgent;
use crate::store::CapabilityStore;

/// The agent orchestrates the agentic loop: sending tasks to the LLM,
/// handling tool calls, and returning a final answer.
pub struct Agent<'a, C: AiClient, M: AiClient, E: Embedder> {
    client: &'a C,
    mutation_client: &'a M,
    store: &'a mut CapabilityStore,
    runner: &'a CapabilityRunner,
    embedder: &'a E,
    capabilities_root: &'a str,
    max_steps: usize,
    /// Track failures per capability to avoid repeated deprecation
    failure_counts: std::collections::HashMap<String, usize>,
}

impl<'a, C: AiClient, M: AiClient, E: Embedder> Agent<'a, C, M, E> {
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
        let tools = self.tool_definitions();

        let system_prompt = format!(
            "You are an agent that MUST solve tasks using executable capabilities.\n\
             You are given a list of capabilities (id and summary).\n\
             RULES:\n\
             - Use run_capability to execute an existing capability.\n\
             - If no capability exists for what you need, use mutate_capability to create one.\n\
             - After mutating, you can immediately run_capability with the new id.\n\n\
             {}",
            capabilities_summary
        );

        println!("system_prompt:\n{}", system_prompt);

        let mut messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": task }),
        ];

        for step in 0..self.max_steps {
            println!("\n[AGENT STEP {}]", step + 1);

            let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
            let response = self.client.chat(request)?;

            let choice = response
                .choices
                .into_iter()
                .next()
                .context("no choices in chat response")?;

            let msg = choice.message;

            // If there are tool calls, handle them
            if let Some(tool_calls) = msg.tool_calls.clone() {
                println!("[ASSISTANT TOOL CALLS]");

                // Push the assistant message with tool_calls into history
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

                // Run each tool and append results
                for tc in tool_calls {
                    let result = self.handle_tool_call(&tc)?;
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "name": tc.function.name,
                        "content": result,
                    }));
                }

                continue;
            }

            // No tool calls => final answer
            let content = msg.content.unwrap_or_else(|| "<no content>".to_string());
            println!("[AGENT FINAL]");
            println!("{content}");
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
        println!("[TOOL CALL] run_capability");

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

        println!("  capability_id = {}", capability_id);
        println!("  input_json    = {}", input_json);

        let cap = self
            .store
            .get_capability(capability_id)
            .with_context(|| format!("Requested capability_id '{}' not found", capability_id))?
            .clone();

        match self.runner.run_capability(&cap, input_json) {
            Ok(output) => {
                // Reset failure count on success
                self.failure_counts.remove(capability_id);
                println!("[TOOL OUTPUT]");
                println!("{output}");
                Ok(output)
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                println!("[TOOL ERROR] {}", error_msg);

                // Track failures - deprecate after 2 consecutive failures
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
                        println!(
                            "[AGENT] Warning: Failed to mark capability as deprecated: {}",
                            dep_err
                        );
                    }
                }

                // Return error to agent so it can try alternatives
                Ok(format!(
                    "ERROR: Capability '{}' failed: {}. Failures: {}/2 before deprecation.",
                    capability_id, error_msg, count
                ))
            }
        }
    }

    fn handle_mutate_capability(&mut self, tc: &ChatToolCall) -> Result<String> {
        println!("[TOOL CALL] mutate_capability");

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

        println!("  task_description     = {}", task_description);
        println!("  parent_capability_id = {}", parent_id);

        // Spawn mutation agent with the dedicated mutation client
        let mut mutation_agent = MutationAgent::new(self.mutation_client, self.capabilities_root);
        let result = mutation_agent.mutate_capability(task_description, parent_id)?;

        // Reload the store to pick up the new capability
        println!("[AGENT] Reloading capability store...");
        self.store.reload(self.capabilities_root, self.embedder)?;

        let output = format!(
            "Created new capability:\n  id: {}\n  summary: {}\n\nYou can now use run_capability with id '{}'.",
            result.capability_id, result.summary, result.capability_id
        );

        println!("[TOOL OUTPUT]");
        println!("{output}");

        Ok(output)
    }

    fn tool_definitions(&self) -> Vec<serde_json::Value> {
        vec![
            json!({
                "type": "function",
                "function": {
                    "name": "run_capability",
                    "description": "Execute one of the available capabilities with the provided JSON input.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "capability_id": {
                                "type": "string",
                                "description": "The ID of the capability to run. Must match one of the provided capabilities."
                            },
                            "input_json": {
                                "type": "string",
                                "description": "A JSON string to send to the capability stdin. The capability will respond with JSON on stdout."
                            }
                        },
                        "required": ["capability_id", "input_json"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "mutate_capability",
                    "description": "Create a new capability by copying and modifying an existing one. Use this when no existing capability can solve the task.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "task_description": {
                                "type": "string",
                                "description": "A clear description of what the new capability should do. Be specific about inputs and outputs."
                            },
                            "parent_capability_id": {
                                "type": "string",
                                "description": "The ID of an existing capability to copy and modify. Choose the most similar capability to what you need."
                            }
                        },
                        "required": ["task_description", "parent_capability_id"]
                    }
                }
            }),
        ]
    }
}
