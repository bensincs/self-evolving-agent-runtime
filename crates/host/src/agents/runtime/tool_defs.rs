// crates/host/src/agents/runtime/tool_defs.rs

//! Tool definitions for the Runtime agent.

use serde_json::json;

/// Runtime agent tool definitions.
pub fn runtime_tool_definitions() -> Vec<serde_json::Value> {
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
