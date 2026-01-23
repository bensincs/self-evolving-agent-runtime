// crates/host/src/mutation_agent/mod.rs

//! Agentic mutation engine that creates Rust-based WASM capabilities.
//!
//! The mutation agent:
//! 1. Copies an existing capability as a template
//! 2. Uses an LLM to modify the code for a new task
//! 3. Builds and tests the capability
//! 4. Registers it with the runtime

mod capability_ops;
mod prompts;
mod tools;

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest};

use capability_ops::CapabilityOps;
use prompts::build_system_prompt;
use tools::{CompletionArgs, ToolHandler, TOOL_DEFINITIONS};

/// An agentic mutation engine that creates Rust-based capabilities.
pub struct MutationAgent<'a, C: AiClient> {
    client: &'a C,
    capabilities_root: &'a str,
    max_steps: usize,
    tool_handler: ToolHandler,
}

/// Result of a successful mutation.
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub capability_id: String,
    pub summary: String,
}

impl<'a, C: AiClient> MutationAgent<'a, C> {
    pub fn new(client: &'a C, capabilities_root: &'a str) -> Self {
        Self {
            client,
            capabilities_root,
            max_steps: 30,
            tool_handler: ToolHandler::new(capabilities_root.to_string()),
        }
    }

    /// Mutate an existing capability to create a new one.
    pub fn mutate_capability(&mut self, task: &str, parent_id: &str) -> Result<MutationResult> {
        // Reset tool handler state
        self.tool_handler.reset();

        // Step 1: Generate new capability ID and copy parent
        let new_id = self.generate_new_id(task)?;
        let cap_ops = CapabilityOps::new(self.capabilities_root);
        cap_ops.copy_capability(parent_id, &new_id)?;

        println!("[MUTATION] Created '{}' from '{}'", new_id, parent_id);

        // Step 2: Read current state and build prompt
        let new_cap_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(&new_id);
        let main_rs_content = std::fs::read_to_string(new_cap_path.join("src/main.rs"))
            .with_context(|| format!("Failed to read {}/src/main.rs", new_cap_path.display()))?;

        let system_prompt = build_system_prompt(
            self.capabilities_root,
            &new_id,
            &new_cap_path,
            &main_rs_content,
            task,
        );

        println!("[MUTATION] Task: {}", task);

        // Step 3: Run the agent loop
        let result = self.run_agent_loop(&new_id, parent_id, task, system_prompt)?;

        Ok(result)
    }

    /// Run the main agent loop until completion or max steps.
    fn run_agent_loop(
        &mut self,
        new_id: &str,
        parent_id: &str,
        _task: &str,
        system_prompt: String,
    ) -> Result<MutationResult> {
        let tools = TOOL_DEFINITIONS.clone();

        let mut messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": format!("Create the capability. Start by writing the updated src/main.rs.") }),
        ];

        for step in 0..self.max_steps {
            println!("\n[STEP {}]", step + 1);

            let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
            let response = self.client.chat(request)?;

            let choice = response
                .choices
                .into_iter()
                .next()
                .context("no choices in chat response")?;

            let msg = choice.message;

            // Handle tool calls
            if let Some(tool_calls) = msg.tool_calls.clone() {
                // Push assistant message with tool calls
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

                // Handle each tool call
                for tc in tool_calls {
                    // Check if this is a completion attempt
                    if tc.function.name == "complete" {
                        if let Some(result) =
                            self.try_complete(&tc, new_id, parent_id, &mut messages)?
                        {
                            return Ok(result);
                        }
                        continue;
                    }

                    // Handle regular tool call
                    let result = self.tool_handler.handle(&tc, new_id)?;
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "name": tc.function.name,
                        "content": result,
                    }));
                }

                continue;
            }

            // No tool calls - agent is responding with text
            let content = msg.content.unwrap_or_default();
            if !content.is_empty() {
                println!("[MUTATION] {}", content);
            }

            // Check if the agent seems to be giving up
            if self.is_giving_up(&content) {
                println!("[MUTATION] Agent indicated task cannot be completed. Exiting.");
                anyhow::bail!(
                    "Mutation agent indicated task cannot be completed: {}",
                    content
                );
            }

            messages.push(json!({ "role": "assistant", "content": content }));
            messages.push(json!({
                "role": "user",
                "content": "Continue with the implementation. Use the tools to write code, build it, test it, and call complete() when done."
            }));
        }

        anyhow::bail!("Mutation agent reached max_steps without completing")
    }

    /// Try to complete the mutation, returns Some(result) on success, None if not ready.
    fn try_complete(
        &mut self,
        tc: &se_runtime_core::ai_client::ChatToolCall,
        new_id: &str,
        parent_id: &str,
        messages: &mut Vec<serde_json::Value>,
    ) -> Result<Option<MutationResult>> {
        let completion: CompletionArgs = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "name": tc.function.name,
                    "content": format!("ERROR: Invalid arguments. Need 'summary'. {}", e),
                }));
                return Ok(None);
            }
        };

        // Check requirements
        let mut missing = Vec::new();
        if !self.tool_handler.build_succeeded {
            missing.push("build (run 'build' tool to compile the WASM)");
        }
        if !self.tool_handler.test_passed {
            missing.push("test (run 'test' tool with sample input)");
        }

        if !missing.is_empty() {
            let error_msg = format!(
                "ERROR: Cannot complete yet. Missing steps:\n- {}\n\nComplete these steps first, then call complete() again.",
                missing.join("\n- ")
            );
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "name": tc.function.name,
                "content": error_msg,
            }));
            return Ok(None);
        }

        // Update meta.json with final summary
        let cap_ops = CapabilityOps::new(self.capabilities_root);
        cap_ops.update_meta_json(new_id, &completion.summary)?;

        // Mark parent as legacy if requested
        if completion.mark_parent_legacy {
            if let Err(e) = cap_ops.mark_as_legacy(parent_id, new_id) {
                println!("[MUTATION] Warning: Failed to mark parent as legacy: {}", e);
            }
        }

        println!("[MUTATION] Complete! Created: {}", new_id);

        Ok(Some(MutationResult {
            capability_id: new_id.to_string(),
            summary: completion.summary,
        }))
    }

    /// Generate a short, descriptive capability ID from the task using the LLM.
    fn generate_new_id(&self, task: &str) -> Result<String> {
        let crates_dir = Path::new(self.capabilities_root).join("crates");

        let naming_prompt = format!(
            r#"Generate a short, descriptive snake_case identifier for a capability that does:

{}

Rules:
- Use snake_case (lowercase with underscores)
- Keep it short (2-4 words max, e.g. "get_weather", "sort_json", "fetch_crypto_price")
- Be descriptive of what it does
- No numbers or version suffixes
- Just output the identifier, nothing else"#,
            task
        );

        let request = ChatRequest::new(vec![json!({
            "role": "user",
            "content": naming_prompt
        })]);

        let response = self.client.chat(request)?;
        let raw_name = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_else(|| "new_capability".to_string());

        // Clean up the name
        let base_name = raw_name
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
            .trim_matches('_')
            .to_string();

        let base_name =
            if base_name.is_empty() || !base_name.chars().next().unwrap_or('_').is_alphabetic() {
                "new_capability".to_string()
            } else {
                base_name
            };

        // Check for collisions
        let mut candidate = base_name.clone();
        let mut version = 1u32;
        while crates_dir.join(&candidate).exists() {
            version += 1;
            candidate = format!("{}_{}", base_name, version);
        }

        println!("[MUTATION] Generated capability name: {}", candidate);
        Ok(candidate)
    }

    /// Check if the agent's response indicates it's giving up.
    fn is_giving_up(&self, content: &str) -> bool {
        let lower = content.to_lowercase();
        lower.contains("cannot complete")
            || lower.contains("unable to")
            || lower.contains("not possible")
            || lower.contains("cannot be done")
            || lower.contains("impossible")
    }
}
