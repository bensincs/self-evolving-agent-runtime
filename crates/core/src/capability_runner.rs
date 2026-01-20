// crates/core/src/capability_runner.rs

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::types::CapabilityRecord;

/// Runs capabilities by executing their configured binary.
///
/// Contract:
/// - We resolve the binary as: {root}/{capability.id}/{binary_rel}
/// - We pass `input_json` to stdin
/// - We capture stdout as `String`
/// - Non-zero exit codes are treated as errors
pub struct CapabilityRunner {
    root: PathBuf,
}

impl CapabilityRunner {
    /// `root` should be the directory where capability folders live, e.g. "capabilities".
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn run_capability(&self, cap: &CapabilityRecord, input_json: &str) -> Result<String> {
        let binary_rel = cap
            .binary
            .as_ref()
            .context("capability has no binary path configured")?;

        // Capabilities are in crates/<id>/ subdirectory
        let binary_path = self.root.join("crates").join(&cap.id).join(binary_rel);

        if !binary_path.exists() {
            anyhow::bail!(
                "capability binary not found at {:?} for capability {}",
                binary_path,
                cap.id
            );
        }

        let mut child = Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn capability binary {:?}", binary_path))?;

        // write input JSON to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input_json.as_bytes())
                .context("failed to write input JSON to capability stdin")?;
        }

        let output = child
            .wait_with_output()
            .context("failed to wait for capability process")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "capability {} exited with status {:?}: {}",
                cap.id,
                output.status.code(),
                stderr
            );
        }

        let stdout =
            String::from_utf8(output.stdout).context("capability stdout was not valid UTF-8")?;

        Ok(stdout)
    }
}
