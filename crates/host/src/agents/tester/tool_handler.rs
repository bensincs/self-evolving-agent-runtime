// crates/host/src/agents/tester/tool_handler.rs

//! Tool handlers for the Tester agent.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use se_runtime_core::ai_client::ChatToolCall;

use super::super::common::{self, ToolResult};

/// Tool handler for the Tester agent.
pub struct TesterToolHandler {
    #[allow(dead_code)]
    capabilities_root: String,
    cap_path: PathBuf,
    new_id: String,
    tests_path: PathBuf,
    src_path: PathBuf,
}

impl TesterToolHandler {
    pub fn new(capabilities_root: &str, new_id: &str, cap_path: &Path) -> Self {
        let tests_path = cap_path.join("tests");
        let src_path = cap_path.join("src");
        Self {
            capabilities_root: capabilities_root.to_string(),
            cap_path: cap_path.to_path_buf(),
            new_id: new_id.to_string(),
            tests_path,
            src_path,
        }
    }

    /// Handle a tool call from the tester.
    pub fn handle(&mut self, tc: &ChatToolCall) -> Result<String> {
        let result = match tc.function.name.as_str() {
            "read_file" => self.handle_read_file(tc)?,
            "write_file" => self.handle_write_file(tc)?,
            "build" => self.handle_build()?,
            other => ToolResult::err(format!("Unknown tool '{}'", other)),
        };

        match result {
            ToolResult::Continue(msg) => Ok(msg),
            ToolResult::Complete(_) => Ok("Complete".into()),
        }
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
        // Tester can read from tests/ and src/ (src is restored after tester finishes)
        let read_scopes = vec![self.tests_path.clone(), self.src_path.clone()];
        common::handle_read_file(&args.path, &self.cap_path, &self.new_id, &read_scopes)
    }

    fn handle_write_file(&mut self, tc: &ChatToolCall) -> Result<ToolResult> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }
        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "Invalid write_file args: {}. Required: {{\"path\": \"tests/integration.rs\", \"content\": \"...\"}}",
                    e
                )));
            }
        };
        // Tester can write to tests/ and src/ (src is restored after tester finishes)
        let write_scopes = vec![self.tests_path.clone(), self.src_path.clone()];
        common::handle_write_file_multi_scope(
            &args.path,
            &args.content,
            &self.cap_path,
            &self.new_id,
            &write_scopes,
        )
    }

    fn handle_build(&self) -> Result<ToolResult> {
        common::handle_build_tests(&self.capabilities_root, &self.new_id)
    }
}
