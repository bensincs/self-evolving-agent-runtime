use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A simple plan describing a capability's IO contract and test cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPlan {
    pub capability_id: String,
    pub request_schema: Value,
    pub response_schema: Value,
    pub test_cases: Vec<TestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub input: Value,
    /// Expected fields (partial match); empty means "no error" assertion only.
    pub expect_contains: Value,
}

impl CapabilityPlan {
    pub fn to_markdown(&self) -> String {
        let mut md = format!("### Capability `{}`\n\n", self.capability_id);
        md.push_str("**Request schema**:\n\n");
        md.push_str(&format!(
            "```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&self.request_schema).unwrap()
        ));
        md.push_str("**Response schema**:\n\n");
        md.push_str(&format!(
            "```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&self.response_schema).unwrap()
        ));
        if !self.test_cases.is_empty() {
            md.push_str("**Test cases**:\n\n");
            for tc in &self.test_cases {
                md.push_str(&format!(
                    "- `{}`\n  - input: `{}`\n  - expect_contains: `{}`\n",
                    tc.name, tc.input, tc.expect_contains
                ));
            }
        }
        md
    }
}

/// Minimal scaffolding for the mutation agent to run deterministic tests.
#[derive(Debug, Clone)]
pub struct MutationAgent {
    pub capabilities_workspace: PathBuf,
}

impl MutationAgent {
    pub fn new(capabilities_workspace: impl Into<PathBuf>) -> Self {
        Self {
            capabilities_workspace: capabilities_workspace.into(),
        }
    }

    /// Run `cargo test -p <package>` inside the capabilities workspace.
    pub fn run_tests(&self, package: &str) -> anyhow::Result<()> {
        let status = Command::new("cargo")
            .arg("test")
            .arg("-p")
            .arg(package)
            .current_dir(&self.capabilities_workspace)
            .status()
            .with_context(|| format!("failed to spawn cargo test for {package}"))?;

        if !status.success() {
            anyhow::bail!("tests failed for {package}: {status}");
        }
        Ok(())
    }

    /// Placeholder: handoff to coding/testing agents (LLM-driven).
    /// In production, this would dispatch tasks to parallel agents.
    pub fn handoff_to_agents(&self, _plan: &CapabilityPlan) {
        // Intentionally left as a stub for now.
    }
}
