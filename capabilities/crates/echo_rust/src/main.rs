//! Echo capability - echoes back whatever JSON it receives.
//!
//! This serves as a template for creating new Rust-based capabilities.

use capability_common::serde_json::Value;

fn main() {
    capability_common::run(|input: Value| Ok(input));
}
