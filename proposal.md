# Self-Evolving Agent Runtime

**Author:** Ben Sinclair  
**Date:** January 2026  
**Status:** Working Prototype

---

## 1. Overview

The **Self-Evolving Agent Runtime** enables AI agents to accumulate executable skills over time.

### The Problem

Agents struggle to choose the right tool when given too many options. To address this, people have started building coding agents with a minimal toolset — agents that write code on-the-fly to solve problems.

But this introduces a new issue: **the code is ephemeral**. The agent might solve the same problem differently each time, turning what should be a deterministic operation into something probabilistic.

### The Solution

This runtime gives the coding agent a way to **persist and reuse** its code:

1. When the agent writes code that works, it's compiled to WASM and stored as a **capability**
2. Next time a similar task arrives, the agent can **run the existing capability** (deterministic) or **mutate it** to create a variant
3. As capabilities accumulate, the agent faces tool overload again — so we use **vector similarity search** to surface only the most relevant capabilities for each task

This is essentially **RAG for tools**: instead of dumping all capabilities into the context, we embed the task and retrieve only the nearest matches.

The result: an agent that **learns skills over time**, reuses proven solutions, and stays focused on relevant capabilities.

### Core Capabilities

The runtime allows language models to:

- **Execute** compiled WASM binaries safely via Wasmtime
- **Retrieve** relevant capabilities via semantic embeddings
- **Mutate** existing capabilities to create specialized variants
- **Persist** capabilities for reuse across sessions
- **Learn** from failures by deprecating broken capabilities

This creates **persistent, evolving, executable capabilities** rather than one-off prompt-based behavior.

---

## 2. Motivation & Rationale

LLMs today excel at generating text and code, but they have **no procedural memory**. They:

- forget how they solved tasks previously,
- regenerate identical or similar code repeatedly,
- cannot specialize beyond prompt conditioning,
- cannot accumulate skills across sessions,
- and cannot improve from feedback loops.

This forces agents to exist in a **stateless world**, making them expensive, brittle, and fundamentally non-learning.

By contrast, humans learn through **procedural refinement**:

1. try using existing skills,
2. observe failures or feedback,
3. adjust technique,
4. store the new skill for next time,
5. recall and reuse skills in future.

This runtime mirrors that behavior, enabling agents to **retain capabilities, specialize with use, and evolve over time**.

---

## 3. Goals

The Self-Evolving Runtime aims to provide:

1. **Persistence** — Binaries and source code survive beyond a single session.
2. **Evolution** — Capabilities improve and specialize through mutation.
3. **Reusability** — Existing binaries can be invoked directly without regeneration.
4. **Safety** — Untrusted code runs in isolated WASM sandboxes.
5. **Teachability** — New behaviors can be added via natural language supervision.
6. **Semantic Retrieval** — Skills are discoverable through embeddings.

These goals reflect a shift from **stateless prompting** to **stateful procedural capability**.

---

## 4. Rationale for Design Decisions

### 4.1 Why Executable Binaries Instead of Text or Prompts?

Generated code that only exists in context:

- evaporates when the session ends,
- must be re-generated,
- is not validated,
- is not reproducible.

Compiling to binaries (WASM):

- creates a **reusable skill artifact**,
- ensures **exact reproducibility**,
- provides **fast execution**,
- prevents **prompt drift**,
- reduces **future inference costs**.

This converts code from "ephemeral output" into **persistent capability**.

### 4.2 Why WASM?

WASM was chosen because it provides:

- **sandboxed execution** (safety),
- **determinism** (no hidden side effects),
- **portability** (runs on any host),
- **language neutrality** (future multi-language),
- **small runtime surfaces** (security).

Alternative approaches (e.g., Python subprocesses) lack isolation and reproducibility.

### 4.3 Why Semantic Retrieval Instead of Tool Lists?

Manually registering tools:

- doesn't scale,
- requires human labeling,
- cannot adapt to emergent capabilities,
- forces brittle function mappings.

Semantic embeddings allow:

- **automatic capability retrieval**,
- **transfer learning across tasks**,
- **new tool discovery without manual wiring**,
- **continuous improvement based on usage**.

This creates an adaptive skill ecosystem.

### 4.4 Why Evolution Through Mutation?

Agents don't always produce optimal or domain-general tools on first try.

Mutation enables:

- **specialization** (e.g., get_salary → update_salary),
- **domain fitting** (e.g., handle new data formats),
- **feature addition** (e.g., handle edge cases),
- **error recovery** (e.g., fix broken capabilities).

This resembles human apprenticeship: **try → fail → refine**.

---

## 5. Current Implementation

### 5.1 Crate Structure

The runtime is split into two crates:

**`se_runtime_core`** — Library crate containing:
- `AiClient` trait — abstraction over LLM providers
- `FoundryClient` — Azure OpenAI / Microsoft Foundry implementation
- `Embedder` trait — abstraction over embedding providers
- `MicrosoftFoundryEmbedder` — embedding implementation
- `CapabilityRegistry` — loads capabilities from disk
- `CapabilityIndex` — semantic similarity search over embeddings
- `CapabilityRunner` — Wasmtime-based WASM execution with host functions

**`se_runtime_host`** — CLI application containing:
- `Agent` — main agentic loop with tool calling
- `MutationAgent` — code generation agent for creating new capabilities
- `CapabilityStore` — combines registry + index for capability management

### 5.2 Capability Layout

Each capability lives in `capabilities/crates/<name>/`:

```
capabilities/crates/get_salary_details/
├── Cargo.toml
├── meta.json
└── src/
    └── main.rs
```

The `meta.json` contains:

```json
{
  "id": "get_salary_details",
  "summary": "Retrieves salary and compensation details for an employee by ID",
  "binary": "get_salary_details.wasm"
}
```

Capabilities are compiled to WASM:
```bash
cargo build -p get_salary_details --release --target wasm32-wasip1
```

### 5.3 Host Functions

The `CapabilityRunner` exposes these host functions to WASM capabilities:

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_get` | `(url_ptr, url_len) -> i32` | HTTP GET, returns response in shared memory |
| `file_read` | `(path_ptr, path_len) -> i32` | Read file contents |
| `file_write` | `(path_ptr, path_len, data_ptr, data_len) -> i32` | Write file contents |
| `current_time_millis` | `() -> i64` | Current Unix timestamp in milliseconds |
| `current_time_secs` | `() -> i64` | Current Unix timestamp in seconds |

The `capability_common` crate provides safe Rust wrappers for these.

### 5.4 Agent Loop

The main agent exposes two tools to the LLM:

**`run_capability(capability_id, input_json)`**
- Executes a capability with JSON input
- Returns JSON output
- Tracks failures; deprecates after 2 consecutive failures

**`mutate_capability(parent_capability_id, task_description)`**
- Spawns the mutation agent
- Clones parent capability
- Generates new code via LLM
- Builds and tests the new capability
- Reloads the capability store

### 5.5 Mutation Agent

The mutation agent is a separate LLM-powered agent with tools:

| Tool | Description |
|------|-------------|
| `web_search` | Search the web for documentation/examples |
| `http_get` | Fetch specific URLs |
| `read_file` | Read existing capability source code |
| `write_file` | Write new capability code |
| `build` | Compile to WASM |
| `test` | Run the capability with test input |
| `rustc_explain` | Get detailed Rust error explanations |
| `complete` | Mark task as done (requires passing build + test) |

The agent follows a structured workflow:
1. Read existing similar capabilities for patterns
2. Write the new capability code
3. Build and fix any compiler errors
4. Test with sample input
5. Complete when output is correct

### 5.6 Semantic Retrieval

When a task arrives:
1. The task is embedded using the embedder
2. Cosine similarity is computed against all capability embeddings
3. Top-K nearest capabilities are returned to the agent
4. The agent decides which to run or whether to mutate

---

## 6. Interaction Flow

Typical flow for solving a task:

1. User provides natural-language task
2. Runtime embeds task and finds nearest capabilities
3. Agent receives task + capability summaries
4. Agent calls `run_capability` to execute existing capability
5. If no suitable capability exists, agent calls `mutate_capability`
6. Mutation agent generates, builds, and tests new capability
7. Capability store reloads with new capability
8. Agent runs new capability and returns result

This forms a **self-improving loop** where capabilities accumulate over time.

---

## 7. Current Capabilities

The runtime includes these employee data capabilities as examples:

| Capability | Type | Description |
|------------|------|-------------|
| `get_salary_details` | GET | Employee salary and compensation |
| `get_hr_records` | GET | HR records and employment info |
| `get_car_details` | GET | Company car information |
| `get_family_details` | GET | Employee family members |
| `get_emergency_contacts` | GET | Emergency contact information |
| `get_leave_balance` | GET | PTO and leave balances |
| `get_benefits_info` | GET | Benefits enrollment |
| `get_performance_reviews` | GET | Performance review history |
| `get_outlook_calendar` | GET | Calendar events (mock) |
| `get_employee_profile` | GET | Basic employee profile |
| `update_employee_car_details` | UPDATE | Modify car information |
| `update_employee_salary` | UPDATE | Modify salary details |

All capabilities share an `employee_database.json` file for data persistence.

---

## 8. Implementation Status

### Completed

- [x] WASM execution via Wasmtime
- [x] Embedding + semantic similarity search
- [x] LLM tool-calling integration (Azure OpenAI / Foundry)
- [x] Mutation agent for capability generation
- [x] Host functions (HTTP, file I/O, time)
- [x] Capability deprecation on repeated failures
- [x] CLI interface for task input
- [x] Example capabilities (employee data domain)

### Future Work

- [ ] Capability versioning / genealogy tracking
- [ ] Multi-language capability support (beyond Rust)
- [ ] Automated capability pruning
- [ ] Performance metrics and benchmarking
- [ ] Web UI for capability management
- [ ] Distributed capability sharing

---

## 9. License

MIT
