Absolutely — here’s a clean grounding.md you can drop into your repo so Copilot (and other LLM tools) understand what the system is and how it works.
It captures the architecture, goals, and naming so it doesn’t “forget” the design.

⸻

grounding.md

Self-Evolving Agent Runtime — Grounding Document

This document exists to give Copilot/AI assistants context about this codebase and what we are building so they can generate correct contributions.

⸻

❓ What This Project Is

This is a self-evolving agent runtime in Rust.

The runtime contains: 1. An agent loop using Azure OpenAI / Foundry Chat 2. A registry of capabilities stored on disk 3. A similarity index over capability summaries 4. A runner that executes capabilities as external binaries 5. An embedder for computing text embeddings 6. A future mutation engine that can generate new capabilities

Capabilities are not OpenAI “tools” — they are executable artifacts (e.g. wasm/CLI binaries) that solve a specific task when invoked.

⸻

🧩 High-Level Agent Behavior

The agent loop works like this: 1. User provides a task in natural language 2. The runtime embeds the task and finds nearest relevant capabilities 3. The agent receives:
• the task
• nearest capability summaries
• available tools: run_capability and mutate_capability 4. The agent can:
• call run_capability to execute an existing capability
• call mutate_capability to create a new capability
• return a final natural-language answer

The agent must not solve complex tasks internally unless trivial (e.g. generating JSON).

⸻

🏗️ Filesystem Layout

Capabilities live under a root directory such as:

capabilities/
echo/
meta.json
bin
sort_json/
meta.json
sort_json.wasm

Each capability folder contains:
• meta.json — metadata for the capability
• binary — an executable file (bin or .wasm) invoked by the runtime

Example meta.json:

{
"id": "echo",
"summary": "Echoes stdin directly to stdout.",
"binary": "bin"
}

⸻

🎯 Runtime Roles

RuntimeContext

Holds mutable runtime state the agent can influence:
• list of capabilities
• similarity index for embeddings
• embedder instance
• capability runner
• capabilities root path

Used by the agent loop.

⸻

CapabilityRecord

Represents a capability loaded from disk:
• id: String
• summary: String
• embedding: Option<Vec<f32>>
• binary: Option<String>

⸻

CapabilityRegistry

Loads capabilities from disk by reading meta.json.

⸻

CapabilityRunner

Executes capabilities by invoking their binary via stdin → stdout.

Later this will support executing wasm binaries via Wasmtime.

⸻

Embedder

Produces vector embeddings from text.

Current implementation: MicrosoftFoundryEmbedder.

⸻

CapabilityIndex

Stores embeddings and allows similarity search nearest_for_task.

⸻

MutationEngine (future)

Responsible for creating new capabilities.

Eventually will:
• generate code from an LLM
• compile to wasm (e.g. wasm32-wasi)
• write new meta.json
• add new capability to runtime
• rebuild embeddings and index

⸻

🛠️ OpenAI Tooling

Inside the agent loop we expose two tools:

run_capability(capability_id, input_json)

Executes a capability and returns JSON.

mutate_capability(parent_capability_id?, task_description)

Creates a new capability if none match the task.

After mutation, capabilities are immediately re-indexed so the agent can call them in the next turn.

⸻

🧠 Overall Architecture

                 ┌─────────────────────────────┐
                 │       Agent (LLM)           │
                 │ - reads task                │
                 │ - sees nearest capabilities │
                 │ - calls tools               │
                 └──────────────┬──────────────┘
                                │
            ┌───────────────────┼──────────────────┐
            │                   │                  │

┌──────────▼───────────┐┌──────▼────────────┐┌────▼────────────────┐
│ run_capability ││ mutate_capability││ natural answer │
│ executes binary ││ creates new cap ││ to caller │
└──────────┬───────────┘└─────────┬──────────┘└─────────────────────┘
│ │
┌──────────▼──────────────┐ ┌─────▼────────────────────────────┐
│ CapabilityRunner │ │ MutationEngine │
│ - run WASM / CLI │ │ - generate/clone code │
│ - capture stdout │ │ - compile to wasm │
└──────────┬──────────────┘ │ - write meta.json + binary │
│ └─────┬────────────────────────────┘
┌──────────▼──────────────┐ │
│ CapabilityRegistry │ │
│ CapabilityIndex │◄───────┘
└─────────────────────────┘

⸻

🚫 What This Project Is NOT

To avoid confusion:
• NOT a LangChain agent
• NOT an OpenAI function registry
• NOT a tool that writes its own prompts for you
• NOT about fine-tuning LLMs

It is about agentic skill evolution over time.

⸻

🧪 Short Example Scenario

User task:

“Sort and deduplicate a JSON array.”

Agent steps: 1. Finds sort_json capability via embedding 2. Calls run_capability with input 3. If no sorting capability exists, it calls mutate_capability 4. Mutation engine creates sort_json_mutated_1 5. Agent retries with the new capability 6. Returns final result to the user

⸻

🎯 Goal of the Project

Enable an agent to:
• Discover capabilities
• Reuse them
• Generate new ones when needed
• Accumulate skills over time
• Without human code intervention

⸻

🧩 Current TODOs
• Extract MutationEngine into its own crate
• Support wasm execution via Wasmtime
• LLM-driven codegen for new wasm capabilities
• Versioning / genealogy tracking
• Capability metadata improvements

⸻

🏁 Why This Matters

This architecture allows:
• Agents that learn skills through experience
• Code reuse instead of hallucination
• Tooling discovery instead of manual registration
• Skill accumulation over long horizons

It is structurally different from traditional tool-calling agents.
