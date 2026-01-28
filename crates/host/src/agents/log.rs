// crates/host/src/agents/log.rs

//! Colored logging for the agent system.

#![allow(dead_code)]

use std::fmt::Display;

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

// Runtime color
const RUNTIME_COLOR: &str = "\x1b[38;5;45m"; // Cyan

// Agent colors
const PLANNER_COLOR: &str = "\x1b[38;5;141m"; // Purple
const CODER_COLOR: &str = "\x1b[38;5;39m"; // Blue
const TESTER_COLOR: &str = "\x1b[38;5;208m"; // Orange

// Status colors
const SUCCESS_COLOR: &str = "\x1b[38;5;82m"; // Green
const ERROR_COLOR: &str = "\x1b[38;5;196m"; // Red
const TOOL_COLOR: &str = "\x1b[38;5;226m"; // Yellow
const INFO_COLOR: &str = "\x1b[38;5;252m"; // Light gray

/// Agent type for logging context
#[derive(Debug, Clone, Copy)]
pub enum Agent {
    Runtime,
    Planner,
    Coder,
    Tester,
}

impl Agent {
    fn color(&self) -> &'static str {
        match self {
            Agent::Runtime => RUNTIME_COLOR,
            Agent::Planner => PLANNER_COLOR,
            Agent::Coder => CODER_COLOR,
            Agent::Tester => TESTER_COLOR,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Agent::Runtime => "RUNTIME",
            Agent::Planner => "PLANNER",
            Agent::Coder => "CODER",
            Agent::Tester => "TESTER",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Agent::Runtime => "🤖",
            Agent::Planner => "📋",
            Agent::Coder => "💻",
            Agent::Tester => "🧪",
        }
    }
}

/// Log an agent step
pub fn agent_step(agent: Agent, step: usize) {
    println!(
        "\n{}{}═══════════════════════════════════════════════════════════════{}",
        agent.color(),
        BOLD,
        RESET
    );
    println!(
        "{}{} {} STEP {}{}",
        agent.color(),
        agent.icon(),
        agent.name(),
        step,
        RESET
    );
    println!(
        "{}═══════════════════════════════════════════════════════════════{}",
        agent.color(),
        RESET
    );
}

/// Log agent message/thought
pub fn agent_message(agent: Agent, message: &str) {
    let truncated = truncate_message(message, 500);
    println!("{}{}  💭 {}{}", agent.color(), DIM, truncated, RESET);
}

/// Log a tool call
pub fn tool_call(agent: Agent, tool_name: &str, args_summary: &str) {
    println!(
        "{}  {}🔧 {}{}  ➜  {}{}{}",
        agent.color(),
        TOOL_COLOR,
        tool_name,
        RESET,
        DIM,
        truncate_message(args_summary, 100),
        RESET
    );
}

/// Log tool result (success)
pub fn tool_success(agent: Agent, result: &str) {
    let truncated = truncate_message(result, 200);
    println!(
        "{}  {}✓ {}{}",
        agent.color(),
        SUCCESS_COLOR,
        truncated,
        RESET
    );
}

/// Log tool result (error) - shows more detail than success
pub fn tool_error(agent: Agent, error: &str) {
    println!("{}  {}✗ ERROR:{}", agent.color(), ERROR_COLOR, RESET);
    // Show up to 20 lines of error output
    for line in error.lines().take(20) {
        let trimmed = if line.len() > 120 {
            format!("{}...", &line[..120])
        } else {
            line.to_string()
        };
        println!("{}    {}{}{}", agent.color(), ERROR_COLOR, trimmed, RESET);
    }
    let total_lines = error.lines().count();
    if total_lines > 20 {
        println!(
            "{}    {}[+{} more lines]{}",
            agent.color(),
            DIM,
            total_lines - 20,
            RESET
        );
    }
}

/// Log a success message (agent-independent)
pub fn success(message: impl Display) {
    println!("{}{}✨ {}{}", SUCCESS_COLOR, BOLD, message, RESET);
}

/// Log an error message (agent-independent)
pub fn error(message: impl Display) {
    println!("{}{}❌ {}{}", ERROR_COLOR, BOLD, message, RESET);
}

/// Log info message
pub fn info(message: impl Display) {
    println!("{}ℹ {}{}", INFO_COLOR, message, RESET);
}

/// Log file operation
pub fn file_op(agent: Agent, op: &str, path: &str, bytes: Option<usize>) {
    let size_info = bytes.map(|b| format!(" ({} bytes)", b)).unwrap_or_default();
    println!(
        "{}  {}📄 {} {}{}{}",
        agent.color(),
        DIM,
        op,
        path,
        size_info,
        RESET
    );
}

/// Log build operation
pub fn build_start(agent: Agent, target: &str) {
    println!(
        "{}  {}🔨 Building {}...{}",
        agent.color(),
        TOOL_COLOR,
        target,
        RESET
    );
}

/// Log build result
pub fn build_result(agent: Agent, success: bool, output: &str) {
    if success {
        println!(
            "{}  {}✓ Build succeeded{}",
            agent.color(),
            SUCCESS_COLOR,
            RESET
        );
    } else {
        println!("{}  {}✗ Build failed{}", agent.color(), ERROR_COLOR, RESET);
        for line in output.lines().take(10) {
            if line.contains("error") || line.contains("Error") {
                println!("{}    {}{}{}", agent.color(), ERROR_COLOR, line, RESET);
            }
        }
    }
}

/// Log test operation
pub fn test_start(agent: Agent, target: &str) {
    println!(
        "{}  {}🧪 Testing {}...{}",
        agent.color(),
        TOOL_COLOR,
        target,
        RESET
    );
}

/// Log test result
pub fn test_result(agent: Agent, success: bool, output: &str) {
    if success {
        println!(
            "{}  {}✓ Tests passed{}",
            agent.color(),
            SUCCESS_COLOR,
            RESET
        );
    } else {
        println!("{}  {}✗ Tests failed{}", agent.color(), ERROR_COLOR, RESET);
        for line in output.lines() {
            if line.contains("FAILED") || line.contains("panicked") || line.contains("assertion") {
                println!("{}    {}{}{}", agent.color(), ERROR_COLOR, line, RESET);
            }
        }
    }
}

/// Log web/http operation
pub fn http_op(agent: Agent, method: &str, url: &str) {
    println!(
        "{}  {}🌐 {} {}{}",
        agent.color(),
        DIM,
        method,
        truncate_message(url, 80),
        RESET
    );
}

/// Log agent completion
pub fn agent_done(agent: Agent) {
    println!(
        "{}{}  ✓ {} finished{}",
        agent.color(),
        SUCCESS_COLOR,
        agent.name(),
        RESET
    );
    println!(
        "{}═══════════════════════════════════════════════════════════════{}",
        agent.color(),
        RESET
    );
}

/// Truncate a message if too long
fn truncate_message(msg: &str, max_len: usize) -> String {
    let msg = msg.trim();
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > max_len {
        format!("{}...", &first_line[..max_len])
    } else if msg.lines().count() > 1 {
        format!("{} [+{} lines]", first_line, msg.lines().count() - 1)
    } else {
        first_line.to_string()
    }
}

/// Print a separator line
pub fn separator() {
    println!(
        "{}───────────────────────────────────────────────────────────────{}",
        DIM, RESET
    );
}
