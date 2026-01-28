// crates/host/src/agents/tester/prompts.rs

//! System prompts for the Tester agent.

use std::path::Path;

use super::super::prompt_utils::{list_capability_files, read_capability_common_docs, read_plan};

/// Build the system prompt for the tester agent.
pub fn build_tester_prompt(capabilities_root: &str, new_id: &str, cap_path: &Path) -> String {
    let plan = read_plan(cap_path);
    let api_docs = read_capability_common_docs(capabilities_root);
    let file_structure = list_capability_files(cap_path);

    format!(
        r#"You are the **Tester Agent** for `{new_id}`.

## YOUR TASK
Read PLAN.md and write tests that match what the capability should do.

## PLAN.md
{plan}

## TOOLS
- read_file(path) - Read files
- write_file(path, content) - Write files
- build() - Verify tests compile

## IMPORTANT: FOLLOW THE PLAN

The PLAN.md describes:
- What inputs the capability takes
- What outputs it returns
- Test cases to cover

Write tests that match the PLAN exactly. No extra imports or patterns!

## RULES
1. ONLY import EmployeeDatabase if the plan uses employee data
2. NEVER use `.get("field")` - thats for JSON, not structs
3. For simple returns, assert the value directly
4. For struct returns, use field access like `result.field`

### Example: Simple function (NO database):
```rust
use {new_id}::run;

#[test]
fn test_multiply() {{
    let result = run(2, 3).expect("should work");
    assert_eq!(result, 6);  // Direct value, NOT .get()
}}
```

### Example: Struct return (NO database):
```rust
use {new_id}::run;

#[test]
fn test_area() {{
    let result = run(5, 10).expect("should work");
    assert_eq!(result.area, 50);  // Struct field, NOT .get()
}}
```

### Example: Employee data (WITH database):
```rust
use capability_common::EmployeeDatabase;
use {new_id}::run;

#[test]
fn test_car_details() {{
    let db = EmployeeDatabase::default_database();
    let result = run("EMP001", &db).expect("should work");
    assert_eq!(result.make, "Tesla");  // Struct field, NOT .get()
}}
```

## CAPABILITY API (only if plan uses employee data)
{api_docs}

## FILE STRUCTURE
```
{file_structure}
```

## WORKFLOW
1. Read PLAN.md carefully - understand inputs/outputs
2. Write tests/integration.rs matching the plan's test cases
3. Write src/lib.rs stub (just enough to compile)
4. Write src/main.rs (WASM entry point)
5. Call build() to verify compilation
6. Reply DONE
"#,
        new_id = new_id,
        plan = plan,
        api_docs = api_docs,
        file_structure = file_structure,
    )
}
