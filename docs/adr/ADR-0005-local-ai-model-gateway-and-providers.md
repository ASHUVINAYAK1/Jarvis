# ADR-0005: Local AI Model Gateway, Provider Abstraction, and Model Routing

- **Status:** Accepted
- **Date:** 2026-08-17
- **Deciders:** Principal Software Architect / Implementation Agent
- **Technical Context:** Document 2, Document 8, Document 22 (AI Subsystem & Model Management)

---

## Context and Problem Statement

JARVIS must operate as a local-first, privacy-preserving AI assistant without mandatory cloud dependencies. To support heterogeneous local inference runtimes (e.g. Ollama for development, llama.cpp for embedded GGUF production execution, and mocks for offline CI), the core system must interact with language models through an abstract gateway rather than direct runtime-specific HTTP calls.

## Decision

1. **Provider-Independent Abstraction (`jarvis-ai`)**:
   - Define `ModelProvider` async trait providing:
     - `check_health(&self) -> Result<ProviderHealth, ModelError>`
     - `list_models(&self) -> Result<Vec<ModelInfo>, ModelError>`
     - `generate(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError>`
     - `stream(&self, request: &ModelRequest) -> Result<ModelStream, ModelError>`
2. **First-Class Providers**:
   - `OllamaProvider`: Connects to `http://127.0.0.1:11434` for rapid development, model tags querying, and JSON streaming.
   - `LlamaCppProvider`: Connects to local `llama-server` for optimized GGUF inference with CPU/GPU offloading.
   - `MockModelProvider`: Deterministic in-memory provider with configurable tool calls, latency simulation, and failure triggers.
3. **Category-Based Model Routing (`ModelRouter`)**:
   - Routes requests across categories: `Fast`, `General`, `Reasoning`, `ToolCalling`, `Vision`, and `Embedding`.
   - Implements observable fallback policies when a primary provider is offline or returns an error.
4. **Unified Gateway (`ModelGateway`)**:
   - Exposes high-level helper APIs for single-turn queries (`ask`), conversations (`chat`), token streams (`chat_stream`), tool call extraction (`plan_action`), and schema-constrained JSON extraction (`extract_json`).
5. **Security Isolation**:
   - Local model output is treated as untrusted input. The model proposes structured `ModelToolCall`s; the Core Policy Engine authorizes; the Tool Runtime executes.

## Consequences

- **Positive:** Core Orchestrator, Task Engine, Policy Engine, and HUD are completely decoupled from Ollama/llama.cpp HTTP details.
- **Positive:** Zero cloud AI dependency for local operation.
- **Positive:** 100% testable in offline environments with `MockModelProvider`.
