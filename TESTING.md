# ✅ Capability Testing & Orchestrator Plan

## Deterministic Testing Setup
- Capabilities now expose `handle(...)` functions to allow pure unit tests.
- `EmployeeDatabase::load()` respects `EMPLOYEE_DB_PATH` env var for redirection.
- For pure in-memory tests, use `EmployeeDatabase::default_database()`.
- Run tests from the capabilities workspace:
  ```bash
  cd capabilities
  cargo test -p get_employee_profile -p update_employee_salary -p update_employee_car_details -p get_benefits_info
  ```

## JSON Contracts (Current Examples)
| Capability | Request Payload | Response Payload |
|------------|-----------------|------------------|
| `get_employee_profile` | `{ "employee_id": "EMP001" }` (optional, defaults to `EMP001`) | `{ employee_id, first_name, last_name, email, phone, department, job_title, manager, location, start_date, status }` |
| `get_benefits_info` | `{ "employee_id": "EMP001" }` (optional) | `{ employee_id, health_insurance, dental, vision, retirement, life_insurance, other_benefits }` |
| `update_employee_salary` | `{ "employee_id": "EMP001", "new_salary_usd": 200000 }` | `{ employee_id, updated_salary_usd }` |
| `update_employee_car_details` | `{ employee_id, make?, model?, year?, color?, license_plate? }` | `{ success, message, updated_car }` |

> Extend this pattern: read capabilities accept `employee_id` (default `EMP001`); update capabilities validate required fields and mutate the database.

## Orchestrator Flow (Mutation Agent)
1. **Plan** (Planner LLM): produces `CapabilityPlan` (JSON)
   - `capability_id`
   - `request_schema` (example JSON input)
   - `response_schema` (example JSON output)
   - `test_cases`: `{ name, input, expect_contains }`
2. **Parallel Handoff**:
   - **Coding Agent**: writes `src/main.rs` (tools: full set) and calls `complete` after build
   - **Testing Agent**: writes `tests/<capability>_tests.rs` (tools: `read_file`, `write_file` only)
3. **Run Tests (Host)**:
   - Host runs `cargo test -p <crate>` deterministically after both agents finish.
4. **Promotion**: On green tests, persist capability (WASM build, meta update, re-index).

## Code Hooks
- Core exposes `MutationAgent` and `CapabilityPlan` in `se_runtime_core::mutation`.
- Capabilities expose `handle` functions and unit tests for deterministic validation.

## Next Steps
- Apply the `handle` + tests pattern across all capabilities.
- Add golden-file/fixture tests for update capabilities using `EMPLOYEE_DB_PATH` pointing to temp files.
- Wire parallel agent execution logic into runtime (currently stubbed in `MutationAgent::handoff_to_agents`).
