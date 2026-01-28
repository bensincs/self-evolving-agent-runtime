// crates/host/src/agents/tester/tool_defs.rs

//! Tool definitions for the Tester agent.

use serde_json::json;

/// Tester agent tool definitions.
pub fn tester_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file to review code.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file." }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write test files to the tests/ directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the test file." },
                        "content": { "type": "string", "description": "The test code to write." }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "build",
                "description": "Compile the tests to check for syntax errors. Tests will fail (no implementation yet) but must compile.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
    ]
}
