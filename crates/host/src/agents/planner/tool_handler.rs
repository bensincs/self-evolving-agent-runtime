// crates/host/src/agents/planner/tool_handler.rs

//! Tool handlers for the Planner agent.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use se_runtime_core::ai_client::{AiClient, ChatToolCall};

use super::super::capability_ops::CapabilityOps;
use super::super::common::{self, CompletionArgs};
use super::super::{coder, tester, MutationResult};

/// Result from planner tool execution.
pub enum PlannerResult {
    Continue(String),
    Complete(MutationResult),
}

/// Tool handler for the Planner agent.
pub struct PlannerToolHandler<'a, C: AiClient + Sync> {
    client: &'a C,
    capabilities_root: String,
    cap_path: PathBuf,
    new_id: String,
    parent_id: String,
    task: String,
    max_steps: usize,
    tests_passed: bool,
}

impl<'a, C: AiClient + Sync> PlannerToolHandler<'a, C> {
    pub fn new(
        client: &'a C,
        capabilities_root: &str,
        new_id: &str,
        parent_id: &str,
        task: &str,
        max_steps: usize,
    ) -> Self {
        let cap_path = Path::new(capabilities_root).join("crates").join(new_id);
        Self {
            client,
            capabilities_root: capabilities_root.to_string(),
            cap_path,
            new_id: new_id.to_string(),
            parent_id: parent_id.to_string(),
            task: task.to_string(),
            max_steps,
            tests_passed: false,
        }
    }

    /// Handle a tool call from the planner.
    pub fn handle(&mut self, tc: &ChatToolCall) -> Result<PlannerResult> {
        match tc.function.name.as_str() {
            "write_plan" => self.handle_write_plan(tc),
            "read_plan" => self.handle_read_plan(),
            "start_coder_agent" => self.handle_start_coder(),
            "start_tester_agent" => self.handle_start_tester(),
            "test" => self.handle_test(),
            "complete" => self.handle_complete(tc),
            other => Ok(PlannerResult::Continue(format!(
                "ERROR: Unknown tool '{}'",
                other
            ))),
        }
    }

    fn handle_write_plan(&self, tc: &ChatToolCall) -> Result<PlannerResult> {
        #[derive(Deserialize)]
        struct Args {
            content: String,
        }
        let args: Args = serde_json::from_str(&tc.function.arguments)?;
        let plan_path = self.cap_path.join("PLAN.md");
        fs::write(&plan_path, args.content)?;
        Ok(PlannerResult::Continue("Wrote PLAN.md".into()))
    }

    fn handle_read_plan(&self) -> Result<PlannerResult> {
        let plan_path = self.cap_path.join("PLAN.md");
        let content = fs::read_to_string(&plan_path).unwrap_or_else(|_| "NOT_FOUND".into());
        Ok(PlannerResult::Continue(content))
    }

    fn handle_start_coder(&mut self) -> Result<PlannerResult> {
        let main_rs = fs::read_to_string(self.cap_path.join("src/main.rs"))?;

        coder::run_coder_agent(
            self.client,
            &self.capabilities_root,
            &self.new_id,
            &self.cap_path,
            &main_rs,
            &self.task,
            self.max_steps,
        )?;

        Ok(PlannerResult::Continue("Coder finished".into()))
    }

    fn handle_start_tester(&mut self) -> Result<PlannerResult> {
        tester::run_tester_agent(
            self.client,
            &self.capabilities_root,
            &self.new_id,
            &self.cap_path,
            30,
        )?;

        Ok(PlannerResult::Continue("Tester finished".into()))
    }

    fn handle_test(&mut self) -> Result<PlannerResult> {
        let (success, output) = common::handle_test(&self.capabilities_root, &self.new_id)?;
        self.tests_passed = success;

        if success {
            Ok(PlannerResult::Continue(output))
        } else {
            Ok(PlannerResult::Continue(format!("ERROR: {}", output)))
        }
    }

    fn handle_complete(&self, tc: &ChatToolCall) -> Result<PlannerResult> {
        let args: CompletionArgs = serde_json::from_str(&tc.function.arguments)?;

        if !self.tests_passed {
            return Ok(PlannerResult::Continue(
                "ERROR: Tests have not passed.".into(),
            ));
        }

        let cap_ops = CapabilityOps::new(&self.capabilities_root);
        cap_ops.update_meta_json(&self.new_id, &args.summary)?;

        if args.mark_parent_legacy {
            let _ = cap_ops.mark_as_legacy(&self.parent_id, &self.new_id);
        }

        Ok(PlannerResult::Complete(MutationResult {
            capability_id: self.new_id.clone(),
            summary: args.summary,
        }))
    }
}
