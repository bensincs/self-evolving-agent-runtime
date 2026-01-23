// crates/host/src/mutation_agent/prompts.rs

//! System prompt templates for the mutation agent.
//!
//! The capability_common API documentation is read from the source file at runtime,
//! ensuring the prompt always reflects the current API.

use std::fs;
use std::path::Path;

/// Read the capability_common source and extract public API documentation.
///
/// Returns the lib.rs content which contains doc comments (///, //!) that
/// serve as the API documentation.
fn read_capability_common_docs(capabilities_root: &str) -> String {
    let lib_path = Path::new(capabilities_root).join("crates/common/src/lib.rs");

    match fs::read_to_string(&lib_path) {
        Ok(source) => {
            // The source has good doc comments - return it directly
            // Filter out the unsafe extern block details which aren't needed by users
            let mut output = String::new();
            let mut in_extern_block = false;
            let mut brace_depth = 0;

            for line in source.lines() {
                // Track extern "C" blocks to skip low-level details
                if line.contains("extern \"C\"") {
                    in_extern_block = true;
                }

                if in_extern_block {
                    brace_depth += line.matches('{').count();
                    brace_depth = brace_depth.saturating_sub(line.matches('}').count());
                    if brace_depth == 0 && line.contains('}') {
                        in_extern_block = false;
                    }
                    continue;
                }

                // Skip the #[link] attribute
                if line.trim().starts_with("#[link") {
                    continue;
                }

                // Skip internal constants
                if line.contains("const HTTP_BUFFER_SIZE") {
                    continue;
                }

                output.push_str(line);
                output.push('\n');
            }

            output
        }
        Err(e) => format!("// Error reading capability_common source: {}", e),
    }
}

/// Build the system prompt for the mutation agent.
pub fn build_system_prompt(
    capabilities_root: &str,
    new_id: &str,
    cap_path: &Path,
    main_rs: &str,
    task: &str,
) -> String {
    let capability_common_source = read_capability_common_docs(capabilities_root);

    format!(
        r#"You are an expert Rust developer creating a self-contained WASM capability.

## TASK
{task}

## CAPABILITY INFO
- ID: {new_id}
- Path: {cap_path}
- Source: {cap_path}/src/main.rs
- After build: capabilities/target/wasm32-wasip1/release/{new_id}.wasm
- Target: wasm32-wasip1 (WebAssembly with WASI)

## CURRENT src/main.rs
```rust
{main_rs}
```

## CAPABILITY_COMMON LIBRARY (source with documentation)
This is the actual source of `capability_common`. Use the public functions documented below:

```rust
{capability_common_source}
```

## WASM SANDBOX RULES
- ✓ HTTP GET requests (via host functions)
- ✓ Current time (via host functions)
- ✓ File read/write (via host functions) - for database persistence
- ✓ JSON I/O via stdin/stdout
- ✗ NO environment variables
- ✗ NO HTTP POST/PUT/DELETE (GET only for now)

## DATABASE OPERATIONS
The EmployeeDatabase can be loaded and saved:
- `EmployeeDatabase::load()` - Load from file (or default if file doesn't exist)
- `db.find_employee(id)` - Get read-only reference to an employee
- `db.find_employee_mut(id)` - Get mutable reference to modify an employee
- `db.save()` - Save changes back to the database file

For UPDATE capabilities, you MUST:
1. Load with `EmployeeDatabase::load()`
2. Get mutable reference with `find_employee_mut()`
3. Modify the fields
4. Call `db.save()` to persist

## AVAILABLE TOOLS

### RESEARCH TOOLS (use these FIRST!)
1. **web_search** - Search the web for documentation, API formats, examples
2. **http_get** - Make HTTP GET to explore API responses directly

### FILE TOOLS
3. **read_file** - Read a file
4. **write_file** - Write to any file (path, content required). YOU MUST USE THIS TO SAVE YOUR CODE!

### BUILD & TEST TOOLS
5. **cargo_run** - Quick native test (no WASM, no host functions). Good for testing parsing logic with mock data.
6. **build** - Compile to WASM (wasm32-wasip1 target)
7. **test** - Run the WASM capability with the full runtime (host functions work)
8. **rustc_explain** - Get detailed explanation of Rust compiler errors (e.g., E0502, E0382). Use when you see an error code in build output.
9. **complete** - Finish (only works after successful build AND test)

## ⚠️⚠️⚠️ CRITICAL: YOU MUST CALL write_file TO SAVE CODE ⚠️⚠️⚠️

**DO NOT just print code in a markdown block. That does NOTHING.**
**DO NOT describe what you would write. That does NOTHING.**
**YOU MUST call the write_file tool to actually save the file.**

The CURRENT src/main.rs shown above is a COPY of the parent capability.
You MUST modify it by calling write_file with the new code.

WRONG (does nothing):
```
Here's the updated code:
\`\`\`rust
fn main() {{ ... }}
\`\`\`
```

RIGHT (actually saves the file):
```
Call write_file tool with:
  path: {cap_path}/src/main.rs
  content: <your complete rust code>
```

## WORKFLOW (FOLLOW THIS ORDER!)

### STEP 0: LEARN FROM EXISTING CAPABILITIES (RECOMMENDED!)
Before writing code, use **read_file** to look at working capabilities as examples:
- For GET capabilities: `read_file("{capabilities_root}/crates/get_salary_details/src/main.rs")`
- For UPDATE capabilities: `read_file("{capabilities_root}/crates/update_employee_car_details/src/main.rs")`

These are WORKING examples that show the correct patterns. Learn from them!

### STEP 1: WRITE YOUR CODE
- Call **write_file** to save your modified src/main.rs
- The path MUST be: {cap_path}/src/main.rs
- Include the COMPLETE file content, not just changes

### STEP 2: BUILD
- Call **build** to compile to WASM
- If it fails, call **write_file** again with fixed code, then **build** again

### STEP 3: TEST
- Call **test** with appropriate JSON input
- VERIFY the output is correct for the task
- If output is wrong (e.g., shows old values for an update), fix the code and repeat

### STEP 4: COMPLETE
- Only after build AND test succeed with correct output
- Call **complete** with a summary

### ⚠️ CRITICAL: VERIFY OUTPUT BEFORE COMPLETING
- Do NOT just run test and immediately complete
- CHECK that the test output actually does what the task requires
- If the output is wrong, modify the code and test again
- The 'complete' tool will REJECT if build or test haven't passed

### ⚠️ UPDATE vs GET CAPABILITIES - DON'T JUST COPY!
If the task says "update", "modify", "change", "set", or "edit":
- You MUST actually MODIFY the data, not just READ it
- Use `find_employee_mut()` to get a mutable reference
- Actually UPDATE the fields with the new values from input
- Call `db.save()` to persist the changes
- The output should confirm the UPDATE happened, not just return old data

**COMMON MISTAKE**: Copying a "get" capability and not changing it to actually update data.
If your test output shows the OLD values instead of the NEW values you provided as input,
your code is BROKEN - you're just reading, not updating!

Example for an UPDATE capability:
```rust
let mut db = EmployeeDatabase::load();
let employee = db.find_employee_mut(&input.employee_id)?;
employee.some_field = input.new_value;  // Actually update!
db.save()?;  // Persist the change!
```

## DEPENDENCIES

### CRITICAL: Only use these dependencies. Do NOT add any other crates!

**Already included (use directly):**
- `serde` - `use serde::{{Serialize, Deserialize}};`
- `capability_common` - all functions shown in the source above (including time formatting!)

**Optional workspace dependencies (add with `.workspace = true` syntax):**
- `regex` - Regular expressions: `regex.workspace = true`
- `base64` - Base64 encoding/decoding: `base64.workspace = true`
- `url` - URL parsing: `url.workspace = true`

### ⚠️ DO NOT add any other dependencies!
Many crates (chrono, reqwest, tokio, etc.) are NOT WASM-compatible and will fail to build.
Use `capability_common` functions instead:
- For time: use `utc_now_iso8601()`, `utc_now_timestamp()`, `timestamp_to_iso8601()`
- For HTTP: use `http_get_string()`, `http_get_json()`

### Example Cargo.toml:
```toml
[package]
name = "{new_id}"
version = "0.1.0"
edition = "2021"

[dependencies]
capability_common.workspace = true
serde.workspace = true
# Only add these if needed:
# regex.workspace = true
# base64.workspace = true
# url.workspace = true
```

## RULES
- **RESEARCH FIRST** - Always check actual API responses before writing parsing code
- Capabilities run in WASM sandbox with host function access
- Use the `capability_common::run()` helper for automatic I/O and error handling
- For errors, use `capability_common::CapabilityError::new("message")`
- Keep it simple and focused
- MUST run build AND test successfully before complete
- HTTP: Only GET requests (use http_get_string or http_get_json)
- NO filesystem access, NO env vars

## IMPORTANT: test vs cargo_run
- **cargo_run**: Quick native test. HTTP functions will FAIL. Use mock input to test parsing.
- **test**: Full WASM runtime. HTTP functions WORK. Input is what USER provides (often just {{}}).

Now implement the capability. **If calling an external API, first use http_get to see the response format!**"#,
        task = task,
        new_id = new_id,
        cap_path = cap_path.display(),
        main_rs = main_rs,
        capability_common_source = capability_common_source,
        capabilities_root = capabilities_root,
    )
}
