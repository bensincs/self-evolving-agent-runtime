// crates/host/src/agents/planner/tool_defs.rs

//! Tool definitions for the Planner agent.

use serde_json::json;

/// Planner agent tool definitions.
pub fn planner_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "write_plan",
                "description": "Write PLAN.md - markdown describing what to build.",
                "parameters": {
                    "type": "object",
                    "properties": { "content": { "type": "string" } },
                    "required": ["content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_plan",
                "description": "Read PLAN.md content.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "start_tester_agent",
                "description": "Tester reads PLAN.md and writes tests.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "start_coder_agent",
                "description": "Coder reads PLAN.md + tests and implements.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "test",
                "description": "Run cargo test.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "complete",
                "description": "Finish when tests pass.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "summary": { "type": "string" }
                    },
                    "required": ["summary"]
                }
            }
        }),
    ]
}
