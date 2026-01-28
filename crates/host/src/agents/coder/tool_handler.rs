// crates/host/src/agents/coder/tool_handler.rs

//! Tool handlers for the Coder agent.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Result;
use serde::Deserialize;

use se_runtime_core::ai_client::ChatToolCall;

use super::super::common::{self, ToolResult};

/// Tool handler for the Coder agent.
pub struct CoderToolHandler {
    capabilities_root: String,
    cap_path: PathBuf,
    new_id: String,
    src_path: PathBuf,
    tests_path: PathBuf,
}

impl CoderToolHandler {
    pub fn new(capabilities_root: &str, new_id: &str, cap_path: &Path) -> Self {
        let src_path = cap_path.join("src");
        let tests_path = cap_path.join("tests");
        Self {
            capabilities_root: capabilities_root.to_string(),
            cap_path: cap_path.to_path_buf(),
            new_id: new_id.to_string(),
            src_path,
            tests_path,
        }
    }

    /// Handle a tool call from the coder.
    pub fn handle(&self, tc: &ChatToolCall) -> Result<String> {
        let result = match tc.function.name.as_str() {
            "web_search" => self.handle_web_search(tc)?,
            "http_get" => self.handle_http_get(tc)?,
            "read_file" => self.handle_read_file(tc)?,
            "write_file" => self.handle_write_file(tc)?,
            "cargo_run" => self.handle_cargo_run(tc)?,
            "build" => self.handle_build()?,
            "test" => self.handle_test()?,
            "rustc_explain" => self.handle_rustc_explain(tc)?,
            other => ToolResult::err(format!("Unknown tool '{}'", other)),
        };

        match result {
            ToolResult::Continue(msg) => Ok(msg),
            ToolResult::Complete(_) => Ok("Complete".into()),
        }
    }

    fn handle_web_search(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            query: String,
        }
        let args: Args = serde_json::from_str(&tc.function.arguments)?;
        common::handle_web_search(&args.query)
    }

    fn handle_http_get(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            url: String,
        }
        let args: Args = serde_json::from_str(&tc.function.arguments)?;
        common::handle_http_get(&args.url)
    }

    fn handle_read_file(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
        }
        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "Invalid read_file args: {}. Required: {{\"path\": \"tests/integration.rs\"}}",
                    e
                )));
            }
        };
        // Coder can read from tests/ and src/
        let read_scopes = vec![self.tests_path.clone(), self.src_path.clone()];
        common::handle_read_file(&args.path, &self.cap_path, &self.new_id, &read_scopes)
    }

    fn handle_write_file(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }
        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "Invalid write_file args: {}. Required: {{\"path\": \"src/lib.rs\", \"content\": \"...\"}}",
                    e
                )));
            }
        };
        // Coder can write to src/ and tests/ (to fix broken tests if needed)
        let allowed_dirs = vec![self.src_path.clone(), self.tests_path.clone()];
        common::handle_write_file_multi_scope(
            &args.path,
            &args.content,
            &self.cap_path,
            &self.new_id,
            &allowed_dirs,
        )
    }

    fn handle_cargo_run(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            input: String,
        }
        let args: Args = serde_json::from_str(&tc.function.arguments)?;

        let workspace = Path::new(&self.capabilities_root);

        // Compile natively
        let compile = Command::new("cargo")
            .args(["build", "--release", "-p", &self.new_id])
            .current_dir(workspace)
            .output()?;

        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr);
            return Ok(ToolResult::err(format!("Build failed:\n{}", stderr)));
        }

        // Run binary
        let binary = workspace.join("target/release").join(&self.new_id);
        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(args.input.as_bytes());
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(ToolResult::ok(format!("SUCCESS:\n{}", stdout)))
        } else {
            Ok(ToolResult::err(format!(
                "FAILED:\nstdout: {}\nstderr: {}",
                stdout, stderr
            )))
        }
    }

    fn handle_build(&self) -> Result<ToolResult> {
        common::handle_build(&self.capabilities_root, &self.new_id)
    }

    fn handle_test(&self) -> Result<ToolResult> {
        let (success, output) = common::handle_test(&self.capabilities_root, &self.new_id)?;
        if success {
            Ok(ToolResult::ok(output))
        } else {
            Ok(ToolResult::err(output))
        }
    }

    fn handle_rustc_explain(&self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            error_code: String,
        }
        let args: Args = serde_json::from_str(&tc.function.arguments)?;
        common::handle_rustc_explain(&args.error_code)
    }
}
