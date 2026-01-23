# Self-Evolving Agent Runtime

**Status:** Working Prototype

---

## The Problem

Agents struggle to choose the right tool when given too many options. To address this, people have started building coding agents with a minimal toolset — agents that write code on-the-fly to solve problems.

But this introduces a new issue: **the code is ephemeral**. The agent might solve the same problem differently each time, turning what should be a deterministic operation into something probabilistic.

## The Solution

This runtime gives the coding agent a way to **persist and reuse** its code:

1. When the agent writes code that works, it's compiled to WASM and stored as a **capability**
2. Next time a similar task arrives, the agent can **run the existing capability** (deterministic) or **mutate it** to create a variant
3. As capabilities accumulate, the agent faces tool overload again — so we use **vector similarity search** to surface only the most relevant capabilities for each task

This is essentially **RAG for tools**: instead of dumping all capabilities into the context, we embed the task and retrieve only the nearest matches.

The result: an agent that **learns skills over time**, reuses proven solutions, and stays focused on relevant capabilities.

---

## What is this?

A runtime that gives LLMs **procedural memory** — the ability to create, store, improve, and reuse executable capabilities across sessions.

Instead of regenerating code every time, agents can:

- **Execute** capabilities safely via WASM (Wasmtime)
- **Retrieve** relevant capabilities via semantic embeddings
- **Mutate** existing capabilities to create specialized variants
- **Persist** skills for reuse across sessions

> **Try → Fail → Mutate → Store → Recall**

---

## Architecture

```
┌─────────────────────────────────────┐
│           Agent (LLM)               │
│  - receives task                    │
│  - sees nearest capabilities        │
│  - calls run_capability or mutate   │
└──────────────┬──────────────────────┘
               │
     ┌─────────┴─────────┐
     ▼                   ▼
┌────────────────┐  ┌────────────────────┐
│ run_capability │  │ mutate_capability  │
│ executes WASM  │  │ creates new skill  │
└───────┬────────┘  └─────────┬──────────┘
        │                     │
        ▼                     ▼
┌─────────────────────────────────────┐
│        CapabilityStore              │
│  - registry (meta.json files)       │
│  - index (embeddings + similarity)  │
│  - runner (Wasmtime execution)      │
└─────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites

- Rust toolchain with `wasm32-wasip1` target
- Azure OpenAI / Microsoft Foundry API access

### Setup

```bash
# Add WASM target
rustup target add wasm32-wasip1

# Set environment variables
export FOUNDRY_ENDPOINT="https://your-endpoint.azure.com"
export FOUNDRY_API_KEY="your-api-key"
export FOUNDRY_CHAT_DEPLOYMENT="gpt-4o"
export FOUNDRY_EMBED_DEPLOYMENT="text-embedding-3-small"
export FOUNDRY_API_VERSION="2024-12-01-preview"

# Build and run
cargo run -p se_runtime_host
```

### Example Session

```
> Get the salary details for employee E001

Nearest capabilities:
  - get_salary_details (score = 0.912)

[AGENT] Using run_capability...

[FINAL ANSWER]
Employee E001 (John Smith) earns $95,000 base salary...
```

---

## Project Structure

```
├── crates/
│   ├── core/           # Runtime library
│   │   ├── ai_client        # AI client trait
│   │   ├── capability_index # Embedding similarity search
│   │   ├── capability_registry
│   │   ├── capability_runner # Wasmtime execution + host functions
│   │   ├── embedding        # Embedder trait + Foundry impl
│   │   └── foundry_client   # Azure OpenAI client
│   │
│   └── host/           # CLI application
│       ├── agent            # Main agent loop
│       ├── mutation_agent   # Code generation agent
│       └── store            # Capability store (registry + index)
│
└── capabilities/
    ├── crates/         # Capability source code
    │   ├── common/          # Shared library (EmployeeDatabase, etc.)
    │   ├── get_salary_details/
    │   ├── get_hr_records/
    │   ├── update_employee_car_details/
    │   └── ...
    │
    └── employee_database.json  # Shared data file
```

---

## Capabilities

Capabilities are **WASM binaries** that:
- Read JSON from stdin
- Write JSON to stdout
- Are discovered via semantic embeddings
- Can be mutated to create new variants

Each capability has a `meta.json`:

```json
{
  "id": "get_salary_details",
  "summary": "Retrieves salary and compensation details for an employee",
  "binary": "get_salary_details.wasm"
}
```

### Host Functions

Capabilities can use these host functions (via `capability_common` crate):

| Function | Description |
|----------|-------------|
| `http_get(url)` | Fetch data from HTTP endpoints |
| `file_read(path)` | Read files (e.g., shared database) |
| `file_write(path, data)` | Write files (for UPDATE capabilities) |
| `current_time_millis()` | Get current timestamp |

---

## Mutation Agent

When no existing capability matches a task, the agent can **mutate** an existing one:

1. Clones parent capability source
2. Generates new code via LLM (with web search, docs lookup)
3. Builds to WASM (`wasm32-wasip1`)
4. Tests the capability
5. Persists with new `meta.json`

The mutation agent has access to tools: `web_search`, `http_get`, `read_file`, `write_file`, `build`, `test`, `rustc_explain`, `complete`.

---

## Current Capabilities

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

---

## Roadmap

- [x] WASM execution via Wasmtime
- [x] Embedding + semantic similarity search
- [x] LLM tool-calling integration (Azure OpenAI)
- [x] Mutation agent for capability generation
- [x] Host functions (HTTP, file I/O, time)
- [x] Capability deprecation on repeated failures
- [ ] Capability versioning / genealogy tracking
- [ ] Multi-language capability support
- [ ] Automated capability pruning
- [ ] Web UI

---

## Documentation

See **[PROPOSAL.md](PROPOSAL.md)** for:

- Design rationale
- Architecture details
- Mutation strategy
- Future directions

---

## License

MIT
