// crates/host/src/agents/coder/tool_defs.rs

//! Tool definitions for the Coder agent.

use serde_json::json;

/// Coder agent tool definitions.
pub fn coder_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for information. Use this to research APIs, documentation, or solutions BEFORE writing code.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The search query" }
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "http_get",
                "description": "Make an HTTP GET request to explore API responses.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to fetch" }
                    },
                    "required": ["url"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file.",
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
                "description": "Write content to a file. Creates directories if needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file." },
                        "content": { "type": "string", "description": "The content to write." }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cargo_run",
                "description": "Quick test by running as native binary (not WASM). HTTP calls will fail, but good for testing parsing logic.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "JSON input to send via stdin." }
                    },
                    "required": ["input"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "build",
                "description": "Compile the capability to WASM. Required before testing.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "test",
                "description": "Run cargo test -p <capability> to execute unit tests.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "rustc_explain",
                "description": "Get detailed explanation of a Rust compiler error code (e.g., E0502).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "error_code": { "type": "string", "description": "The Rust error code (e.g., 'E0502')." }
                    },
                    "required": ["error_code"]
                }
            }
        }),
    ]
}
