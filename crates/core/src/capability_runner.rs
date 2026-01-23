// crates/core/src/capability_runner.rs

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use wasmtime::{Caller, Engine, Linker, Module, Store};
use wasmtime_wasi::pipe::MemoryOutputPipe;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::types::CapabilityRecord;

/// Default path for the shared employee database file.
const DEFAULT_DB_PATH: &str = "employee_database.json";

/// Runs WASM capabilities using Wasmtime with WASI + custom host functions.
///
/// Contract:
/// - Capabilities are .wasm modules compiled for wasm32-wasip1
/// - Input JSON is passed via stdin
/// - Output JSON is captured from stdout
/// - Host functions provide: HTTP GET, current time, file I/O, etc.
pub struct CapabilityRunner {
    root: PathBuf,
    engine: Engine,
    /// Path to the shared database file
    db_path: PathBuf,
}

impl CapabilityRunner {
    /// `root` should be the directory where capability folders live, e.g. "capabilities".
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let engine = Engine::default();
        let root_path = root.as_ref().to_path_buf();
        let db_path = root_path.join(DEFAULT_DB_PATH);
        Ok(Self {
            root: root_path,
            engine,
            db_path,
        })
    }

    /// Create a runner with a custom database path.
    pub fn with_db_path<P: AsRef<Path>, D: AsRef<Path>>(root: P, db_path: D) -> Result<Self> {
        let engine = Engine::default();
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            engine,
            db_path: db_path.as_ref().to_path_buf(),
        })
    }

    /// Get the path to the shared database file.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn run_capability(&self, cap: &CapabilityRecord, input_json: &str) -> Result<String> {
        let binary_rel = cap
            .binary
            .as_ref()
            .context("capability has no binary path configured")?;

        // Capabilities are in crates/<id>/ subdirectory
        let wasm_path = self.root.join("crates").join(&cap.id).join(binary_rel);

        if !wasm_path.exists() {
            anyhow::bail!(
                "capability WASM not found at {:?} for capability {}",
                wasm_path,
                cap.id
            );
        }

        // Compile the WASM module
        let module = Module::from_file(&self.engine, &wasm_path)
            .with_context(|| format!("failed to compile WASM module {:?}", wasm_path))?;

        // Set up stdin/stdout/stderr capture
        let stdin_data: bytes::Bytes = input_json.as_bytes().to_vec().into();
        let stdout_pipe = MemoryOutputPipe::new(1024 * 1024); // 1MB buffer
        let stderr_pipe = MemoryOutputPipe::new(64 * 1024); // 64KB buffer

        // Build WASI context with captured I/O
        let wasi_ctx = WasiCtxBuilder::new()
            .stdin(wasmtime_wasi::pipe::MemoryInputPipe::new(stdin_data))
            .stdout(stdout_pipe.clone())
            .stderr(stderr_pipe.clone())
            .build_p1();

        let mut store = Store::new(&self.engine, wasi_ctx);

        // Create linker with WASI + our host functions
        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        preview1::add_to_linker_sync(&mut linker, |cx| cx)?;

        // Add our custom host functions under "host" module
        Self::add_host_functions(&mut linker)?;

        // Instantiate and run
        let instance = linker
            .instantiate(&mut store, &module)
            .context("failed to instantiate WASM module")?;

        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .context("WASM module missing _start function")?;

        let result = start.call(&mut store, ());

        // Drop the store to release the pipes
        drop(store);

        // Get captured output
        let stdout_bytes = stdout_pipe.try_into_inner().unwrap_or_default();
        let stderr_bytes = stderr_pipe.try_into_inner().unwrap_or_default();

        let stdout = String::from_utf8(stdout_bytes.to_vec())
            .context("capability stdout was not valid UTF-8")?;
        let stderr = String::from_utf8(stderr_bytes.to_vec()).unwrap_or_default();

        // Handle execution result
        match result {
            Ok(()) => Ok(stdout),
            Err(e) => {
                // Check if it's a normal exit (exit code 0)
                if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                    if exit.0 == 0 {
                        return Ok(stdout);
                    }
                    anyhow::bail!(
                        "capability {} exited with code {}: {}",
                        cap.id,
                        exit.0,
                        stderr
                    );
                }
                anyhow::bail!("capability {} failed: {}: {}", cap.id, e, stderr)
            }
        }
    }

    /// Add custom host functions that capabilities can call.
    fn add_host_functions(linker: &mut Linker<WasiP1Ctx>) -> Result<()> {
        // host::http_get(url_ptr, url_len, result_ptr) -> i32
        // Returns: length of response body written to result_ptr, or negative on error
        linker.func_wrap(
            "host",
            "http_get",
            |mut caller: Caller<'_, WasiP1Ctx>,
             url_ptr: i32,
             url_len: i32,
             result_ptr: i32|
             -> i32 {
                // Read URL from WASM memory
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let url_bytes = {
                    let data = memory.data(&caller);
                    let start = url_ptr as usize;
                    let end = start + url_len as usize;
                    if end > data.len() {
                        return -2;
                    }
                    data[start..end].to_vec()
                };

                let url = match String::from_utf8(url_bytes) {
                    Ok(s) => s,
                    Err(_) => return -3,
                };

                // Make the HTTP request
                let response = match reqwest::blocking::get(&url) {
                    Ok(r) => r,
                    Err(_) => return -4,
                };

                let body = match response.text() {
                    Ok(b) => b,
                    Err(_) => return -5,
                };

                let body_bytes = body.as_bytes();

                // Write response to WASM memory
                let data = memory.data_mut(&mut caller);
                let start = result_ptr as usize;
                let end = start + body_bytes.len();
                if end > data.len() {
                    return -6; // Buffer too small
                }
                data[start..end].copy_from_slice(body_bytes);

                body_bytes.len() as i32
            },
        )?;

        // host::current_time_millis() -> i64
        // Returns: Unix timestamp in milliseconds
        linker.func_wrap("host", "current_time_millis", || -> i64 {
            chrono::Utc::now().timestamp_millis()
        })?;

        // host::current_time_secs() -> i64
        // Returns: Unix timestamp in seconds
        linker.func_wrap("host", "current_time_secs", || -> i64 {
            chrono::Utc::now().timestamp()
        })?;

        // host::file_read(path_ptr, path_len, result_ptr) -> i32
        // Returns: length of file content written to result_ptr, or negative on error
        // Error codes: -1 memory error, -2 path bounds, -3 invalid path, -4 not found,
        //              -5 permission denied, -6 read error, -7 buffer too small
        linker.func_wrap(
            "host",
            "file_read",
            |mut caller: Caller<'_, WasiP1Ctx>,
             path_ptr: i32,
             path_len: i32,
             result_ptr: i32|
             -> i32 {
                // Read path from WASM memory
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let path_bytes = {
                    let data = memory.data(&caller);
                    let start = path_ptr as usize;
                    let end = start + path_len as usize;
                    if end > data.len() {
                        return -2;
                    }
                    data[start..end].to_vec()
                };

                let path = match String::from_utf8(path_bytes) {
                    Ok(s) => s,
                    Err(_) => return -3,
                };

                // Read the file (relative paths resolved from current working directory)
                let contents = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        return match e.kind() {
                            std::io::ErrorKind::NotFound => -4,
                            std::io::ErrorKind::PermissionDenied => -5,
                            _ => -6,
                        };
                    }
                };

                let content_bytes = contents.as_bytes();

                // Write content to WASM memory
                let data = memory.data_mut(&mut caller);
                let start = result_ptr as usize;
                let end = start + content_bytes.len();
                if end > data.len() {
                    return -7; // Buffer too small
                }
                data[start..end].copy_from_slice(content_bytes);

                content_bytes.len() as i32
            },
        )?;

        // host::file_write(path_ptr, path_len, content_ptr, content_len) -> i32
        // Returns: 0 on success, or negative on error
        // Error codes: -1 memory error, -2 path bounds, -3 invalid path,
        //              -4 content bounds, -5 permission denied, -6 write error
        linker.func_wrap(
            "host",
            "file_write",
            |mut caller: Caller<'_, WasiP1Ctx>,
             path_ptr: i32,
             path_len: i32,
             content_ptr: i32,
             content_len: i32|
             -> i32 {
                // Read path and content from WASM memory
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let data = memory.data(&caller);

                // Read path
                let path_start = path_ptr as usize;
                let path_end = path_start + path_len as usize;
                if path_end > data.len() {
                    return -2;
                }
                let path_bytes = data[path_start..path_end].to_vec();

                let path = match String::from_utf8(path_bytes) {
                    Ok(s) => s,
                    Err(_) => return -3,
                };

                // Read content
                let content_start = content_ptr as usize;
                let content_end = content_start + content_len as usize;
                if content_end > data.len() {
                    return -4;
                }
                let content = data[content_start..content_end].to_vec();

                // Write the file
                match std::fs::write(&path, &content) {
                    Ok(()) => 0,
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::PermissionDenied => -5,
                        _ => -6,
                    },
                }
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CapabilityStatus;

    #[test]
    fn test_run_echo_capability() {
        // This test requires the echo_rust capability to be built first:
        // cd capabilities && cargo build --release --target wasm32-wasip1 -p echo_rust
        let runner = CapabilityRunner::new("capabilities").unwrap();

        let cap = CapabilityRecord {
            id: "echo_rust".to_string(),
            summary: "echo".to_string(),
            embedding: None,
            binary: Some("../../target/wasm32-wasip1/release/echo_rust.wasm".to_string()),
            status: CapabilityStatus::Active,
            replaced_by: None,
        };

        let input = r#"{"message": "hello world"}"#;
        let output = runner.run_capability(&cap, input).unwrap();

        assert!(output.contains("hello world"));
        assert!(output.contains("message"));
    }
}
