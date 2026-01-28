// crates/host/src/agents/common.rs

//! Common types and utilities shared across all agents.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;

use super::MutationResult;

/// Result from tool execution.
#[derive(Debug)]
#[allow(dead_code)]
pub enum ToolResult {
    /// Continue the agent loop with this message
    Continue(String),
    /// Mutation is complete
    Complete(MutationResult),
}

impl ToolResult {
    /// Create a continue result from a string.
    pub fn ok(msg: impl Into<String>) -> Self {
        Self::Continue(msg.into())
    }

    /// Create an error continue result.
    pub fn err(msg: impl Into<String>) -> Self {
        Self::Continue(format!("ERROR: {}", msg.into()))
    }
}

/// Arguments for the complete tool.
#[derive(Debug, Deserialize)]
pub struct CompletionArgs {
    pub summary: String,
    #[serde(default)]
    pub mark_parent_legacy: bool,
}

/// Normalize a path provided by an agent.
/// Handles: absolute paths, relative to cap_path, and paths containing capability id.
pub fn normalize_path(path_str: &str, cap_path: &Path, new_id: &str) -> PathBuf {
    let path = Path::new(path_str);

    // Already absolute
    if path.is_absolute() {
        return path.to_path_buf();
    }

    // If path starts with src/ or tests/, join with cap_path
    if path_str.starts_with("src/") || path_str.starts_with("tests/") {
        return cap_path.join(path);
    }

    // If path contains the capability id, extract the part after it
    if let Some(idx) = path_str.find(new_id) {
        let after_id = &path_str[idx + new_id.len()..];
        let trimmed = after_id.trim_start_matches('/');
        if !trimmed.is_empty() {
            return cap_path.join(trimmed);
        }
    }

    // Default: join with cap_path
    cap_path.join(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared tool implementations
// ─────────────────────────────────────────────────────────────────────────────

/// Handle web_search tool.
pub fn handle_web_search(query: &str) -> Result<ToolResult> {
    let encoded = urlencoding::encode(query);
    let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; CapabilityAgent/1.0)")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(&url).send() {
        Ok(resp) if resp.status().is_success() => {
            let html = resp.text()?;
            let snippets = extract_search_snippets(&html);
            if snippets.is_empty() {
                Ok(ToolResult::ok("No results found."))
            } else {
                Ok(ToolResult::ok(format!(
                    "Results:\n{}",
                    snippets.join("\n---\n")
                )))
            }
        }
        Ok(resp) => Ok(ToolResult::err(format!("HTTP {}", resp.status()))),
        Err(e) => Ok(ToolResult::err(e.to_string())),
    }
}

/// Handle http_get tool.
pub fn handle_http_get(url: &str) -> Result<ToolResult> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; CapabilityAgent/1.0)")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(url).send() {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            let truncated = if body.len() > 4000 {
                format!("{}...[truncated]", &body[..4000])
            } else {
                body
            };
            Ok(ToolResult::ok(format!("HTTP {} - {}", status, truncated)))
        }
        Err(e) => Ok(ToolResult::err(e.to_string())),
    }
}

/// Handle read_file tool with scope checking.
pub fn handle_read_file(
    path_str: &str,
    cap_path: &Path,
    new_id: &str,
    read_scopes: &[PathBuf],
) -> Result<ToolResult> {
    let path = normalize_path(path_str, cap_path, new_id);

    // Check read scopes
    if !read_scopes.is_empty() {
        let allowed = read_scopes.iter().any(|scope| path.starts_with(scope));
        if !allowed {
            let scopes: Vec<_> = read_scopes
                .iter()
                .map(|s| s.display().to_string())
                .collect();
            return Ok(ToolResult::err(format!(
                "Read access denied. Allowed: {:?}",
                scopes
            )));
        }
    }

    match fs::read_to_string(&path) {
        Ok(content) => Ok(ToolResult::ok(content)),
        Err(e) => Ok(ToolResult::err(e.to_string())),
    }
}

/// Handle write_file tool with scope checking.
pub fn handle_write_file(
    path_str: &str,
    content: &str,
    cap_path: &Path,
    new_id: &str,
    write_scope: Option<&PathBuf>,
) -> Result<ToolResult> {
    let path = normalize_path(path_str, cap_path, new_id);

    // Check write scope
    if let Some(scope) = write_scope {
        if !path.starts_with(scope) {
            return Ok(ToolResult::err(format!(
                "Write access denied. Only {} is allowed.",
                scope.display()
            )));
        }
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    match fs::write(&path, content) {
        Ok(()) => Ok(ToolResult::ok(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path.display()
        ))),
        Err(e) => Ok(ToolResult::err(e.to_string())),
    }
}

/// Handle write_file tool with multiple allowed scopes.
pub fn handle_write_file_multi_scope(
    path_str: &str,
    content: &str,
    cap_path: &Path,
    new_id: &str,
    write_scopes: &[PathBuf],
) -> Result<ToolResult> {
    let path = normalize_path(path_str, cap_path, new_id);

    // Check if path is in any allowed scope
    if !write_scopes.is_empty() {
        let allowed = write_scopes.iter().any(|scope| path.starts_with(scope));
        if !allowed {
            let scopes: Vec<_> = write_scopes
                .iter()
                .map(|s| s.display().to_string())
                .collect();
            return Ok(ToolResult::err(format!(
                "Write access denied. Allowed: {:?}",
                scopes
            )));
        }
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    match fs::write(&path, content) {
        Ok(()) => Ok(ToolResult::ok(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path.display()
        ))),
        Err(e) => Ok(ToolResult::err(e.to_string())),
    }
}

/// Handle build tool for WASM compilation.
pub fn handle_build(capabilities_root: &str, new_id: &str) -> Result<ToolResult> {
    let workspace = Path::new(capabilities_root);
    let output = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "-p",
            new_id,
        ])
        .current_dir(workspace)
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(ToolResult::ok(format!("Build succeeded\n{}", stderr)))
    } else {
        Ok(ToolResult::err(format!("Build failed:\n{}", stderr)))
    }
}

/// Handle build tool for tests (native, not WASM).
pub fn handle_build_tests(capabilities_root: &str, new_id: &str) -> Result<ToolResult> {
    let output = Command::new("cargo")
        .args(["build", "--tests", "-p", new_id])
        .current_dir(capabilities_root)
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(ToolResult::ok("Tests compile successfully."))
    } else {
        Ok(ToolResult::err(format!(
            "Tests failed to compile:\n{}",
            stderr
        )))
    }
}

/// Handle test tool.
pub fn handle_test(capabilities_root: &str, new_id: &str) -> Result<(bool, String)> {
    let output = Command::new("cargo")
        .args(["test", "-p", new_id, "--", "--nocapture"])
        .current_dir(capabilities_root)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok((true, format!("Tests passed\n{}", stdout)))
    } else {
        // Return full output so LLM can see exactly what failed
        let full_output = format!(
            "Tests failed!\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
            stdout, stderr
        );
        Ok((false, full_output))
    }
}

/// Handle rustc_explain tool.
pub fn handle_rustc_explain(error_code: &str) -> Result<ToolResult> {
    let code = if error_code.starts_with('E') || error_code.starts_with('e') {
        error_code.to_uppercase()
    } else {
        format!("E{}", error_code)
    };

    let output = Command::new("rustc").args(["--explain", &code]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && !stdout.is_empty() {
        Ok(ToolResult::ok(format!(
            "Explanation of {}:\n{}",
            code, stdout
        )))
    } else {
        Ok(ToolResult::err(format!("Unknown error code '{}'", code)))
    }
}

/// Extract text snippets from DuckDuckGo HTML results.
fn extract_search_snippets(html: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    for line in html.lines() {
        if line.contains("result__snippet") || line.contains("result__title") {
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
    snippets.truncate(10);
    snippets
}
