# JARVIS — Master Plan

**Version:** 1.0  
**Created:** 2026-08-17  
**Status:** ACTIVE

---

## 1. Project Vision

JARVIS is a local-first, multi-platform, AI-powered personal computer companion that operates as an intelligent agent capable of understanding natural language and voice commands, reasoning about complex multi-step tasks, controlling desktop applications, automating browsers, managing files, maintaining persistent memory, and communicating through speech — all while running primarily on the user's own hardware.

The final system should feel like a persistent AI companion running on the user's devices — not a chatbot, not a website, but a genuine operating-layer companion.

---

## 2. Non-Goals

- Cloud-first or cloud-dependent operation
- Replacing the OS or existing UI
- Competing with general-purpose cloud assistants
- Storing private data on external servers by default
- Bypassing OS security boundaries
- Autonomous action without configurable approval boundaries

---

## 3. Core Architectural Principle

```
User
 ↓
Voice / UI
 ↓
Core Orchestrator (JARVIS Daemon — Rust)
 ↓
Agent / Planner (Python)
 ↓
Policy / Security Engine (Rust)
 ↓
Tool Runtime (Rust + Python)
 ↓
Platform Adapter (Windows/Linux/Android)
 ↓
Operating System / Browser / Application
 ↓
Observation / Verification
 ↓
Memory / Task State
 ↓
Voice / UI (response)
```

**The LLM proposes. The Core decides. The Policy authorizes. The Tool executes. The Environment changes. The Verifier observes. The Memory records. The TTS communicates.**

The LLM is NOT the authority. It is ONE component.

---

## 4. Platforms

| Platform | Priority | Implementation Language |
|----------|----------|------------------------|
| Windows 10/11 x64 | Primary | Rust (daemon) + Python (AI) + Tauri/React (UI) |
| Ubuntu 22.04/24.04 LTS | Secondary | Rust (daemon) + Python (AI) + Tauri/React (UI) |
| Android 10+ | Tertiary | Kotlin + Jetpack Compose |

---

## 5. Technology Decisions

### 5.1 Core Daemon (jarvisd)
- **Language:** Rust
- **Rationale:** Memory safety for a highly-privileged process; performance; cross-platform; excellent async runtime (Tokio)

### 5.2 AI Orchestration
- **Language:** Python
- **Rationale:** AI/ML ecosystem; LangChain/LangGraph; model integrations; RAG tooling
- **Boundary:** Python NEVER gets direct OS privileges; calls Rust via typed IPC

### 5.3 Desktop UI
- **Framework:** Tauri + React + TypeScript
- **Rationale:** Native performance, small bundle, Rust backend, cross-platform (Windows + Linux)

### 5.4 Android
- **Framework:** Kotlin + Jetpack Compose
- **Rationale:** Native Android; official Google support; Compose for modern UI

### 5.5 Protocol / IPC
- **Primary:** gRPC + Protocol Buffers
- **Streaming:** gRPC streaming / WebSocket
- **Local IPC Windows:** Named Pipes (behind IPC abstraction)
- **Local IPC Linux:** Unix Domain Sockets (behind IPC abstraction)
- **Cross-device:** Authenticated encrypted TCP/TLS (LAN-first)

### 5.6 Database
- **Primary:** SQLite (via sqlx in Rust, sqlite3 in Python)
- **Vector:** sqlite-vss (embedded) → Qdrant (if scale requires)
- **Encryption:** SQLCipher for sensitive data where required

### 5.7 Local LLM Runtime
- **Primary:** llama.cpp (GGUF, CPU + GPU, production)
- **Development:** Ollama (easy model management, local HTTP API)
- **Android:** ONNX Runtime + platform accelerators (fallback: PC-hosted)

### 5.8 Voice Stack
- **Wake Word:** openWakeWord (local, runs continuously)
- **VAD:** WebRTC VAD / Silero VAD
- **STT:** whisper.cpp (cross-platform, CPU+GPU, VAD-integrated)
- **TTS:** Piper (fast local neural TTS, streaming)

### 5.9 Browser Automation
- **Primary:** Playwright (Python) via CDP
- **Secondary:** WebDriver (fallback)

### 5.10 Model Strategy
- **Architecture:** Model hierarchy (Tiny → Main → Specialist)
- **Tiny:** ~3B parameter model for deterministic commands
- **Main:** ~7–14B parameter model for planning, reasoning
- **Vision:** Moondream / LLaVA-style local VLM for screen understanding
- **STT/TTS:** Whisper + Piper (separate models, not LLM)

---

## 6. Repository Structure

```
jarvis/
│
├── apps/
│   ├── desktop/           # Tauri desktop app (Windows + Linux)
│   │   ├── src/           # React/TypeScript frontend
│   │   └── src-tauri/     # Tauri Rust backend
│   ├── android/           # Kotlin/Compose Android app
│   └── tray/              # System tray daemon (optional)
│
├── core/                  # Rust: Central daemon (jarvisd)
│   ├── supervisor/        # Process lifecycle
│   ├── orchestrator/      # Request/task lifecycle
│   ├── event-bus/         # Internal event system
│   ├── policy/            # Permission/policy engine
│   ├── task-engine/       # Task state machine
│   ├── ipc/               # IPC abstraction
│   └── config/            # Configuration management
│
├── services/              # Independent service processes
│   ├── ai/                # Python: AI/LLM orchestration
│   ├── speech/            # Rust/C: Voice pipeline (STT/TTS/Wake)
│   ├── vision/            # Python: Vision/screenshot understanding
│   ├── browser/           # Python: Browser automation
│   ├── tools/             # Rust/Python: Tool runtime
│   ├── memory/            # Python: Memory/RAG/knowledge
│   └── mesh/              # Rust: Device mesh communication
│
├── crates/                # Rust library crates
│   ├── protocol/          # Protobuf-generated types
│   ├── ipc/               # IPC transport abstractions
│   ├── security/          # Crypto, auth utilities
│   ├── logging/           # Structured logging/tracing
│   ├── platform/          # Platform trait definitions
│   └── filesystem/        # Safe filesystem operations
│
├── python/                # Python packages
│   ├── jarvis_ai/         # Main AI orchestration package
│   ├── jarvis_rag/        # RAG/memory package
│   ├── jarvis_eval/       # Evaluation framework
│   └── experiments/       # Research/prototyping
│
├── platforms/             # Platform-specific implementations
│   ├── windows/           # Windows platform adapter (Rust)
│   ├── linux/             # Linux platform adapter (Rust)
│   └── android/           # Android service layer
│
├── proto/                 # Protobuf definitions
│
├── skills/                # Built-in skills/plugins
│   ├── browser/
│   ├── filesystem/
│   ├── applications/
│   ├── productivity/
│   └── jobs/
│
├── installers/            # Platform packaging
│   ├── windows/           # NSIS/WiX installer
│   ├── linux/             # .deb / AppImage
│   └── android/           # APK/AAB
│
├── models/                # Model configs and metadata
├── tests/                 # Test suites
│   ├── unit/
│   ├── integration/
│   ├── e2e/
│   └── evaluation/        # AI evaluation datasets
├── scripts/               # Developer scripts
├── docs/                  # Architecture documentation
└── project/               # Project management
```

---

## 7. Security Model

Security is a first-class architectural concern, not an afterthought.

### 7.1 Trust Hierarchy
```
User (highest trust)
 ↓
Core Daemon (Rust, deterministic — trusted runtime)
 ↓
Policy Engine (authorizes specific actions)
 ↓
Tool Runtime (executes authorized actions only)
 ↓
AI/Python Layer (proposes actions — UNTRUSTED for execution)
 ↓
LLM (reason only — no direct system access)
```

### 7.2 Key Principles
- LLM cannot directly call shell, filesystem, network, or credentials
- All tool calls go through schema validation → policy check → audit log
- Credentials stored in OS credential store (Windows Credential Manager / Linux Secret Service / Android Keystore)
- Sensitive actions require `ASK_USER` before execution
- Prompt injection defenses at every external data boundary
- Configurable autonomy levels (0–5)

---

## 8. Agent Architecture

```
Supervisor
 ├── Planner
 ├── Executor
 ├── Verifier
 └── Specialized Agents:
     ├── Browser Agent
     ├── Desktop Agent
     ├── File Agent
     ├── Memory Agent
     ├── Research Agent
     ├── Job Agent
     └── Communication Agent
```

---

## 9. Memory Architecture

```
User / Environment
 ↓
Observation
 ↓
Memory Classifier
 ├── Ignore
 ├── Short-Term Context (conversation buffer)
 ├── Episodic Memory (events, tasks, conversations)
 ├── Semantic Memory (facts, knowledge)
 ├── Procedural Memory (how-to steps)
 └── Personal Knowledge (user profile, preferences)
 ↓
Storage:
 ├── SQLite (structured data)
 ├── Vector DB (embeddings for RAG)
 └── Document Store (files, artifacts)
 ↓
Retrieval → Context Builder → LLM
```

---

## 10. Voice Architecture

```
Microphone
 ↓
Noise Suppression
 ↓
VAD (Voice Activity Detection)
 ↓
Wake Word (openWakeWord — always-on)
 ↓
whisper.cpp (STT)
 ↓
Intent Router
 ↓
Agent / LLM
 ↓
Response Text
 ↓
Piper (TTS — streaming, sentence-by-sentence)
 ↓
Speaker
```

---

## 11. Device Architecture

```
JARVIS Device Mesh
 ├── Windows Node (primary desktop)
 │   ├── Full LLM inference
 │   ├── Desktop automation
 │   └── Browser control
 ├── Ubuntu Node (optional)
 │   ├── Shared core
 │   └── Linux platform adapter
 └── Android Node (mobile companion)
     ├── Voice interface
     ├── Notification bridge
     ├── Remote PC control
     └── Local AI fallback (small models)
```

Communication: LAN-first, TLS-encrypted, no cloud required

---

## 12. Testing Architecture

| Level | Framework | Scope |
|-------|-----------|-------|
| Unit | Rust: cargo test; Python: pytest | Individual functions/modules |
| Integration | Custom harness | Service-to-service communication |
| Contract | Protobuf schema tests | API contracts |
| E2E | Custom JARVIS test runner | Full voice→action→verify flows |
| Platform | Platform-specific test machines | OS-specific behavior |
| Security | Security unit + threat simulation | Policy, prompt injection |
| AI Evaluation | Evaluation datasets + metrics | Intent, tool, planning quality |
| Performance | Benchmarks | Latency, throughput, resource use |

---

## 13. Source Documents

This plan is derived from all 23 specification documents. See `project/DOCUMENT_INDEX.md` for full inventory.

Primary authoritative documents:
- Master Blueprint (`Doc 0`)
- Doc 1: Core Architecture
- Doc 7: Monorepo Architecture
- Doc 21: Implementation Roadmap
- Doc 22: API/IPC Interfaces

---

*Last updated: 2026-08-17*
