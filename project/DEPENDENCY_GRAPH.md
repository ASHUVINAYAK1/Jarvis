# JARVIS — Dependency Graph

**Created:** 2026-08-17

---

## Phase Dependency Graph

```
Phase 00 (Discovery)
    │
    ▼
Phase 01 (Repository & Build)
    │
    ▼
Phase 02 (Protocol & IPC)  ──────────────────────────────────────────────────────┐
    │                                                                              │
    ▼                                                                              │
Phase 03 (Supervisor & Core Runtime)                                               │
    │                    │                                                         │
    ▼                    ▼                                                         │
Phase 04 (Local AI)   Phase 06 (Desktop Platform Foundation)                       │
    │                    │                                                         │
    │                    ▼                                                         │
    │              Phase 07 (Tool Runtime) ◄── Phase 06                           │
    │                    │                                                         │
Phase 05 (Voice)         │                                                         │
    │ depends: 03,04      │                                                         │
    │                    ▼                                                         │
    │              Phase 08 (Vision) ◄── Phase 04, Phase 06                       │
    │                    │                                                         │
    │                    ▼                                                         │
    │              Phase 09 (Browser) ◄── Phase 07, Phase 08                      │
    │                    │                                                         │
    │                    ▼                                                         │
    └──────────────► Phase 10 (Agent/Planner) ◄── Phase 04, Phase 07, Phase 09   │
                         │                                                         │
                         ▼                                                         │
                    Phase 11 (Security) ◄── Phase 07, Phase 10                    │
                         │                                                         │
                         ▼                                                         │
                    Phase 12 (Memory/RAG) ◄── Phase 04, Phase 11                  │
                         │                                                         │
                         ▼                                                         │
                    Phase 13 (Skills/Plugins) ◄── Phase 10, 11, 12                │
                         │                                                         │
              ┌──────────┤                                                         │
              ▼          ▼          ▼                                              │
         Phase 14    Phase 15    Phase 16 (Android)                                │
         (Windows)  (Linux)         │                                              │
              │          │          │                                              │
              └──────────┤          │                                              │
                         ▼          │                                              │
                    Phase 17 (Device Mesh) ◄── 14, 15, 16                         │
                         │                                                         │
                         ▼                                                         │
                    Phase 18 (Autonomous Workflows) ◄── All preceding             │
                         │                                                         │
                         ▼                                                         │
                    Phase 19 (Reliability) ◄── All preceding                      │
                    Phase 20 (Testing) ◄── All preceding                           │
                    Phase 21 (Packaging) ◄── All preceding                         │
                         │                                                         │
                         ▼                                                         │
                    Phase 22 (Hardening)                                           │
```

---

## Component Dependency Graph

```
Protocol Definitions (proto/)
    │
    ├──────────────────────────────────────────────────────────────────────────┐
    ▼                                                                          ▼
Rust crates/protocol            Python packages/protocol
    │                                    │
    ├── jarvisd (core supervisor)         ├── jarvis_ai (AI orchestration)
    │       │                            │       │
    │       ├── event-bus                │       ├── model adapters (Ollama, llama.cpp)
    │       ├── task-engine              │       ├── agent runtime
    │       ├── config                   │       ├── planner
    │       └── ipc                      │       ├── browser service (Playwright)
    │           │                        │       ├── memory/RAG
    │           └── Named Pipe / UDS     │       └── vision service
    │                   │                │
    │                   └────────────────┘
    │                          │
    ├── crates/platform         │
    │       │                  │
    │       ├── windows/        ▼
    │       └── linux/    gRPC service calls
    │
    ├── crates/security
    ├── crates/logging
    └── services/speech
            │
            ├── openWakeWord
            ├── whisper.cpp
            └── Piper TTS
```

---

## Critical Path

The critical path to the **first vertical slice** (voice command → action → response):

```
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
                                   ↓         ↓
                             Phase 6    Phase 7 → VERTICAL SLICE 1 (no voice yet)
                                              ↓
                             Phase 5 + Phase 10 → VERTICAL SLICE 2 (with voice + LLM)
```

**Vertical Slice 1 (CLI):** Phase 0+1+2+3+6+7
> `jarvis open_application chrome` → Chrome opens

**Vertical Slice 2 (Voice + LLM):** Phase 0+1+2+3+4+5+6+7+10
> "JARVIS, open Chrome" → Wake word → STT → LLM → Tool → Chrome opens → TTS

---

## Language Dependency Graph

```
Rust Components depend on:
    - Cargo workspace
    - tokio (async runtime)
    - tonic (gRPC)
    - prost (protobuf)
    - sqlx (SQLite)
    - tracing (observability)
    - windows-rs (Windows APIs)
    - nix (Linux APIs)

Python Components depend on:
    - gRPC (grpcio)
    - Protobuf (protobuf)
    - LangChain / LangGraph
    - Playwright
    - sqlite3
    - openai (Ollama-compatible API)
    - numpy / transformers

TypeScript/Tauri depends on:
    - Rust (tauri backend)
    - React
    - WebSocket (for daemon events)

Kotlin/Android depends on:
    - Jetpack Compose
    - gRPC (grpc-kotlin)
    - Room (SQLite)
    - OkHttp (network)
```

---

*Last updated: 2026-08-17*
