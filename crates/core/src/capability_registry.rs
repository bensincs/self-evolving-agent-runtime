// crates/core/src/capability_registry.rs
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::types::{CapabilityRecord, CapabilityStatus};

/// On-disk representation of a capability's metadata.
///
/// This maps 1:1 to meta.json for now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityMeta {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>, // allow preload if you want later
    #[serde(default)]
    pub binary: Option<String>, // relative path to binary within the capability dir
    #[serde(default)]
    pub status: CapabilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
}

/// Registry is responsible for loading capabilities from disk.
pub struct CapabilityRegistry {
    root: PathBuf,
}

impl CapabilityRegistry {
    /// Create a new registry rooted at a directory like "capabilities" or "storage/capabilities".
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Load all capabilities from the registry root.
    ///
    /// Expected layout:
    /// capabilities/
    ///   crates/
    ///     <capability_id>/
    ///       meta.json
    pub fn load_capabilities(&self) -> Result<Vec<CapabilityRecord>> {
        let mut records = Vec::new();

        // Look in the crates/ subdirectory
        let crates_dir = self.root.join("crates");

        let entries = match fs::read_dir(&crates_dir) {
            Ok(e) => e,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // No crates directory yet – return empty.
                return Ok(records);
            }
            Err(err) => {
                return Err(err).context(format!(
                    "failed to read capabilities directory at {:?}",
                    &crates_dir
                ));
            }
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            let data = fs::read_to_string(&meta_path)
                .with_context(|| format!("failed to read {:?}", meta_path))?;
            let meta: CapabilityMeta = serde_json::from_str(&data)
                .with_context(|| format!("failed to parse {:?}", meta_path))?;

            let record = CapabilityRecord {
                id: meta.id,
                summary: meta.summary,
                embedding: meta.embedding,
                binary: meta.binary,
                status: meta.status,
                replaced_by: meta.replaced_by,
            };

            records.push(record);
        }

        Ok(records)
    }
}
