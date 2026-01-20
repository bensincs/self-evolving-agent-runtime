//! Common utilities for self-evolving agent capabilities.
//!
//! This crate provides helpers for:
//! - Reading JSON input from stdin
//! - Writing JSON output to stdout
//! - Making HTTP requests
//! - Time/date utilities
//! - Error handling patterns

use serde::{de::DeserializeOwned, Serialize};
use std::io::Read;

// Re-export chrono for time operations
pub use chrono;

/// Error type for capability operations.
#[derive(Debug, Serialize)]
pub struct CapabilityError {
    pub error: String,
}

impl CapabilityError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for CapabilityError {}

/// Read and parse JSON input from stdin.
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct Input { city: String }
///
/// let input: Input = capability_common::read_input()?;
/// ```
pub fn read_input<T: DeserializeOwned>() -> Result<T, CapabilityError> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| CapabilityError::new(format!("Failed to read stdin: {}", e)))?;

    serde_json::from_str(&input)
        .map_err(|e| CapabilityError::new(format!("Invalid JSON input: {}", e)))
}

/// Read raw JSON value from stdin (when you don't know the structure).
pub fn read_input_value() -> Result<serde_json::Value, CapabilityError> {
    read_input()
}

/// Write a successful JSON response to stdout.
pub fn write_output<T: Serialize>(output: &T) {
    match serde_json::to_string(output) {
        Ok(json) => println!("{}", json),
        Err(e) => write_error(&format!("Failed to serialize output: {}", e)),
    }
}

/// Write an error response to stdout as JSON.
pub fn write_error(msg: &str) {
    let err = CapabilityError::new(msg);
    // Unwrap is safe here since CapabilityError is simple
    println!("{}", serde_json::to_string(&err).unwrap());
}

/// Make an HTTP GET request and return the response body as a string.
///
/// # Example
/// ```ignore
/// let body = capability_common::http_get("https://api.example.com/data")?;
/// ```
pub fn http_get(url: &str) -> Result<String, CapabilityError> {
    ureq::get(url)
        .call()
        .map_err(|e| CapabilityError::new(format!("HTTP request failed: {}", e)))?
        .into_string()
        .map_err(|e| CapabilityError::new(format!("Failed to read response body: {}", e)))
}

/// Make an HTTP GET request and parse the response as JSON.
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct Weather { temp: f64 }
///
/// let weather: Weather = capability_common::http_get_json("https://wttr.in/London?format=j1")?;
/// ```
pub fn http_get_json<T: DeserializeOwned>(url: &str) -> Result<T, CapabilityError> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| CapabilityError::new(format!("HTTP request failed: {}", e)))?;

    response
        .into_json()
        .map_err(|e| CapabilityError::new(format!("Failed to parse JSON response: {}", e)))
}

/// Run a capability with automatic error handling.
///
/// This is the recommended way to write a capability main function.
/// It handles errors gracefully and outputs JSON error responses.
///
/// # Example
/// ```ignore
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize)]
/// struct Input { name: String }
///
/// #[derive(Serialize)]
/// struct Output { greeting: String }
///
/// fn main() {
///     capability_common::run(|input: Input| {
///         Ok(Output {
///             greeting: format!("Hello, {}!", input.name)
///         })
///     });
/// }
/// ```
pub fn run<I, O, F>(handler: F)
where
    I: DeserializeOwned,
    O: Serialize,
    F: FnOnce(I) -> Result<O, CapabilityError>,
{
    match read_input::<I>() {
        Ok(input) => match handler(input) {
            Ok(output) => write_output(&output),
            Err(e) => write_error(&e.error),
        },
        Err(e) => write_error(&e.error),
    }
}

// Re-export commonly used items for convenience
pub use serde;
pub use serde_json;

// ============ Time Utilities ============

/// Get the current UTC time as an ISO 8601 string.
///
/// # Example
/// ```ignore
/// let time = capability_common::utc_now_iso8601();
/// // Returns something like "2024-01-20T15:30:45.123456789Z"
/// ```
pub fn utc_now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Get the current UTC time as a Unix timestamp (seconds since epoch).
pub fn utc_now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Get the current UTC time as a Unix timestamp with milliseconds.
pub fn utc_now_timestamp_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
