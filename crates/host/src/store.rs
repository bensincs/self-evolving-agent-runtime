// crates/host/src/store.rs

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use se_runtime_core::capability_index::CapabilityIndex;
use se_runtime_core::capability_registry::CapabilityRegistry;
use se_runtime_core::embedding::Embedder;
use se_runtime_core::types::{CapabilityRecord, CapabilityStatus};

/// Pure state: the capabilities and their similarity index.
/// This is what evolves over time as the agent creates new capabilities.
pub struct CapabilityStore {
    capabilities: Vec<CapabilityRecord>,
    index: CapabilityIndex,
}

impl CapabilityStore {
    /// Load capabilities from disk and build the similarity index.
    pub fn load(capabilities_root: &str, embedder: &impl Embedder) -> Result<Self> {
        let registry = CapabilityRegistry::new(capabilities_root);
        let mut capabilities = registry.load_capabilities()?;

        if capabilities.is_empty() {
            anyhow::bail!(
                "No capabilities found under {} – add some meta.json files!",
                capabilities_root
            );
        }

        let index = CapabilityIndex::build(&mut capabilities, embedder)?;

        Ok(Self {
            capabilities,
            index,
        })
    }

    /// Rebuild the similarity index after capabilities change (for mutate_capability later).
    pub fn rebuild_index(&mut self, embedder: &impl Embedder) -> Result<()> {
        self.index = CapabilityIndex::build(&mut self.capabilities, embedder)?;
        Ok(())
    }

    /// Build a model-friendly summary of the k nearest capabilities for a given task.
    /// Only includes active capabilities (not legacy or deprecated).
    pub fn capabilities_summary_for_task(
        &self,
        task: &str,
        embedder: &impl Embedder,
        k: usize,
    ) -> Result<(String, Vec<(String, f32)>)> {
        let nearest = self.index.nearest_for_task(task, embedder, k)?;

        // Filter to only active capabilities
        let active_nearest: Vec<_> = nearest
            .into_iter()
            .filter(|(id, _)| {
                self.capabilities
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| c.is_active())
                    .unwrap_or(false)
            })
            .collect();

        let mut lines = Vec::new();
        lines.push("You have access to the following capabilities:".to_string());
        for (id, _score) in &active_nearest {
            if let Some(cap) = self.capabilities.iter().find(|c| &c.id == id) {
                lines.push(format!("- id: {}\n  summary: {}", cap.id, cap.summary));
            }
        }

        Ok((lines.join("\n"), active_nearest))
    }

    /// Lookup a capability by id.
    pub fn get_capability(&self, id: &str) -> Option<&CapabilityRecord> {
        self.capabilities.iter().find(|c| c.id == id)
    }

    /// Get all capabilities.
    pub fn capabilities(&self) -> &[CapabilityRecord] {
        &self.capabilities
    }

    /// Number of loaded capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if store is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Reload all capabilities from disk (used after mutation creates new ones).
    pub fn reload(&mut self, capabilities_root: &str, embedder: &impl Embedder) -> Result<()> {
        let registry = CapabilityRegistry::new(capabilities_root);
        let mut capabilities = registry.load_capabilities()?;

        if capabilities.is_empty() {
            anyhow::bail!(
                "No capabilities found under {} – add some meta.json files!",
                capabilities_root
            );
        }

        let index = CapabilityIndex::build(&mut capabilities, embedder)?;

        self.capabilities = capabilities;
        self.index = index;

        println!("[STORE] Reloaded {} capabilities", self.capabilities.len());
        Ok(())
    }

    /// Mark a capability as deprecated (broken/non-functional).
    /// Updates both in-memory state and meta.json on disk.
    pub fn mark_deprecated(
        &mut self,
        capabilities_root: &str,
        capability_id: &str,
        reason: &str,
    ) -> Result<()> {
        // Update in-memory state
        if let Some(cap) = self.capabilities.iter_mut().find(|c| c.id == capability_id) {
            cap.status = CapabilityStatus::Deprecated;
        } else {
            anyhow::bail!("Capability '{}' not found", capability_id);
        }

        // Update meta.json on disk
        let meta_path = Path::new(capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        if !meta_path.exists() {
            anyhow::bail!(
                "meta.json not found for capability '{}' at {}",
                capability_id,
                meta_path.display()
            );
        }

        let content = fs::read_to_string(&meta_path)?;
        let mut meta: serde_json::Value = serde_json::from_str(&content)?;

        meta["status"] = json!("deprecated");
        meta["deprecated_reason"] = json!(reason);

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

        println!(
            "[STORE] Marked '{}' as deprecated: {}",
            capability_id, reason
        );
        Ok(())
    }
}
