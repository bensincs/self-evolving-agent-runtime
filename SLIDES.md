# Self-Evolving Agent Runtime
## Learning, Deterministic Agents via Persistent Capabilities

---

# The Problem with Tool-Heavy Agents

**Too many tools → decision paralysis**

- Agents struggle to choose correctly when given 50+ tools
- Performance degrades as context becomes overloaded
- Common workaround: give agents minimal toolsets

---

# The Problem with Coding Agents

**Ephemeral code → probabilistic behavior**

- Agents write code on-the-fly to solve tasks
- Same task → different code each run
- What *should* be deterministic becomes a dice roll
- No learning between sessions

---

# The Core Idea

> **What if agents could remember and reuse their own code?**

Instead of:
```
Task → LLM generates code → execute → discard
```

We want:
```
Task → find existing capability → execute deterministically
      ↓ (if none exists)
      LLM generates code → compile → store → reuse forever
```

---

# The Solution: Persistent Capabilities

A **capability** is:

- Working code the agent wrote
- Compiled to **WASM** for portability + sandboxing
- Stored with metadata + embeddings
- Retrievable via vector similarity search

```
capabilities/
  get_employee_profile/
    meta.json          ← summary for retrieval
    get_employee_profile.wasm   ← executable artifact
```

---

# Architecture Overview

```
┌────────────────────────────────────────────────────────────┐
│                      User Task                             │
└─────────────────────────┬──────────────────────────────────┘
                          ▼
┌────────────────────────────────────────────────────────────┐
│              Embed task → Vector Search                    │
│              Find nearest capabilities                     │
└─────────────────────────┬──────────────────────────────────┘
                          ▼
┌────────────────────────────────────────────────────────────┐
│                     Agent Loop                             │
│  • Sees task + top-k relevant capabilities                 │
│  • Decides: run_capability OR mutate_capability            │
└─────────────┬──────────────────────────────┬───────────────┘
              │                              │
              ▼                              ▼
┌─────────────────────────┐    ┌─────────────────────────────┐
│    run_capability       │    │    mutate_capability        │
│    Execute WASM         │    │    Generate + compile       │
│    Deterministic!       │    │    Store new capability     │
└─────────────────────────┘    └─────────────────────────────┘
```

---

# Pattern 1: Vector Search for Tool Selection

## The Problem

Dumping all tools into context doesn't scale.

## The Solution

**RAG for tools, not documents.**

```rust
// Embed the task
let query_embedding = embedder.embed(task_description)?;

// Find nearest capabilities via cosine similarity  
let nearest = index.nearest_from_embedding(&query_embedding, k);

// Only surface top-k to the agent
```

---

# Pattern 1: How It Works

```
Task: "Get John's salary details"
                    │
                    ▼
            ┌───────────────┐
            │  Embed Task   │
            └───────┬───────┘
                    ▼
     ┌──────────────────────────────┐
     │  Cosine Similarity Search    │
     │  against capability index    │
     └──────────────┬───────────────┘
                    ▼
┌─────────────────────────────────────────────────┐
│  Top 2 Results:                                 │
│   • get_salary_details (0.89)                   │
│   • get_employee_profile (0.76)                 │
└─────────────────────────────────────────────────┘
```

Agent only sees 2 relevant tools, not 50.

---

# Pattern 1: The Capability Index

```rust
pub struct CapabilityIndex {
    dim: usize,
    embeddings: HashMap<String, Vec<f32>>,  // id → embedding
}

impl CapabilityIndex {
    pub fn nearest_for_task(&self, task: &str, embedder: &E, k: usize) 
        -> Vec<(String, f32)> 
    {
        let query_emb = embedder.embed(task)?;
        
        // Score all capabilities
        let mut scored: Vec<_> = self.embeddings.iter()
            .map(|(id, emb)| (id, cosine_similarity(&query_emb, emb)))
            .collect();
        
        // Return top-k
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(k);
        scored
    }
}
```

---

# Pattern 1: Benefits

| Before | After |
|--------|-------|
| Agent sees ALL tools | Agent sees top-k relevant |
| O(n) context growth | O(1) context (fixed k) |
| Decision paralysis | Focused choices |
| Manual tool curation | Automatic retrieval |

---

# Pattern 2: Persistent Code as Capabilities

## The Problem

Agent-written code is ephemeral.

## The Solution

**Treat working code as a first-class artifact.**

```
Code that works → Compile to WASM → Store → Reuse
```

---

# Pattern 2: Capability Lifecycle

```
                    ┌─────────────────┐
                    │   New Task      │
                    └────────┬────────┘
                             ▼
                    ┌─────────────────┐
             ┌──────│ Capability      │──────┐
             │  YES │ exists?         │  NO  │
             │      └─────────────────┘      │
             ▼                               ▼
    ┌─────────────────┐            ┌─────────────────┐
    │ run_capability  │            │mutate_capability│
    │ Execute WASM    │            │ LLM writes code │
    │ Deterministic   │            │ Compile to WASM │
    └─────────────────┘            │ Store + Index   │
                                   └────────┬────────┘
                                            │
                                            ▼
                                   Available next time!
```

---

# Pattern 2: What's a Capability?

```json
// meta.json
{
  "id": "get_employee_profile",
  "summary": "Returns basic employee profile information 
              including name, email, department, job title, 
              and employee ID.",
  "binary": "get_employee_profile.wasm"
}
```

- **id**: Unique identifier
- **summary**: Human-readable, used for embedding
- **binary**: WASM executable

---

# Pattern 2: WASM Execution

```rust
pub fn run_capability(&self, cap: &CapabilityRecord, input_json: &str) 
    -> Result<String> 
{
    // Load WASM module
    let module = Module::from_file(&self.engine, &wasm_path)?;
    
    // Set up sandboxed I/O
    let wasi_ctx = WasiCtxBuilder::new()
        .stdin(input_json.as_bytes())
        .stdout(output_pipe)
        .build();
    
    // Run deterministically
    let start = instance.get_typed_func("_start")?;
    start.call(&mut store, ())?;
    
    // Return captured stdout (JSON)
    Ok(stdout)
}
```

---

# Pattern 2: Why WASM?

| Property | Benefit |
|----------|---------|
| **Sandboxed** | Capabilities can't escape their box |
| **Deterministic** | Same input → same output, always |
| **Portable** | Runs anywhere Wasmtime runs |
| **Fast** | Near-native performance |
| **Language-agnostic** | Rust, Go, C, etc. → WASM |

---

# Pattern 2: The Mutation Flow

When no capability exists:

```
1. Agent calls mutate_capability(task, parent_id)
                    │
                    ▼
2. Mutation agent copies parent capability
                    │
                    ▼
3. LLM modifies code to match new task
                    │
                    ▼
4. Compile to wasm32-wasip1
                    │
                    ▼
5. Write meta.json + binary
                    │
                    ▼
6. Reload index with new embedding
                    │
                    ▼
7. Agent can immediately use the new capability
```

---

# The Agent Loop

```rust
pub fn run_task(&mut self, task: &str) -> Result<String> {
    // Build context with only relevant capabilities
    let (caps_summary, _) = store.capabilities_summary_for_task(task, k)?;
    
    let system = format!(
        "You are an agent that MUST solve tasks using capabilities.\n\
         Use run_capability to execute an existing capability.\n\
         Use mutate_capability to create a new one if needed.\n\n\
         Available capabilities:\n{}", 
        caps_summary
    );

    loop {
        let response = llm.chat(messages, tools)?;
        
        match response {
            ToolCall("run_capability", args) => {
                // Execute WASM, deterministic result
            }
            ToolCall("mutate_capability", args) => {
                // Create new capability, reload index
            }
            FinalAnswer(content) => return Ok(content),
        }
    }
}
```

---

# Key Insight: Only 2 Tools

The agent only needs:

| Tool | Purpose |
|------|---------|
| `run_capability` | Execute stored WASM |
| `mutate_capability` | Create new capability |

Everything else is **encoded in capabilities**.

- No "search files" tool → `search_files` capability
- No "call API" tool → `call_api` capability  
- No "query DB" tool → `query_db` capability

---

# Example Scenario

**Task:** "What's John's salary?"

```
Step 1: Embed task
Step 2: Vector search finds: get_salary_details (0.91)
Step 3: Agent sees capability summary
Step 4: Agent calls run_capability("get_salary_details", {"name": "John"})
Step 5: WASM executes → returns {"salary": 85000, "currency": "USD"}
Step 6: Agent returns answer
```

**Fully deterministic.** Same task tomorrow → identical execution path.

---

# Example Scenario: Mutation

**Task:** "What's John's salary in GBP?"

```
Step 1: Embed task
Step 2: Vector search finds: get_salary_details (0.87)
Step 3: Agent tries it, but it returns USD
Step 4: Agent calls mutate_capability(
           "Return salary converted to specified currency",
           "get_salary_details"
        )
Step 5: Mutation agent creates get_salary_with_conversion
Step 6: Agent calls run_capability("get_salary_with_conversion", {...})
Step 7: Returns answer
```

**Tomorrow:** get_salary_with_conversion is already available!

---

# What This Enables

## Short-term
- Deterministic execution of solved problems
- Reduced LLM calls (reuse > regenerate)
- Better tool selection via similarity search

## Long-term
- Agents that **accumulate skills over time**
- Organizational knowledge encoded in capabilities
- Less hallucination (execute stored code, don't guess)

---

# What This Is NOT

| ❌ Not This | ✅ Actually This |
|------------|------------------|
| LangChain agent | Custom Rust runtime |
| OpenAI function registry | WASM execution engine |
| Prompt engineering | Capability mutation |
| Fine-tuning | Skill accumulation |
| Document RAG | **Tool RAG** |

---

# Summary

## Pattern 1: Vector Search for Tool Selection
> Embed tasks, match to capabilities, surface only relevant ones.
> RAG for tools, not documents.

## Pattern 2: Persistent Code as Capabilities
> Working code is compiled to WASM and stored.
> Deterministic reuse beats probabilistic regeneration.

---

# The Vision

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   Agents that learn skills through experience               │
│                                                             │
│   Code reuse instead of hallucination                       │
│                                                             │
│   Tooling discovery instead of manual registration          │
│                                                             │
│   Skill accumulation over long horizons                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

# Links

**Repository:** https://github.com/bensincs/self-evolving-agent-runtime

**Stack:**
- Rust + Wasmtime
- Azure OpenAI / Foundry for embeddings + chat
- WASM (wasm32-wasip1) for capability execution

---

# Questions?
