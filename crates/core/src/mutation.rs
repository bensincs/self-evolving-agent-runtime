// crates/core/src/mutation.rs

//! Capability mutation types (used by host).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A plan describing a capability's IO contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPlan {
    pub capability_id: String,
    pub request_schema: Value,
    pub response_schema: Value,
    pub test_cases: Vec<TestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub input: Value,
    pub expect_contains: Value,
}
