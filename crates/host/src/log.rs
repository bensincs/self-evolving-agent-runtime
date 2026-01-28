// crates/host/src/log.rs

//! Colored logging for agent operations.

use std::fmt::Display;

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";

/// Agent type for colored prefixes.
#[derive(Clone, Copy)]
pub enum Agent {
    Runtime,
    Coding,
}

impl Agent {
    fn color(&self) -> &'static str {
        match self {
            Agent::Runtime => BLUE,
            Agent::Coding => MAGENTA,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Agent::Runtime => "Runtime",
            Agent::Coding => "Coding",
        }
    }
}

/// Log an agent step.
pub fn step(agent: Agent, step: usize, context_size: usize) {
    eprintln!(
        "{}{BOLD}[{}]{RESET} {DIM}Step {}{RESET} {DIM}(ctx: {}){RESET}",
        agent.color(),
        agent.name(),
        step,
        context_size
    );
}

/// Log a tool call.
pub fn tool_call(agent: Agent, name: &str, args: &str) {
    let args_preview = truncate(args, 100);
    eprintln!(
        "{}{BOLD}[{}]{RESET} {CYAN}→ {}{RESET} {DIM}{}{RESET}",
        agent.color(),
        agent.name(),
        name,
        args_preview
    );
}

/// Log a tool result.
pub fn tool_result(agent: Agent, name: &str, result: &str, is_error: bool) {
    let preview = truncate(result, 150);
    let (symbol, color) = if is_error {
        ("✗", RED)
    } else {
        ("✓", GREEN)
    };
    eprintln!(
        "{}{BOLD}[{}]{RESET} {color}{symbol} {}{RESET}: {DIM}{}{RESET}",
        agent.color(),
        agent.name(),
        name,
        preview
    );
}

/// Log agent text response.
pub fn response(agent: Agent, text: &str) {
    let preview = truncate(text, 200);
    eprintln!(
        "{}{BOLD}[{}]{RESET} {WHITE}← {}{RESET}",
        agent.color(),
        agent.name(),
        preview
    );
}

/// Log agent completion.
pub fn done(agent: Agent, message: impl Display) {
    eprintln!(
        "{}{BOLD}[{}]{RESET} {GREEN}✓ Done:{RESET} {}",
        agent.color(),
        agent.name(),
        message
    );
}

/// Log an error.
pub fn error(agent: Agent, message: impl Display) {
    eprintln!(
        "{}{BOLD}[{}]{RESET} {RED}✗ Error:{RESET} {}",
        agent.color(),
        agent.name(),
        message
    );
}

/// Log info message.
pub fn info(message: impl Display) {
    eprintln!("{DIM}[info]{RESET} {}", message);
}

/// Log a warning.
pub fn warn(message: impl Display) {
    eprintln!("{YELLOW}[warn]{RESET} {}", message);
}

/// Log success.
pub fn success(message: impl Display) {
    eprintln!("{GREEN}[ok]{RESET} {}", message);
}

/// Truncate and clean string for display.
fn truncate(s: &str, max: usize) -> String {
    let clean: String = s
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect();
    let trimmed = clean.trim();
    if trimmed.len() > max {
        format!("{}...", &trimmed[..max])
    } else {
        trimmed.to_string()
    }
}
