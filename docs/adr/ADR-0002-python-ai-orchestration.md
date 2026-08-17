# ADR-0002: Use Python for AI Orchestration

**Status:** ACCEPTED  
**Date:** 2026-08-17

---

## Context

JARVIS needs an AI orchestration layer for:
- LLM inference integration (Ollama API, llama.cpp API)
- Agent planning (LangChain/LangGraph or equivalent)
- RAG pipeline (vector search, document ingestion)
- Browser automation (Playwright)
- Vision model integration
- AI evaluation framework

The AI/ML ecosystem is overwhelmingly Python-first. The primary tooling for LLMs, agents, and RAG is Python-based.

---

## Decision

Use **Python** for:
- AI orchestration service
- Model adapter implementations (Ollama, llama.cpp)
- Agent runtime
- RAG pipeline
- Browser automation (Playwright)
- Vision processing
- Memory management
- Evaluation framework

**Critical Security Boundary:** The Python layer **never** receives direct OS privileges. All system calls, file operations, process control, and credential access happen through the Rust IPC boundary. Python calls → gRPC → Rust policy engine → Rust tool execution → OS.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|------------|----------------|
| **Rust-only** | Would require reimplementing LangChain, Playwright bindings, ML tooling in Rust — enormous effort, poor ecosystem |
| **TypeScript/JS** | Weaker AI/ML ecosystem; async model less suitable for CPU-bound inference work |
| **Go** | Limited AI/ML ecosystem; gRPC fine but agent/RAG tooling immature |

---

## Consequences

**Positive:**
- Access to the full Python AI/ML ecosystem
- Rapid prototyping and experimentation
- LangGraph for complex agent workflows
- Playwright for browser automation
- sentence-transformers, chromadb for RAG
- pytest, evaluation frameworks

**Negative:**
- Polyglot codebase (Rust + Python)
- IPC overhead between Python and Rust (acceptable — dominated by inference latency)
- Python dependency management complexity (managed via `uv` or `poetry`)
- GIL (mitigated by async + subprocess model)

---

## References

- Doc 0, Section 8.2
- Doc 21, Section 4
