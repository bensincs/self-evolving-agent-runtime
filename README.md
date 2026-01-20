# Self-Evolving Runtime

**Status:** Early Research / Prototype

---

## What is this?

A runtime that gives LLMs **procedural memory** — the ability to create, store, improve, and reuse executable tools across sessions.

Instead of regenerating code every time, agents can:

- **Create** tools from natural language
- **Execute** them safely via WASM
- **Mutate** them to specialize or improve
- **Retrieve** them semantically for new tasks

> **Try → Fail → Adjust → Store → Recall**

---

## Quick Start

```bash
# Coming soon — prototype in progress
cargo build --release
./target/release/runtime --task "parse CSV to JSON"
```

---

## Roadmap

- [ ] Storage layer + tool versioning
- [ ] WASM execution engine
- [ ] Embedding + vector search
- [ ] LLM tool-calling integration
- [ ] Code generation + TDD mutation
- [ ] CLI demonstration

---

## Documentation

See **[PROPOSAL.md](PROPOSAL.md)** for:

- Full architecture
- Design rationale
- API surface
- Storage model
- Mutation strategy

---

## Contributing

Open an issue or reach out if you'd like to collaborate.

---

## License

TBD
