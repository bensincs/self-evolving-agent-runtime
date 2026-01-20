// crates/host/src/mutation_agent.rs

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, ChatRequest, ChatToolCall};

/// An agentic mutation engine that creates Rust-based capabilities.
pub struct MutationAgent<'a, C: AiClient> {
    client: &'a C,
    capabilities_root: &'a str,
    max_steps: usize,
    /// Tracks whether cargo build --release has succeeded
    build_succeeded: bool,
    /// Tracks whether the capability has been tested
    test_passed: bool,
    /// Tracks consecutive build failures to detect loops
    consecutive_build_failures: usize,
}

/// Result of a successful mutation.
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
            build_succeeded: false,
            test_passed: false,
            consecutive_build_failures: 0,
        }
    }

    /// Mutate an existing capability to create a new one.
    pub fn mutate_capability(&mut self, task: &str, parent_id: &str) -> Result<MutationResult> {
        // Reset state for new mutation
        self.build_succeeded = false;
        self.test_passed = false;

        // Step 1: Generate new capability id and copy parent
        let new_id = self.generate_new_id(parent_id)?;
        self.copy_capability(parent_id, &new_id)?;

        // Update Cargo.toml with new package name
        self.update_cargo_toml(&new_id)?;

        println!("[MUTATION] Created '{}' from '{}'", new_id, parent_id);

        // Step 2: Let agent modify the copy
        let tools = self.tool_definitions();
        let new_cap_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(&new_id);

        // Read current state of the copied capability
        let main_rs_path = new_cap_path.join("src/main.rs");
        let main_rs_content = fs::read_to_string(&main_rs_path)
            .with_context(|| format!("Failed to read {}", main_rs_path.display()))?;

        let system_prompt =
            self.build_system_prompt(&new_id, &new_cap_path, &main_rs_content, task);

        println!("[MUTATION] Task: {}", task);

        let mut messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": format!("Create a capability that: {}", task) }),
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
                    let result = self.handle_tool_call(&tc, &new_id)?;

                    // Check if this was the complete() call
                    if tc.function.name == "complete" {
                        if let Ok(completion) =
                            serde_json::from_str::<CompletionArgs>(&tc.function.arguments)
                        {
                            // Check all requirements before allowing completion
                            let mut missing = Vec::new();
                            if !self.build_succeeded {
                                missing
                                    .push("build (run 'cargo build --release' to create binary)");
                            }
                            if !self.test_passed {
                                missing.push("test (run the capability with sample input)");
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
                                continue;
                            }

                            // Update meta.json with the final summary
                            self.update_meta_json(&new_id, &completion.summary)?;

                            // If requested, mark the parent as legacy
                            if completion.mark_parent_legacy {
                                if let Err(e) = self.mark_as_legacy(parent_id, &new_id) {
                                    println!(
                                        "[MUTATION] Warning: Failed to mark parent as legacy: {}",
                                        e
                                    );
                                }
                            }

                            println!("[MUTATION] Complete! Created: {}", new_id);
                            return Ok(MutationResult {
                                capability_id: new_id,
                                summary: completion.summary,
                            });
                        }
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

            // No tool calls - the agent is responding with text
            let content = msg.content.unwrap_or_default();
            if !content.is_empty() {
                println!("[MUTATION] {}", content);
            }

            // Check if the agent seems to be giving up (mentions inability, cannot, etc.)
            let lower = content.to_lowercase();
            if lower.contains("cannot complete")
                || lower.contains("unable to")
                || lower.contains("not possible")
                || lower.contains("cannot be done")
                || lower.contains("impossible")
            {
                println!("[MUTATION] Agent indicated task cannot be completed. Exiting.");
                anyhow::bail!(
                    "Mutation agent indicated task cannot be completed: {}",
                    content
                );
            }

            messages.push(json!({
                "role": "assistant",
                "content": content
            }));

            messages.push(json!({
                "role": "user",
                "content": "Continue with the implementation. Use the tools to write code, build it, test it, and call complete() when done."
            }));
        }

        anyhow::bail!("Mutation agent reached max_steps without completing")
    }

    fn build_system_prompt(
        &self,
        new_id: &str,
        cap_path: &Path,
        main_rs: &str,
        task: &str,
    ) -> String {
        format!(
            r#"You are an expert Rust developer creating a self-contained capability.

## TASK
{task}

## CAPABILITY INFO
- ID: {new_id}
- Path: {cap_path}
- Source: {cap_path}/src/main.rs
- After build: capabilities/target/release/{new_id}

## CURRENT src/main.rs
```rust
{main_rs}
```

## CAPABILITY_COMMON LIBRARY
You have access to the `capability_common` crate with these helpers:

```rust
// Read typed JSON input from stdin
let input: MyInput = capability_common::read_input()?;

// Read raw JSON value
let value: serde_json::Value = capability_common::read_input_value()?;

// Write JSON output
capability_common::write_output(&my_output);

// Write error response
capability_common::write_error("Something went wrong");

// HTTP GET request (returns String)
let body = capability_common::http_get("https://api.example.com/data")?;

// HTTP GET with JSON parsing
let data: MyResponse = capability_common::http_get_json("https://api.example.com/data")?;

// Time utilities
let iso_time = capability_common::utc_now_iso8601();  // "2024-01-20T15:30:45Z"
let timestamp = capability_common::utc_now_timestamp();  // Unix seconds
let millis = capability_common::utc_now_timestamp_millis();  // Unix milliseconds

// Recommended: Use the run() helper for automatic error handling
capability_common::run(|input: MyInput| {{
    // Your logic here
    Ok(MyOutput {{ ... }})
}});
```

## EXAMPLE: Weather Capability
```rust
use serde::{{Deserialize, Serialize}};
use capability_common::serde_json::Value;

#[derive(Deserialize)]
struct Input {{
    city: Option<String>,
}}

#[derive(Serialize)]
struct Output {{
    temperature_c: f64,
    description: String,
}}

fn main() {{
    capability_common::run(|input: Input| {{
        let city = input.city.unwrap_or_else(|| "London".to_string());
        let url = format!("https://wttr.in/{{}}?format=j1", city);

        let body: Value = capability_common::http_get_json(&url)?;

        let temp = body["current_condition"][0]["temp_C"]
            .as_str()
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap_or(0.0);
        let desc = body["current_condition"][0]["weatherDesc"][0]["value"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();

        Ok(Output {{ temperature_c: temp, description: desc }})
    }});
}}
```

## AVAILABLE TOOLS
1. **write_file** - Write to any file (path, content required)
2. **read_file** - Read a file
3. **build** - Run `cargo build --release` to create the binary (required before complete)
4. **test** - Run the capability with test input JSON
5. **complete** - Finish (only works after successful build AND test)

## WORKFLOW (FOLLOW THIS ORDER)
1. Modify src/main.rs for your task using capability_common helpers
2. If you need additional dependencies, edit Cargo.toml (see below)
3. Run **build** to compile the release binary
4. If build fails, fix errors and build again
5. Run **test** with sample input to verify it works
6. Call **complete** with a summary

## DEPENDENCIES

### Already included (use directly):
- **serde** - `use serde::{{Serialize, Deserialize}};`
- **capability_common** - HTTP helpers, time utilities, run() function, serde_json

### Workspace dependencies (add to Cargo.toml if needed):
To add a workspace dependency, add a line like `regex.workspace = true` under [dependencies].

Available in workspace:
- **regex** - Regular expressions
- **base64** - Base64 encoding/decoding
- **url** - URL parsing
- **rand** - Random number generation
- **uuid** - UUID generation (v4)
- **chrono** - Date/time (also available via capability_common)

### Example Cargo.toml with extra dependency:
```toml
[package]
name = "{new_id}"
version = "0.1.0"
edition = "2021"

[dependencies]
capability_common.workspace = true
serde.workspace = true
regex.workspace = true  # Added for this capability
```

### Adding non-workspace dependencies:
If you need a crate NOT in the workspace, add it with a version:
```toml
some_crate = "1.0"
```
But prefer workspace dependencies when available.

## RULES
- Use capability_common helpers (run, http_get_json, write_output, utc_now_iso8601, etc.)
- For errors in handlers, use `capability_common::CapabilityError::new("message")`
- Use real APIs, never mock data
- Free no-auth APIs: wttr.in, ip-api.com, api.coingecko.com
- Handle all errors - the run() helper does this automatically
- Keep it simple and focused
- MUST run build AND test successfully before complete
- If the task is impossible with available dependencies, call complete with an error summary

Now implement the capability. Start by writing the updated src/main.rs."#,
            task = task,
            new_id = new_id,
            cap_path = cap_path.display(),
            main_rs = main_rs,
        )
    }

    fn generate_new_id(&self, parent_id: &str) -> Result<String> {
        let crates_dir = Path::new(self.capabilities_root).join("crates");

        // Strip _rust suffix for cleaner naming
        let base_id = parent_id.strip_suffix("_rust").unwrap_or(parent_id);

        // Find existing versions
        let mut max_version = 0u32;
        let prefix = format!("{}_v", base_id);

        if let Ok(entries) = fs::read_dir(&crates_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&prefix) {
                    if let Some(version_str) = name_str.strip_prefix(&prefix) {
                        if let Ok(v) = version_str.parse::<u32>() {
                            max_version = max_version.max(v);
                        }
                    }
                }
            }
        }

        Ok(format!("{}_v{}", base_id, max_version + 1))
    }

    /// Create a new capability by copying the parent's entire crate directory.
    fn copy_capability(&self, parent_id: &str, new_id: &str) -> Result<()> {
        let crates_dir = Path::new(self.capabilities_root).join("crates");
        let src = crates_dir.join(parent_id);
        let dst = crates_dir.join(new_id);

        if !src.exists() {
            anyhow::bail!(
                "Parent capability '{}' not found at {}",
                parent_id,
                src.display()
            );
        }

        if dst.exists() {
            anyhow::bail!("Destination '{}' already exists", dst.display());
        }

        // Copy entire directory tree
        self.copy_dir_recursive(&src, &dst)?;

        // Update package name in Cargo.toml
        let cargo_path = dst.join("Cargo.toml");
        let cargo_content = fs::read_to_string(&cargo_path)?;
        let updated_cargo = cargo_content.replace(
            &format!("name = \"{}\"", parent_id),
            &format!("name = \"{}\"", new_id),
        );
        fs::write(&cargo_path, updated_cargo)?;

        // Update meta.json with new id
        let meta = json!({
            "id": new_id,
            "summary": "New capability (pending implementation)",
            "binary": format!("../../target/release/{}", new_id)
        });
        fs::write(dst.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

        Ok(())
    }

    /// Recursively copy a directory.
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    /// Update Cargo.toml with the new package name (no longer needed but kept for compatibility).
    fn update_cargo_toml(&self, _new_id: &str) -> Result<()> {
        // No longer needed since we create the Cargo.toml fresh
        Ok(())
    }

    fn update_meta_json(&self, capability_id: &str, summary: &str) -> Result<()> {
        let meta_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        let meta = json!({
            "id": capability_id,
            "summary": summary,
            "binary": format!("../../target/release/{}", capability_id),
            "status": "active"
        });

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }

    /// Mark a capability as legacy (replaced by a newer version).
    fn mark_as_legacy(&self, capability_id: &str, replaced_by: &str) -> Result<()> {
        let meta_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        if !meta_path.exists() {
            anyhow::bail!("Capability '{}' not found", capability_id);
        }

        let content = fs::read_to_string(&meta_path)?;
        let mut meta: serde_json::Value = serde_json::from_str(&content)?;

        meta["status"] = json!("legacy");
        meta["replaced_by"] = json!(replaced_by);

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        println!(
            "[MUTATION] Marked '{}' as legacy (replaced by '{}')",
            capability_id, replaced_by
        );
        Ok(())
    }

    fn handle_tool_call(&mut self, tc: &ChatToolCall, new_id: &str) -> Result<String> {
        match tc.function.name.as_str() {
            "read_file" => self.handle_read_file(tc),
            "write_file" => self.handle_write_file(tc),
            "build" => self.handle_build(new_id),
            "test" => self.handle_test(tc, new_id),
            "complete" => self.handle_complete(tc),
            other => Ok(format!("ERROR: Unknown tool '{}'", other)),
        }
    }

    fn handle_read_file(&self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'path'. {}", e)),
        };

        println!("[TOOL] read_file: {}", args.path);

        match fs::read_to_string(&args.path) {
            Ok(content) => Ok(content),
            Err(e) => Ok(format!("ERROR: {}", e)),
        }
    }

    fn handle_write_file(&mut self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'path' and 'content'. {}",
                    e
                ))
            }
        };

        println!("[TOOL] write_file: {}", args.path);

        // Reset validation state since code has changed
        self.build_succeeded = false;
        self.test_passed = false;

        // Create parent directories if needed
        if let Some(parent) = Path::new(&args.path).parent() {
            fs::create_dir_all(parent)?;
        }

        match fs::write(&args.path, &args.content) {
            Ok(()) => Ok(format!(
                "OK: Wrote {} bytes to {}",
                args.content.len(),
                args.path
            )),
            Err(e) => Ok(format!("ERROR: {}", e)),
        }
    }

    fn handle_build(&mut self, new_id: &str) -> Result<String> {
        let workspace_root = Path::new(self.capabilities_root);

        println!(
            "[TOOL] build: cargo build --release -p {} in {}",
            new_id,
            workspace_root.display()
        );

        let output = Command::new("cargo")
            .args(["build", "--release", "-p", new_id])
            .current_dir(workspace_root)
            .output()
            .context("failed to run cargo build")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            self.build_succeeded = true;
            self.consecutive_build_failures = 0; // Reset on success
            let binary_path = workspace_root.join("target/release").join(new_id);
            Ok(format!(
                "OK: Build successful! Binary at: {}\n{}",
                binary_path.display(),
                stderr
            ))
        } else {
            self.build_succeeded = false;
            self.consecutive_build_failures += 1;

            // If we've failed 3+ times in a row, provide a stronger hint
            if self.consecutive_build_failures >= 3 {
                Ok(format!(
                    "ERROR: Build failed {} times in a row. You may be trying to use a dependency that isn't available.\n\
                    REMINDER: You can ONLY use what's in capability_common. You CANNOT add dependencies to Cargo.toml.\n\
                    Available: serde, serde_json, ureq (HTTP), chrono (time).\n\
                    If the task requires unavailable dependencies, use the complete tool to report that the task cannot be done.\n\n\
                    Build error:\n{}\n{}",
                    self.consecutive_build_failures,
                    stdout,
                    stderr
                ))
            } else {
                Ok(format!("ERROR: Build failed:\n{}\n{}", stdout, stderr))
            }
        }
    }

    fn handle_test(&mut self, tc: &ChatToolCall, new_id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            input: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'input' (JSON string). {}",
                    e
                ))
            }
        };

        // Check if we need to rebuild first
        if !self.build_succeeded {
            return Ok(
                "ERROR: Code has changed since last build. Run 'build' first to compile your changes."
                    .to_string(),
            );
        }

        let workspace_root = Path::new(self.capabilities_root);
        let binary_path = workspace_root.join("target/release").join(new_id);

        if !binary_path.exists() {
            return Ok("ERROR: Binary not found. Run 'build' first.".to_string());
        }

        println!(
            "[TOOL] test: {} with input: {}",
            binary_path.display(),
            args.input
        );

        let mut child = Command::new(&binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to run capability")?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(args.input.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            self.test_passed = true;
            Ok(format!("OK: Output:\n{}\nStderr:\n{}", stdout, stderr))
        } else {
            self.test_passed = false;
            Ok(format!(
                "ERROR: Exit code {:?}\nStdout:\n{}\nStderr:\n{}",
                output.status.code(),
                stdout,
                stderr
            ))
        }
    }

    fn handle_complete(&self, tc: &ChatToolCall) -> Result<String> {
        let args: CompletionArgs = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'summary'. {}", e)),
        };

        println!("[TOOL] complete: {}", args.summary);

        Ok(format!("Mutation complete! Summary: {}", args.summary))
    }

    fn tool_definitions(&self) -> Vec<serde_json::Value> {
        vec![
            json!({
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read the contents of a file.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Absolute path to the file."
                            }
                        },
                        "required": ["path"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description": "Write content to a file. Creates directories if needed.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Absolute path to the file."
                            },
                            "content": {
                                "type": "string",
                                "description": "The content to write."
                            }
                        },
                        "required": ["path", "content"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "build",
                    "description": "Run 'cargo build --release' to compile the capability into a binary. Required before calling complete.",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "test",
                    "description": "Test the compiled capability by running it with sample input.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "input": {
                                "type": "string",
                                "description": "JSON input to send to the capability via stdin."
                            }
                        },
                        "required": ["input"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "complete",
                    "description": "Signal that the capability is finished. Only works after successful check, build, and test.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "summary": {
                                "type": "string",
                                "description": "A one-line description of what the capability does."
                            },
                            "mark_parent_legacy": {
                                "type": "boolean",
                                "description": "Set to true if this capability REPLACES or IMPROVES the parent (marks parent as legacy). Set to false if this is just a new variant/derivative."
                            }
                        },
                        "required": ["summary"]
                    }
                }
            }),
        ]
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CompletionArgs {
    summary: String,
    /// If true, mark the parent capability as legacy (replaced by this new one).
    /// Use this when the new capability is an improvement/fix, not just a variant.
    #[serde(default)]
    mark_parent_legacy: bool,
}
