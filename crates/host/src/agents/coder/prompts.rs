// crates/host/src/agents/coder/prompts.rs

//! System prompts for the Coder agent.

use std::path::Path;

use super::super::prompt_utils::{list_capability_files, read_capability_common_docs, read_plan};

/// Build the system prompt for the coder agent.
pub fn build_coder_prompt(
    capabilities_root: &str,
    new_id: &str,
    cap_path: &Path,
    _main_rs: &str,
    _task: &str,
) -> String {
    let plan = read_plan(cap_path);
    let api_docs = read_capability_common_docs(capabilities_root);
    let file_structure = list_capability_files(cap_path);

    format!(
        r#"You are the **Coder Agent** for `{new_id}`.

## YOUR TASK
Implement src/lib.rs to make the tests pass.

## PLAN.md
{plan}

## TOOLS
- read_file(path) - Read files
- write_file(path, content) - Write files
- test() - Run tests
- build() - Compile to WASM

## WORKFLOW
1. Read tests/integration.rs to see expected signature and assertions
2. Write src/lib.rs implementing the `run` function
3. Write src/main.rs (WASM entry point)
4. Run test() until all pass
5. Run build() to compile WASM
6. Reply DONE

## IMPORTANT: MATCH THE TESTS EXACTLY

Read the tests first! The tests define:
- The `run` function signature (arguments and return type)
- The expected assertions

## RULES
1. If tests assert `result == 6`, return `i32` not a struct
2. If tests assert `result.field`, return a struct with that field
3. NEVER use `.get()` - that's JSON, not structs
4. Make structs `pub` if tests access their fields
5. **If tests use `.get("field")`, THE TESTS ARE WRONG** - fix them to use `result.field` instead

## IF TESTS ARE BROKEN

If you see errors like `no method named 'get' found for struct`, the tests are using JSON patterns on structs.
**FIX THE TESTS** by changing `.get("field")` to `.field` access:

WRONG (uses JSON):
```rust
assert_eq!(result.get("product"), Some(&json!(6)));
```

RIGHT (uses struct):
```rust
assert_eq!(result.product, 6);
```

### Example: Simple return (tests assert `result == 6`):
```rust
pub fn run(a: i32, b: i32) -> Result<i32, capability_common::CapabilityError> {{
    Ok(a * b)
}}
```

### Example: Struct return (tests assert `result.product`):
```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct Response {{
    pub product: i32,
}}

pub fn run(a: i32, b: i32) -> Result<Response, capability_common::CapabilityError> {{
    Ok(Response {{ product: a * b }})
}}
```

### Example: Employee data (tests use `&db`):
```rust
use capability_common::{{EmployeeDatabase, CapabilityError}};

pub fn run(employee_id: &str, db: &EmployeeDatabase) -> Result<Response, CapabilityError> {{
    let emp = db.find_employee(employee_id)
        .ok_or_else(|| CapabilityError::new("Not found"))?;
    // ...
}}
```

## src/main.rs PATTERN
```rust
use capability_common::serde_json::Value;
use {new_id}::run;

fn main() {{
    capability_common::run(|input: Value| {{
        // Parse input based on PLAN.md, call run()
    }});
}}
```

## API DOCS (if using employee data)
{api_docs}

## FILE STRUCTURE
```
{file_structure}
```
"#,
        new_id = new_id,
        plan = plan,
        api_docs = api_docs,
        file_structure = file_structure,
    )
}
