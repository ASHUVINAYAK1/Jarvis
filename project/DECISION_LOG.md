# JARVIS — Decision Log

**Created:** 2026-08-17  
**Purpose:** Record every major architectural decision with context, alternatives, and rationale.

---

## Decision Format

```
DEC-XXXX
Title:
Date:
Status: ACCEPTED | SUPERSEDED | PROPOSED
Context:
Alternatives Considered:
Decision:
Rationale:
Consequences:
Related ADR:
```

---

## DEC-0001 — Use Rust for Core Daemon

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** The JARVIS daemon will be an extremely privileged process with direct access to OS APIs, keyboard/mouse, credentials, filesystem, and IPC. It needs to be correct, memory-safe, and performant.  
**Alternatives Considered:**
- Go: Good concurrency, but less memory safety guarantees for unsafe operations; weaker FFI story for Windows APIs
- C++: Maximum performance and Windows API coverage; memory unsafety is a critical concern for a privileged process
- Node.js: Poor systems integration; unsuitable for security-sensitive privileged daemon
**Decision:** Use Rust for the core daemon (`jarvisd`) and all security-sensitive components  
**Rationale:** Memory safety for privileged code; excellent async runtime (Tokio); strong FFI for Windows/Linux APIs; good performance; increasingly strong Windows ecosystem  
**Consequences:** Team needs Rust expertise; compilation times are longer; some crates still maturing  
**Related ADR:** ADR-0001

---

## DEC-0002 — Use Python for AI Orchestration

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** AI/ML ecosystem is overwhelmingly Python-first. LangChain, LangGraph, model integration SDKs, RAG tooling, evaluation frameworks are all Python.  
**Alternatives Considered:**
- Rust AI layer: Possible but limited ecosystem; re-implementing LangChain in Rust unnecessary
- Single language (Rust only): Too much friction for AI/ML prototyping and model integration
**Decision:** Python for AI orchestration, model integration, RAG, evaluation  
**Critical Boundary:** Python layer NEVER has direct OS privileges. All system calls go through the Rust IPC boundary  
**Rationale:** Best AI/ML ecosystem; familiar to AI engineers; separation of concerns (Python reasons, Rust executes)  
**Consequences:** Polyglot codebase; IPC overhead (acceptable for AI operations which are latency-dominated by inference anyway)  
**Related ADR:** ADR-0002

---

## DEC-0003 — Use Tauri + React/TypeScript for Desktop UI

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** Desktop UI needs to run on Windows and Linux, integrate with the Rust daemon, and present a modern interface.  
**Alternatives Considered:**
- Electron: Large bundle size; Node.js backend duplicates Rust daemon functionality
- Qt: Powerful but C++ only; complex licensing
- Dear ImGui: Excellent for tools, not for a polished user-facing application
- WPF/WinForms: Windows-only; violates cross-platform requirement
- GTK: Linux-primary; weaker Windows story  
**Decision:** Tauri (Rust + WebView) with React/TypeScript frontend  
**Rationale:** Native performance via Rust backend; WebView rendering for rich UI; cross-platform (Windows + Linux); small bundle; React ecosystem for rapid UI development  
**Consequences:** WebView rendering differences between platforms (minor); requires web tech knowledge  
**Related ADR:** ADR-0003

---

## DEC-0004 — Use Kotlin + Jetpack Compose for Android

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** Android app needs to be a first-class citizen, not a web wrapper. Android has rich native APIs for accessibility, background services, and OS integration.  
**Alternatives Considered:**
- Flutter: Cross-platform but Dart ecosystem; harder Android-native API access
- React Native: JS bridge overhead; harder native API access  
- Kotlin Multiplatform: Share business logic but keep UI native (possible future enhancement)  
**Decision:** Kotlin + Jetpack Compose (native Android)  
**Rationale:** Native Android; official Google tooling; Compose for modern declarative UI; best access to Android APIs (AccessibilityService, Foreground Service, Keystore)  
**Consequences:** Android codebase is separate from desktop; KMM could be added later for shared logic  
**Related ADR:** ADR-0004

---

## DEC-0005 — Use gRPC + Protobuf for Service Communication

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** JARVIS is composed of multiple processes (daemon, AI service, speech service, browser service, UI). These need typed, versioned, documented interfaces.  
**Alternatives Considered:**
- JSON REST: Easy to implement but lacks streaming; no schema enforcement at runtime
- MessagePack: Compact but no IDL; harder to evolve
- Cap'n Proto: Excellent performance but less ecosystem support
- Flatbuffers: Google-backed but less gRPC integration  
**Decision:** Protocol Buffers + gRPC for all service interfaces; WebSocket as streaming fallback for UI  
**Rationale:** Strong typing; code generation in Rust/Python/Kotlin; streaming support; versioning; excellent tooling; cross-language  
**Consequences:** More upfront proto definition work; proto compilation step required  
**Related ADR:** ADR-0005

---

## DEC-0006 — llama.cpp as Primary LLM Runtime

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** Local LLM inference needed for offline operation. Multiple runtimes available.  
**Alternatives Considered:**
- Ollama only: Easy to use but adds a layer; less control over context/batching
- ONNX Runtime: Good for mobile; less flexible for LLM GGUF models on desktop
- vLLM: Excellent throughput but requires Python server + GPU; heavy for personal use
- MLX: macOS-only; not cross-platform  
**Decision:** llama.cpp (production) + Ollama (development/convenience)  
**Rationale:** llama.cpp: GGUF, CPU + GPU, Windows + Linux + Android, quantization, maximum control; Ollama: developer-friendly model management for early development  
**Consequences:** GGUF model format required; quantization tradeoffs need evaluation; llama.cpp server for inter-process calls  
**Related ADR:** ADR-0006

---

## DEC-0007 — SQLite as Primary Database

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** Need to persist tasks, memory, settings, audit logs. Must be local-first.  
**Alternatives Considered:**
- PostgreSQL: Powerful but requires a server process; overkill for local personal assistant
- DuckDB: Excellent analytical queries but less proven for OLTP patterns
- RocksDB: Fast KV store; lacks SQL queries needed for task/memory queries  
**Decision:** SQLite via sqlx (Rust) and sqlite3 (Python). SQLCipher for encrypted databases where needed.  
**Rationale:** Zero-server; embedded; SQLite is the most deployed database engine in the world; sufficient for personal assistant scale; excellent tooling  
**Consequences:** Single-writer concurrency (acceptable for local use); WAL mode for better concurrency  

---

## DEC-0008 — Monorepo Structure

**Date:** 2026-08-17  
**Status:** ACCEPTED  
**Context:** Multiple languages (Rust, Python, TypeScript, Kotlin), multiple services, multiple platforms — all part of one project.  
**Alternatives Considered:**
- Polyrepo: Each service in its own repo; hard to coordinate protocol changes; version mismatch issues
- Partial monorepo: Some things together, some separate; worst of both worlds  
**Decision:** Full monorepo under `jarvis/`  
**Rationale:** Atomic protocol changes; shared CI; easier cross-service refactoring; single source of truth; consistent tooling  
**Consequences:** Larger repo; per-language tooling in same repo; CI must handle all languages  

---

*Last updated: 2026-08-17*
