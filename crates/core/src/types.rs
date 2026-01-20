use serde::{Deserialize, Serialize};

/// Unique identifier for a specific capability version (e.g. a fingerprint).
pub type CapabilityId = String;

/// Status of a capability in its lifecycle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityStatus {
    /// Active and available for use.
    #[default]
    Active,
    /// Replaced by a better version but still functional.
    Legacy,
    /// No longer functional or supported.
    Deprecated,
}

/// Capability metadata as seen by the embedding/index layer.
/// In the full system this will usually be built from meta.json on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRecord {
    pub id: CapabilityId,
    /// Human-readable summary of what the capability does.
    pub summary: String,
    /// Optional cached embedding (all embeddings must share the same dimension).
    pub embedding: Option<Vec<f32>>,
    /// Relative path to the capability binary (e.g. "bin.wasm" or "bin").
    pub binary: Option<String>,
    /// Lifecycle status of this capability.
    #[serde(default)]
    pub status: CapabilityStatus,
    /// If this capability was replaced, the ID of its replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
}

impl CapabilityRecord {
    /// Check if this capability is active (not legacy or deprecated).
    pub fn is_active(&self) -> bool {
        self.status == CapabilityStatus::Active
    }
}
