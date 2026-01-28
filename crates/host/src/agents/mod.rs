// crates/host/src/agents/mod.rs

//! Agent modules for the self-evolving runtime.
//!
//! Each agent is self-contained with its own:
//! - mod.rs (agent loop)
//! - prompts.rs (system prompts)
//! - tool_defs.rs (tool definitions)
//! - tool_handler.rs (tool execution)

pub mod capability_ops;
pub mod common;
pub mod log;
pub mod prompt_utils;

pub mod coder;
pub mod planner;
pub mod runtime;
pub mod tester;

// Re-export main entry points
pub use runtime::Agent;

/// Result of a successful mutation.
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub capability_id: String,
    pub summary: String,
}
