// crates/host/src/agents/capability_ops.rs

//! Capability filesystem operations: copying, updating metadata, etc.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use super::log;

/// Handles capability filesystem operations.
pub struct CapabilityOps<'a> {
    capabilities_root: &'a str,
}

impl<'a> CapabilityOps<'a> {
    pub fn new(capabilities_root: &'a str) -> Self {
        Self { capabilities_root }
    }

    /// Create a new capability by copying the parent's entire crate directory.
    pub fn copy_capability(&self, parent_id: &str, new_id: &str) -> Result<()> {
        let crates_dir = Path::new(self.capabilities_root).join("crates");
        let src = crates_dir.join(parent_id);
        let dst = crates_dir.join(new_id);

        if !src.exists() {
            anyhow::bail!(
                "Parent capability '{}' not found at {}",
                parent_id,
                src.display()
            );
        }

        if dst.exists() {
            anyhow::bail!("Destination '{}' already exists", dst.display());
        }

        // Copy entire directory tree
        self.copy_dir_recursive(&src, &dst)?;

        // Update package name in Cargo.toml
        let cargo_path = dst.join("Cargo.toml");
        let cargo_content = fs::read_to_string(&cargo_path)?;
        let updated_cargo = cargo_content.replace(
            &format!("name = \"{}\"", parent_id),
            &format!("name = \"{}\"", new_id),
        );
        fs::write(&cargo_path, updated_cargo)?;

        // Update imports in main.rs (use new_id::run instead of parent_id::run)
        let main_rs_path = dst.join("src/main.rs");
        if main_rs_path.exists() {
            let main_content = fs::read_to_string(&main_rs_path)?;
            let updated_main = main_content
                .replace(
                    &format!("use {}::", parent_id),
                    &format!("use {}::", new_id),
                )
                .replace(&format!("{}::", parent_id), &format!("{}::", new_id));
            fs::write(&main_rs_path, updated_main)?;
        }

        // Update imports in tests/integration.rs
        let tests_path = dst.join("tests/integration.rs");
        if tests_path.exists() {
            let tests_content = fs::read_to_string(&tests_path)?;
            let updated_tests = tests_content
                .replace(
                    &format!("use {}::", parent_id),
                    &format!("use {}::", new_id),
                )
                .replace(&format!("{}::", parent_id), &format!("{}::", new_id));
            fs::write(&tests_path, updated_tests)?;
        }

        // Update meta.json with new id (pointing to WASM file)
        let meta = json!({
            "id": new_id,
            "summary": "New capability (pending implementation)",
            "binary": format!("../../target/wasm32-wasip1/release/{}.wasm", new_id)
        });
        fs::write(dst.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

        Ok(())
    }

    /// Recursively copy a directory.
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    /// Update the capability's meta.json with a new summary.
    pub fn update_meta_json(&self, capability_id: &str, summary: &str) -> Result<()> {
        let meta_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        let meta = json!({
            "id": capability_id,
            "summary": summary,
            "binary": format!("../../target/wasm32-wasip1/release/{}.wasm", capability_id),
            "status": "active"
        });

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }

    /// Mark a capability as legacy (replaced by a newer version).
    pub fn mark_as_legacy(&self, capability_id: &str, replaced_by: &str) -> Result<()> {
        let meta_path = Path::new(self.capabilities_root)
            .join("crates")
            .join(capability_id)
            .join("meta.json");

        if !meta_path.exists() {
            anyhow::bail!("Capability '{}' not found", capability_id);
        }

        let content = fs::read_to_string(&meta_path)?;
        let mut meta: serde_json::Value = serde_json::from_str(&content)?;

        meta["status"] = json!("legacy");
        meta["replaced_by"] = json!(replaced_by);

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        log::info(format!(
            "Marked '{}' as legacy (replaced by '{}')",
            capability_id, replaced_by
        ));
        Ok(())
    }
}
