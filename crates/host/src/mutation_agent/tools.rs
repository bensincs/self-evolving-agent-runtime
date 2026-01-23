// crates/host/src/mutation_agent/tools.rs

//! Tool handlers for the mutation agent.
//!
//! Tools available to the LLM:
//! - web_search: Search the web for information (research APIs before coding)
//! - http_get: Make HTTP GET requests to explore API responses
//! - read_file: Read file contents
//! - write_file: Write to a file
//! - cargo_run: Quick native test (no WASM, no host functions)
//! - build: Compile the capability to WASM
//! - test: Run the WASM capability with test input (using runtime with host functions)
//! - complete: Signal completion

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;

use se_runtime_core::ai_client::ChatToolCall;
use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::types::{CapabilityRecord, CapabilityStatus};

/// Extract search result snippets from DuckDuckGo HTML.
fn extract_search_snippets(html: &str) -> Vec<String> {
    let mut snippets = Vec::new();

    // Look for result divs - DuckDuckGo uses class="result__snippet"
    for line in html.lines() {
        if line.contains("result__snippet") || line.contains("result__title") {
            // Basic HTML tag stripping
            let text: String = line
                .replace("<b>", "")
                .replace("</b>", "")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .chars()
                .filter(|c| !matches!(c, '<'..='>' | '\n' | '\r'))
                .collect::<String>()
                .split('<')
                .filter_map(|s| s.split('>').last())
                .collect::<Vec<_>>()
                .join(" ");

            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 20 {
                snippets.push(trimmed.to_string());
            }
        }
    }

    // Limit to first 10 results
    snippets.truncate(10);
    snippets
}

/// Tool definitions exposed to the LLM.
pub static TOOL_DEFINITIONS: Lazy<Vec<serde_json::Value>> = Lazy::new(|| {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for information. Use this to research APIs, documentation, or solutions BEFORE writing code. Essential for understanding external APIs (e.g., CoinGecko response format) before implementing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query. Be specific, e.g., 'CoinGecko API bitcoin price JSON response format'"
                        }
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "http_get",
                "description": "Make an HTTP GET request and see the response. Use this to explore API responses before writing code. Helps you understand the exact JSON structure an API returns.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to fetch"
                        }
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
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file."
                        }
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
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file."
                        },
                        "content": {
                            "type": "string",
                            "description": "The content to write."
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cargo_run",
                "description": "Quick test by running as native binary (not WASM). Faster iteration - use this to check logic before the full WASM build. Does NOT have host functions, so HTTP calls will fail, but good for testing parsing logic.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string",
                            "description": "JSON input to send via stdin. For testing parsing, you can mock API responses here."
                        }
                    },
                    "required": ["input"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "build",
                "description": "Compile the capability to WASM. Required before testing or completing.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "test",
                "description": "Test the compiled WASM capability by running it with sample input. Uses the full runtime with host functions (HTTP, time). IMPORTANT: The 'input' is what a USER would provide via stdin - NOT the expected HTTP response. For HTTP-based capabilities that need no user input, use {}.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string",
                            "description": "JSON input to send to the capability via STDIN. This is USER input, not mock API responses. For capabilities that fetch data via HTTP and need no user params, use '{}'."
                        }
                    },
                    "required": ["input"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "rustc_explain",
                "description": "Get detailed explanation of a Rust compiler error code. Use this when you see an error like 'E0502' or 'E0382' to understand how to fix it. Helps with borrow checker, ownership, and other Rust-specific errors.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "error_code": {
                            "type": "string",
                            "description": "The Rust error code (e.g., 'E0502', 'E0382', 'E0277'). Just the code, not the full error message."
                        }
                    },
                    "required": ["error_code"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "complete",
                "description": "Signal that the capability is finished. Only works after successful build AND test.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": "A one-line description of what the capability does."
                        },
                        "mark_parent_legacy": {
                            "type": "boolean",
                            "description": "Set to true if this capability REPLACES or IMPROVES the parent (marks parent as legacy). Set to false if this is just a new variant/derivative."
                        }
                    },
                    "required": ["summary"]
                }
            }
        }),
    ]
});

/// Completion arguments from the LLM.
#[derive(Debug, Deserialize, Serialize)]
pub struct CompletionArgs {
    pub summary: String,
    #[serde(default)]
    pub mark_parent_legacy: bool,
}

/// Handles tool calls from the mutation agent.
pub struct ToolHandler {
    capabilities_root: String,
    /// Tracks whether cargo build --release has succeeded
    pub build_succeeded: bool,
    /// Tracks whether the capability has been tested
    pub test_passed: bool,
    /// Tracks whether write_file has been called (code was actually written)
    pub code_written: bool,
    /// Tracks consecutive build failures to detect loops
    consecutive_build_failures: usize,
    /// Tracks consecutive test failures to detect loops
    consecutive_test_failures: usize,
    /// Last test error for context
    last_test_error: Option<String>,
}

impl ToolHandler {
    pub fn new(capabilities_root: String) -> Self {
        Self {
            capabilities_root,
            build_succeeded: false,
            test_passed: false,
            code_written: false,
            consecutive_build_failures: 0,
            consecutive_test_failures: 0,
            last_test_error: None,
        }
    }

    /// Reset state for a new mutation.
    pub fn reset(&mut self) {
        self.build_succeeded = false;
        self.test_passed = false;
        self.code_written = false;
        self.consecutive_build_failures = 0;
        self.consecutive_test_failures = 0;
        self.last_test_error = None;
    }

    /// Handle a tool call, returning the result string.
    pub fn handle(&mut self, tc: &ChatToolCall, new_id: &str) -> Result<String> {
        match tc.function.name.as_str() {
            "web_search" => self.handle_web_search(tc),
            "http_get" => self.handle_http_get(tc),
            "read_file" => self.handle_read_file(tc),
            "write_file" => self.handle_write_file(tc),
            "cargo_run" => self.handle_cargo_run(tc, new_id),
            "build" => self.handle_build(new_id),
            "test" => self.handle_test(tc, new_id),
            "rustc_explain" => self.handle_rustc_explain(tc),
            "complete" => self.handle_complete(tc),
            other => Ok(format!("ERROR: Unknown tool '{}'", other)),
        }
    }

    fn handle_web_search(&self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            query: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'query'. {}", e)),
        };

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ WEB SEARCH: {}", args.query);
        println!("╚══════════════════════════════════════════════════════════════════╝");

        // Use DuckDuckGo HTML search (no API key needed)
        let encoded_query = urlencoding::encode(&args.query);
        let url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);

        let client = match reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; CapabilityAgent/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(format!("ERROR: Failed to create HTTP client: {}", e)),
        };

        match client.get(&url).send() {
            Ok(response) => {
                if !response.status().is_success() {
                    return Ok(format!(
                        "ERROR: Search returned status {}",
                        response.status()
                    ));
                }
                match response.text() {
                    Ok(html) => {
                        // Extract text snippets from DuckDuckGo HTML results
                        let snippets = extract_search_snippets(&html);
                        println!("Found {} results", snippets.len());
                        if snippets.is_empty() {
                            Ok("No search results found. Try a different query.".to_string())
                        } else {
                            Ok(format!(
                                "Search results for '{}':\n\n{}",
                                args.query,
                                snippets.join("\n\n---\n\n")
                            ))
                        }
                    }
                    Err(e) => Ok(format!("ERROR: Failed to read response: {}", e)),
                }
            }
            Err(e) => Ok(format!("ERROR: Search request failed: {}", e)),
        }
    }

    fn handle_http_get(&self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            url: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'url'. {}", e)),
        };

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ HTTP GET: {}", args.url);
        println!("╚══════════════════════════════════════════════════════════════════╝");

        let client = match reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; CapabilityAgent/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(format!("ERROR: Failed to create HTTP client: {}", e)),
        };

        match client.get(&args.url).send() {
            Ok(response) => {
                let status = response.status();
                match response.text() {
                    Ok(body) => {
                        // Truncate if too long
                        let truncated = if body.len() > 4000 {
                            format!(
                                "{}...\n\n[TRUNCATED - {} bytes total]",
                                &body[..4000],
                                body.len()
                            )
                        } else {
                            body
                        };
                        println!("Response (status {}):\n{}", status, truncated);
                        Ok(format!("HTTP {} - Response:\n{}", status, truncated))
                    }
                    Err(e) => Ok(format!("ERROR: Failed to read response body: {}", e)),
                }
            }
            Err(e) => Ok(format!("ERROR: Request failed: {}", e)),
        }
    }

    fn handle_read_file(&self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'path'. {}", e)),
        };

        println!("[TOOL] read_file: {}", args.path);

        match fs::read_to_string(&args.path) {
            Ok(content) => Ok(content),
            Err(e) => Ok(format!("ERROR: {}", e)),
        }
    }

    fn handle_write_file(&mut self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'path' and 'content'. {}",
                    e
                ))
            }
        };

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ WRITE FILE: {}", args.path);
        println!("╠══════════════════════════════════════════════════════════════════╣");
        // Print content with line numbers
        for (i, line) in args.content.lines().enumerate() {
            println!("║ {:3} │ {}", i + 1, line);
        }
        println!("╚══════════════════════════════════════════════════════════════════╝\n");

        // Reset validation state since code has changed
        self.build_succeeded = false;
        self.test_passed = false;
        self.code_written = true;  // Mark that we actually wrote code

        // Create parent directories if needed
        if let Some(parent) = Path::new(&args.path).parent() {
            fs::create_dir_all(parent)?;
        }

        match fs::write(&args.path, &args.content) {
            Ok(()) => Ok(format!(
                "OK: Wrote {} bytes to {}",
                args.content.len(),
                args.path
            )),
            Err(e) => Ok(format!("ERROR: {}", e)),
        }
    }

    fn handle_cargo_run(&self, tc: &ChatToolCall, new_id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            input: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'input' (JSON string). {}",
                    e
                ))
            }
        };

        let workspace_root = Path::new(&self.capabilities_root);

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ CARGO RUN (native, no WASM): {}", new_id);
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║ Input (stdin): {}", args.input);
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║ NOTE: HTTP calls will FAIL in this mode - use for testing logic only");
        println!("╚══════════════════════════════════════════════════════════════════╝");

        // First compile natively (not WASM)
        let compile = Command::new("cargo")
            .args(["build", "--release", "-p", new_id])
            .current_dir(workspace_root)
            .output()
            .context("failed to run cargo build")?;

        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr);
            println!("┌─ Compile Error ──────────────────────────────────────────────────┐");
            println!("{}", stderr);
            println!("└───────────────────────────────────────────────────────────────────┘\n");
            return Ok(format!("ERROR: Native build failed:\n{}", stderr));
        }

        // Run the binary with input
        let binary_path = workspace_root.join("target/release").join(new_id);

        let mut child = Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn binary")?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(args.input.as_bytes());
        }

        let output = child
            .wait_with_output()
            .context("failed to wait for binary")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            println!("┌─ Output ─────────────────────────────────────────────────────────┐");
            println!("{}", stdout);
            if !stderr.is_empty() {
                println!("┌─ Stderr ─────────────────────────────────────────────────────────┐");
                println!("{}", stderr);
            }
            println!("└─ CARGO RUN SUCCESS ─────────────────────────────────────────────┘\n");
            Ok(format!(
                "SUCCESS (native run):\nInput: {}\nOutput:\n{}\n\nNote: This was a native build. HTTP calls would have failed. Now run 'build' for WASM and 'test' with the real runtime.",
                args.input, stdout
            ))
        } else {
            println!("┌─ Error ──────────────────────────────────────────────────────────┐");
            if !stdout.is_empty() {
                println!("stdout: {}", stdout);
            }
            println!("stderr: {}", stderr);
            println!("└─ CARGO RUN FAILED ───────────────────────────────────────────────┘\n");

            let mut result = format!(
                "CARGO RUN FAILED:\nInput: {}\nstdout: {}\nstderr: {}",
                args.input, stdout, stderr
            );

            if stderr.contains("not linked")
                || stderr.contains("undefined")
                || stderr.contains("host")
            {
                result.push_str("\n\nNOTE: If you see 'undefined' or 'not linked' errors about host functions, that's expected - HTTP and time functions only work in WASM mode. Focus on fixing any logic/parsing errors first.");
            }

            Ok(result)
        }
    }

    fn handle_build(&mut self, new_id: &str) -> Result<String> {
        // Check if write_file was called first
        if !self.code_written {
            return Ok(
                "ERROR: You must call write_file to save your code BEFORE calling build!\n\n\
                The current src/main.rs is just a copy of the parent capability.\n\
                You need to:\n\
                1. Call write_file with path=<capability_path>/src/main.rs and your new code\n\
                2. Then call build\n\n\
                DO NOT just print code in a markdown block - that does nothing.\n\
                You MUST call the write_file tool to actually save the file."
                    .to_string(),
            );
        }

        let workspace_root = Path::new(&self.capabilities_root);

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!(
            "║ BUILD: cargo build --release --target wasm32-wasip1 -p {}",
            new_id
        );
        println!("╚══════════════════════════════════════════════════════════════════╝");

        let output = Command::new("cargo")
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-wasip1",
                "-p",
                new_id,
            ])
            .current_dir(workspace_root)
            .output()
            .context("failed to run cargo build")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Always print the build output to console
        if !stdout.is_empty() {
            println!("┌─ stdout ─────────────────────────────────────────────────────────┐");
            println!("{}", stdout);
        }
        if !stderr.is_empty() {
            println!("┌─ stderr ─────────────────────────────────────────────────────────┐");
            println!("{}", stderr);
        }

        if output.status.success() {
            self.build_succeeded = true;
            self.consecutive_build_failures = 0;
            let wasm_path = workspace_root
                .join("target/wasm32-wasip1/release")
                .join(format!("{}.wasm", new_id));
            println!("└─ BUILD SUCCESS ──────────────────────────────────────────────────┘\n");
            Ok(format!(
                "OK: Build successful! WASM at: {}\n{}",
                wasm_path.display(),
                stderr
            ))
        } else {
            self.build_succeeded = false;
            self.consecutive_build_failures += 1;
            println!(
                "└─ BUILD FAILED (attempt {}) ─────────────────────────────────────┘\n",
                self.consecutive_build_failures
            );

            if self.consecutive_build_failures >= 3 {
                Ok(format!(
                    "ERROR: Build failed {} times in a row. You may be trying to use a dependency that isn't WASM-compatible.\n\
                    REMINDER: Use only WASM-compatible deps. For HTTP, use capability_common::http_get_json().\n\
                    Available: serde, serde_json, regex, base64, url + capability_common (has http_get_*, time functions).\n\n\
                    Build error:\n{}\n{}",
                    self.consecutive_build_failures,
                    stdout,
                    stderr
                ))
            } else {
                // Check for Rust error codes and suggest rustc_explain
                let combined = format!("{}\n{}", stdout, stderr);
                let mut error_hint = String::new();

                // Look for error codes like E0502, E0382, etc.
                let re = regex::Regex::new(r"\[E(\d{4})\]").ok();
                if let Some(re) = re {
                    let error_codes: Vec<String> = re
                        .captures_iter(&combined)
                        .filter_map(|cap| cap.get(1).map(|m| format!("E{}", m.as_str())))
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    if !error_codes.is_empty() {
                        error_hint = format!(
                            "\n\n━━━ HINT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                            Found Rust error code(s): {}\n\
                            Use the 'rustc_explain' tool with the error code to understand how to fix it.\n\
                            Example: rustc_explain(\"{}\")",
                            error_codes.join(", "),
                            error_codes.first().unwrap_or(&"E0502".to_string())
                        );
                    }
                }

                Ok(format!(
                    "ERROR: Build failed:\n{}\n{}{}",
                    stdout, stderr, error_hint
                ))
            }
        }
    }

    fn handle_test(&mut self, tc: &ChatToolCall, new_id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            input: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'input' (JSON string). {}",
                    e
                ))
            }
        };

        // Check if we need to rebuild first
        if !self.build_succeeded {
            return Ok(
                "ERROR: Code has changed since last build. Run 'build' first to compile your changes."
                    .to_string(),
            );
        }

        let wasm_path = Path::new(&self.capabilities_root)
            .join("target/wasm32-wasip1/release")
            .join(format!("{}.wasm", new_id));

        if !wasm_path.exists() {
            return Ok("ERROR: WASM file not found. Run 'build' first.".to_string());
        }

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ TEST: {}", new_id);
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║ Input (stdin): {}", args.input);
        println!("╚══════════════════════════════════════════════════════════════════╝");

        // Use the CapabilityRunner which has host functions
        let runner = match CapabilityRunner::new(&self.capabilities_root) {
            Ok(r) => r,
            Err(e) => return Ok(format!("ERROR: Failed to create runner: {}", e)),
        };

        let cap = CapabilityRecord {
            id: new_id.to_string(),
            summary: "test".to_string(),
            embedding: None,
            binary: Some(format!(
                "../../target/wasm32-wasip1/release/{}.wasm",
                new_id
            )),
            status: CapabilityStatus::Active,
            replaced_by: None,
        };

        match runner.run_capability(&cap, &args.input) {
            Ok(output) => {
                self.test_passed = true;
                self.consecutive_test_failures = 0;
                self.last_test_error = None;
                println!("┌─ Output ─────────────────────────────────────────────────────────┐");
                println!("{}", output);
                println!("└─ TEST SUCCESS ─────────────────────────────────────────────────┘\n");

                // Check if this looks like an UPDATE task but output doesn't reflect the update
                let mut warning = String::new();
                let id_lower = new_id.to_lowercase();
                let is_update_task = id_lower.contains("update") || id_lower.contains("set") || id_lower.contains("modify");

                if is_update_task {
                    // Try to detect if input values appear in output (they should for an update)
                    if let Ok(input_json) = serde_json::from_str::<serde_json::Value>(&args.input) {
                        if let Ok(output_json) = serde_json::from_str::<serde_json::Value>(&output) {
                            // Check if key input values are reflected in output
                            let mut missing_updates = Vec::new();

                            if let Some(input_obj) = input_json.as_object() {
                                for (key, input_val) in input_obj {
                                    // Skip employee_id, it's just an identifier
                                    if key == "employee_id" {
                                        continue;
                                    }

                                    // Check if this input value appears anywhere in output
                                    let input_str = input_val.to_string();
                                    let output_str = output_json.to_string();

                                    if !output_str.contains(&input_str.trim_matches('"')) {
                                        missing_updates.push(format!("{}: {}", key, input_val));
                                    }
                                }
                            }

                            if !missing_updates.is_empty() {
                                warning = format!(
                                    "\n\n⚠️  WARNING: UPDATE TASK OUTPUT CHECK ⚠️\n\
                                    This looks like an UPDATE capability, but the output doesn't contain your input values:\n\
                                    Missing from output: {}\n\n\
                                    If the output shows OLD values instead of NEW values, your code is NOT actually updating!\n\
                                    Common mistakes:\n\
                                    1. You copied a 'get' capability but didn't modify it to update\n\
                                    2. You're using find_employee() instead of find_employee_mut()\n\
                                    3. You forgot to call db.save() after making changes\n\n\
                                    Please verify your code actually updates the data, not just reads it.",
                                    missing_updates.join(", ")
                                );
                            }
                        }
                    }
                }

                Ok(format!(
                    "SUCCESS: Test passed!\n\
                    Input provided via stdin: {}\n\
                    Output from capability:\n{}{}",
                    args.input, output, warning
                ))
            }
            Err(e) => {
                self.test_passed = false;
                self.consecutive_test_failures += 1;
                let error_str = e.to_string();

                println!("┌─ Error ──────────────────────────────────────────────────────────┐");
                println!("{}", error_str);
                println!(
                    "└─ TEST FAILED (attempt {}) ───────────────────────────────────────┘\n",
                    self.consecutive_test_failures
                );

                // Build detailed error context
                let mut result = format!(
                    "TEST FAILED (attempt {}):\n\
                    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                    Input provided via STDIN: {}\n\
                    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                    Error: {}\n",
                    self.consecutive_test_failures, args.input, error_str
                );

                // Add context-specific hints based on error
                if error_str.contains("parse")
                    || error_str.contains("JSON")
                    || error_str.contains("deserialize")
                {
                    result.push_str("\n━━━ HINT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                    result.push_str("JSON parsing failed. Possible causes:\n");
                    result.push_str(
                        "1. Your code's input struct doesn't match what you're passing to stdin\n",
                    );
                    result.push_str("2. For HTTP capabilities: The test input should be YOUR INPUT SCHEMA (or {}),\n");
                    result.push_str("   NOT the expected HTTP response. The http_get_* functions fetch data at runtime.\n");
                    result.push_str("3. Check that your deserialization struct matches the actual data format.\n");

                    // If input looks like API response data, be more specific
                    if args.input.contains("bitcoin")
                        || args.input.contains("price")
                        || args.input.contains("usd")
                    {
                        result.push_str(
                            "\n⚠️  Your test input looks like it might be API response data.\n",
                        );
                        result.push_str(
                            "   The test INPUT should be what the USER provides (often just {}),\n",
                        );
                        result.push_str("   not what you expect from an HTTP API call.\n");
                    }
                }

                if error_str.contains("HTTP")
                    || error_str.contains("network")
                    || error_str.contains("connection")
                {
                    result.push_str("\n━━━ HINT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                    result.push_str("Network error occurred. Check:\n");
                    result.push_str("1. The URL is correct and accessible\n");
                    result.push_str("2. The API endpoint exists and is responding\n");
                }

                // Check if same error is repeating
                if let Some(ref last_err) = self.last_test_error {
                    if last_err == &error_str {
                        result.push_str("\n━━━ WARNING ━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                        result.push_str("⚠️  Same error as last attempt! You may be:\n");
                        result.push_str("1. Not changing anything between test runs\n");
                        result.push_str("2. Misunderstanding the root cause of the error\n");
                        result.push_str(
                            "3. Changing the test input when you should change the code\n",
                        );
                        result.push_str(
                            "\nTry a different approach or re-read the error message carefully.\n",
                        );
                    }
                }

                // Provide escalating guidance for repeated failures
                if self.consecutive_test_failures >= 3 {
                    result.push_str("\n━━━ LOOP DETECTION ━━━━━━━━━━━━━━━━━━━━━\n");
                    result.push_str(&format!(
                        "⚠️  {} consecutive test failures!\n",
                        self.consecutive_test_failures
                    ));
                    result.push_str("STOP and think carefully about the problem:\n");
                    result.push_str(
                        "• What data format does your code EXPECT to receive via stdin?\n",
                    );
                    result.push_str(
                        "• What data format do you actually send in the test 'input' field?\n",
                    );
                    result.push_str(
                        "• For HTTP capabilities: stdin input is NOT the HTTP response!\n",
                    );
                    result.push_str("• Re-read capability_common documentation above.\n");
                }

                self.last_test_error = Some(error_str);
                Ok(result)
            }
        }
    }

    fn handle_rustc_explain(&self, tc: &ChatToolCall) -> Result<String> {
        #[derive(Deserialize)]
        struct Args {
            error_code: String,
        }

        let args: Args = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Invalid arguments. Need 'error_code'. {}",
                    e
                ))
            }
        };

        // Normalize the error code (ensure it starts with E if it's just a number)
        let code = if args.error_code.starts_with('E') || args.error_code.starts_with('e') {
            args.error_code.to_uppercase()
        } else {
            format!("E{}", args.error_code)
        };

        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║ RUSTC EXPLAIN: {}", code);
        println!("╚══════════════════════════════════════════════════════════════════╝");

        let output = Command::new("rustc")
            .args(["--explain", &code])
            .output()
            .context("failed to run rustc --explain")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() && !stdout.is_empty() {
            println!("{}", stdout);
            println!("└─ EXPLANATION END ─────────────────────────────────────────────────┘\n");
            Ok(format!(
                "Explanation of Rust error {}:\n\n{}\n\n━━━ HOW TO FIX ━━━\nUse this explanation to understand why your code doesn't compile and restructure it accordingly.",
                code, stdout
            ))
        } else {
            let error_msg = if !stderr.is_empty() {
                stderr.to_string()
            } else {
                format!("Unknown error code: {}", code)
            };
            println!("ERROR: {}", error_msg);
            Ok(format!(
                "ERROR: Could not explain error code '{}'. {}",
                code, error_msg
            ))
        }
    }

    fn handle_complete(&self, tc: &ChatToolCall) -> Result<String> {
        let args: CompletionArgs = match serde_json::from_str(&tc.function.arguments) {
            Ok(a) => a,
            Err(e) => return Ok(format!("ERROR: Invalid arguments. Need 'summary'. {}", e)),
        };

        // Verify build succeeded
        if !self.build_succeeded {
            return Ok(
                "ERROR: Cannot complete - build has not succeeded. Run 'build' first and fix any errors."
                    .to_string(),
            );
        }

        // Verify test passed
        if !self.test_passed {
            return Ok(
                "ERROR: Cannot complete - test has not passed. Run 'test' with appropriate input and verify the output is correct for the task before completing."
                    .to_string(),
            );
        }

        println!("[TOOL] complete: {}", args.summary);

        Ok(format!("Mutation complete! Summary: {}", args.summary))
    }
}
