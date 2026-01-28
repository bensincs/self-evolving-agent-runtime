// crates/host/src/coding_agent.rs

//! Single unified coding agent that writes tests and implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use se_runtime_core::ai_client::{AiClient, InputItem, ResponseItem, Tool};

use crate::log::{self, Agent};

/// Result of a successful mutation.
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub capability_id: String,
    pub summary: String,
}

/// The coding agent that creates and tests capabilities.
pub struct CodingAgent<'a, C: AiClient> {
    client: &'a C,
    capabilities_root: &'a str,
    max_steps: usize,
}

impl<'a, C: AiClient> CodingAgent<'a, C> {
    pub fn new(client: &'a C, capabilities_root: &'a str) -> Self {
        Self {
            client,
            capabilities_root,
            max_steps: 30,
        }
    }

    /// Create a new capability with nearest capabilities for reference.
    pub fn create_capability(&self, task: &str, nearest_caps: &[String]) -> Result<MutationResult> {
        // Generate unique ID
        let new_id = generate_capability_id();
        log::info(format!("Creating {}", new_id));

        // Scaffold empty capability
        self.scaffold_capability(&new_id)?;

        // Get capability path
        let cap_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(&new_id);

        // Build prompt with nearest capabilities
        let instructions = build_prompt(&new_id, task, nearest_caps);
        let tools = tool_definitions();

        // Agent loop
        let mut input = vec![InputItem::user(
            "Read files first, then implement. Reply DONE when tests pass.",
        )];

        for step_num in 0..self.max_steps {
            log::step(Agent::Coding, step_num + 1, input.len());

            if step_num > 0 {
                std::thread::sleep(std::time::Duration::from_millis(500));
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
                        log::tool_call(Agent::Coding, name, arguments);
                        input.push(InputItem::function_call(call_id, name, arguments));

                        let result = self.handle_tool(name, arguments, &cap_path, &new_id)?;
                        let is_error = result.starts_with("ERROR") || result.starts_with("FAILED");
                        log::tool_result(Agent::Coding, name, &result, is_error);

                        let truncated = if result.len() > 4000 {
                            format!("{}...[truncated]", &result[..4000])
                        } else {
                            result
                        };
                        input.push(InputItem::function_output(call_id, truncated));
                    }
                }
            } else if let Some(text) = response.text() {
                log::response(Agent::Coding, text);

                if text.to_uppercase().contains("DONE") {
                    // Run tests - if they pass, we're done
                    let (passed, output) = run_cargo_test(self.capabilities_root, &new_id)?;
                    if passed {
                        // Build WASM
                        let build_ok = run_wasm_build(self.capabilities_root, &new_id)?;
                        if build_ok {
                            let summary = clean_summary(task);
                            self.update_meta(&new_id, &summary)?;
                            log::done(Agent::Coding, &new_id);
                            return Ok(MutationResult {
                                capability_id: new_id,
                                summary,
                            });
                        } else {
                            input.push(InputItem::assistant(text));
                            input.push(InputItem::user("WASM build failed. Fix it."));
                        }
                    } else {
                        // Tests failed - tell agent and continue loop
                        let truncated = &output[..output.len().min(2000)];
                        input.push(InputItem::assistant(text));
                        input.push(InputItem::user(&format!(
                            "Tests FAILED:\n{}\nFix and say DONE.",
                            truncated
                        )));
                    }
                } else {
                    input.push(InputItem::assistant(text));
                    input.push(InputItem::user("Continue."));
                }
            }
        }

        log::error(Agent::Coding, "Reached max steps");
        anyhow::bail!("Agent reached max steps without completing")
    }

    fn handle_tool(
        &self,
        name: &str,
        arguments: &str,
        cap_path: &Path,
        new_id: &str,
    ) -> Result<String> {
        match name {
            "list_files" => {
                let args: ListFilesArgs = serde_json::from_str(arguments)?;
                let path = resolve_path(&args.path, cap_path);
                match fs::read_dir(&path) {
                    Ok(entries) => {
                        let mut files: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                if e.path().is_dir() {
                                    format!("{}/", name)
                                } else {
                                    name
                                }
                            })
                            .collect();
                        files.sort();
                        Ok(files.join("\n"))
                    }
                    Err(e) => Ok(format!("ERROR: {}", e)),
                }
            }
            "read_file" => {
                let args: ReadFileArgs = serde_json::from_str(arguments)?;
                let path = resolve_path(&args.path, cap_path);
                match fs::read_to_string(&path) {
                    Ok(content) => Ok(content),
                    Err(e) => Ok(format!("ERROR: {}", e)),
                }
            }
            "write_file" => {
                let args: WriteFileArgs = serde_json::from_str(arguments)?;
                let path = resolve_path(&args.path, cap_path);

                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, &args.content)?;
                Ok(format!(
                    "Wrote {} bytes to {}",
                    args.content.len(),
                    path.display()
                ))
            }
            "test" => {
                let (passed, output) = run_cargo_test(self.capabilities_root, new_id)?;
                if passed {
                    Ok(format!("PASSED\n{}", output))
                } else {
                    Ok(format!("FAILED\n{}", output))
                }
            }
            "build" => {
                let success = run_wasm_build(self.capabilities_root, new_id)?;
                if success {
                    Ok("OK".to_string())
                } else {
                    Ok("FAILED".to_string())
                }
            }
            _ => Ok(format!("Unknown: {}", name)),
        }
    }

    fn scaffold_capability(&self, new_id: &str) -> Result<()> {
        let crates_dir = Path::new(self.capabilities_root).join("crates");
        let dst = crates_dir.join(new_id);

        if dst.exists() {
            anyhow::bail!("Destination '{}' already exists", new_id);
        }

        // Create directories
        fs::create_dir_all(dst.join("src"))?;
        fs::create_dir_all(dst.join("tests"))?;

        // Cargo.toml
        let cargo = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
capability_common = {{ path = "../common" }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
            new_id
        );
        fs::write(dst.join("Cargo.toml"), cargo)?;

        // src/lib.rs stub - compile error forces agent to write real code
        let lib_rs = r#"compile_error!("lib.rs not implemented - write your code here");
"#;
        fs::write(dst.join("src/lib.rs"), lib_rs)?;

        // src/main.rs stub - compile error forces agent to write real code
        let main_rs = r#"compile_error!("main.rs not implemented - write your code here");
"#;
        fs::write(dst.join("src/main.rs"), main_rs)?;

        // tests/integration.rs stub - fails by default so agent must write real tests
        let test_rs = r#"#[test]
fn test_not_implemented() {
    panic!("Tests not implemented - write real tests!");
}
"#;
        fs::write(dst.join("tests/integration.rs"), test_rs)?;

        // meta.json
        let meta = json!({
            "id": new_id,
            "summary": "TODO",
            "binary": format!("../../target/wasm32-wasip1/release/{}.wasm", new_id)
        });
        fs::write(dst.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

        Ok(())
    }

    fn update_meta(&self, capability_id: &str, summary: &str) -> Result<()> {
        let meta_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        let meta = json!({
            "id": capability_id,
            "summary": summary,
            "binary": format!("../../target/wasm32-wasip1/release/{}.wasm", capability_id)
        });
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool argument types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListFilesArgs {
    path: String,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    path: String,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool definitions
// ─────────────────────────────────────────────────────────────────────────────

fn tool_definitions() -> Vec<Tool> {
    vec![
        Tool::function(
            "list_files",
            "List files in a directory",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path, use . for capability root" }
                },
                "required": ["path"]
            }),
        ),
        Tool::function(
            "read_file",
            "Read file contents",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        ),
        Tool::function(
            "write_file",
            "Write file",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        Tool::function(
            "test",
            "Run cargo test",
            json!({ "type": "object", "properties": {}, "required": [] }),
        ),
        Tool::function(
            "build",
            "Build WASM",
            json!({ "type": "object", "properties": {}, "required": [] }),
        ),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Prompt
// ─────────────────────────────────────────────────────────────────────────────

fn build_prompt(new_id: &str, task: &str, nearest_caps: &[String]) -> String {
    let example_cap = nearest_caps.first().map(|s| s.as_str()).unwrap_or("cap1");
    format!(
        r#"Rust capability agent. Implement: {task}

Working dir: {new_id}
Example: ../{example_cap}/src/lib.rs, ../{example_cap}/src/main.rs

YOU MUST WRITE CODE. The scaffold has compile_error! so tests will fail until you write real code.

Steps:
1. read_file("../{example_cap}/src/lib.rs") and read_file("../{example_cap}/src/main.rs") for pattern
2. write_file("./src/lib.rs", YOUR_IMPLEMENTATION) - REQUIRED
3. write_file("./src/main.rs", YOUR_MAIN) - REQUIRED
4. write_file("./tests/integration.rs", YOUR_TESTS) - REQUIRED
5. test() to verify
6. Say DONE only after tests pass

DO NOT say DONE until you have written all 3 files and tests pass.
"#
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn generate_capability_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("cap_{}", ts % 100000)
}

/// Clean up task description to be a proper summary.
/// Removes "Create capability 'X'" prefixes and other noise.
fn clean_summary(task: &str) -> String {
    // Remove common prefixes like "Create capability 'foo' to"
    let cleaned = if let Some(idx) = task.find("' to ") {
        task[(idx + 5)..].to_string()
    } else if let Some(idx) = task.find("' that ") {
        task[(idx + 7)..].to_string()
    } else {
        task.to_string()
    };

    // Capitalize first letter
    let mut chars: Vec<char> = cleaned.chars().collect();
    if let Some(c) = chars.first_mut() {
        *c = c.to_uppercase().next().unwrap_or(*c);
    }
    chars.into_iter().collect()
}

fn resolve_path(path_str: &str, cap_path: &Path) -> PathBuf {
    let path = Path::new(path_str);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cap_path.join(path)
    }
}

fn run_cargo_test(capabilities_root: &str, package: &str) -> Result<(bool, String)> {
    let output = Command::new("cargo")
        .args(["test", "-p", package, "--", "--nocapture"])
        .current_dir(capabilities_root)
        .output()
        .context("failed to run cargo test")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    Ok((output.status.success(), combined))
}

fn run_wasm_build(capabilities_root: &str, package: &str) -> Result<bool> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "-p",
            package,
        ])
        .current_dir(capabilities_root)
        .output()
        .context("failed to run cargo build")?;

    Ok(output.status.success())
}
