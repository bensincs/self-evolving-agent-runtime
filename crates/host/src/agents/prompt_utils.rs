// crates/host/src/agents/prompt_utils.rs

//! Shared utilities for building agent prompts.

use std::fs;
use std::path::Path;

/// Convert snake_case to PascalCase.
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// List files in a capability directory recursively.
pub fn list_capability_files(cap_path: &Path) -> String {
    fn list_dir_recursive(path: &Path, prefix: &str) -> String {
        let mut result = String::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name().to_string_lossy().to_string();
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    result.push_str(&format!("{}{}/ \n", prefix, name));
                    result.push_str(&list_dir_recursive(&entry_path, &format!("{}  ", prefix)));
                } else {
                    result.push_str(&format!("{}{} \n", prefix, name));
                }
            }
        }
        result
    }
    list_dir_recursive(cap_path, "")
}

/// Read the capability_common API documentation.
pub fn read_capability_common_docs(capabilities_root: &str) -> String {
    let doc_path = Path::new(capabilities_root).join("crates/common/API.md");
    fs::read_to_string(&doc_path).unwrap_or_else(|_| "API documentation not found.".to_string())
}

/// Read PLAN.md from the capability directory.
pub fn read_plan(cap_path: &Path) -> String {
    let plan_path = cap_path.join("PLAN.md");
    fs::read_to_string(&plan_path).unwrap_or_else(|_| "No plan found.".to_string())
}
