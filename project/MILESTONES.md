# JARVIS — Milestones

**Created:** 2026-08-17  
**Purpose:** Detailed definition of every milestone across all 22 phases.

---

## Milestone Naming Convention

```
M[Phase][Milestone]
Example: M03.04 = Phase 03, Milestone 4
```

## Task Naming Convention

```
T[Phase].[Milestone].[Task]
Example: T03.04.002 = Phase 03, Milestone 4, Task 2
```

## Status Values

- `NOT_STARTED` — work not begun
- `IN_PROGRESS` — actively being implemented
- `PROTOTYPE` — stub/mock in place (not complete)
- `IMPLEMENTED` — code exists and compiles
- `TESTED` — unit/integration tests pass
- `VERIFIED` — acceptance criteria met
- `COMPLETE` — implemented + tested + verified + documented
- `BLOCKED` — blocked by a dependency or issue
- `DEFERRED` — postponed to a later phase

---

# Phase 00 — Discovery & Setup

## M00.01 — Document Inventory

**Status:** COMPLETE  
**Objective:** Read and catalogue all 23 specification documents.  
**Acceptance Criteria:** All 23 documents inventoried in `DOCUMENT_INDEX.md` with title, purpose, dependencies, and implementation stage.

| Task | Description | Status |
|------|-------------|--------|
| T00.01.001 | List all files in project directory | COMPLETE |
| T00.01.002 | Read Master Blueprint (Doc 0) | COMPLETE |
| T00.01.003 | Read Doc 1 (Core Architecture) | COMPLETE |
| T00.01.004 | Read Doc 2 (AI Engine) | COMPLETE |
| T00.01.005 | Read Doc 3 (Agent Core) | COMPLETE |
| T00.01.006 | Read Doc 4 (OS Automation) | COMPLETE |
| T00.01.007 | Read Doc 5 (Browser Agent) | COMPLETE |
| T00.01.008 | Read Doc 6 (Memory) | COMPLETE |
| T00.01.009 | Read Doc 7 (Monorepo Architecture) | COMPLETE |
| T00.01.010 | Read Doc 8 (AI Engine detailed) | COMPLETE |
| T00.01.011 | Read Doc 9 (Windows) | COMPLETE |
| T00.01.012 | Read Doc 10 (Linux) | COMPLETE |
| T00.01.013 | Read Doc 11 (Android) | COMPLETE |
| T00.01.014 | Read Doc 12 (Browser Engine) | COMPLETE |
| T00.01.015 | Read Doc 13 (Planner) | COMPLETE |
| T00.01.016 | Read Doc 14 (Security) | COMPLETE |
| T00.01.017 | Read Doc 15 (Memory RAG) | COMPLETE |
| T00.01.018 | Read Doc 16 (Plugins) | COMPLETE |
| T00.01.019 | Read Doc 17 (Cross-Device) | COMPLETE |
| T00.01.020 | Read Doc 18 (Testing) | COMPLETE |
| T00.01.021 | Read Doc 19 (Packaging) | COMPLETE |
| T00.01.022 | Read Doc 20 (Security Hardening) | COMPLETE |
| T00.01.023 | Read Doc 21 (Roadmap) | COMPLETE |
| T00.01.024 | Read Doc 22 (IPC/API) | COMPLETE |
| T00.01.025 | Create DOCUMENT_INDEX.md | COMPLETE |

---

## M00.02 — Project Management Files

**Status:** IN_PROGRESS  
**Objective:** Create all persistent project management files.  
**Acceptance Criteria:** All required files exist and are populated.

| Task | Description | Status |
|------|-------------|--------|
| T00.02.001 | Create docs/ and project/ directories | COMPLETE |
| T00.02.002 | Create DOCUMENT_INDEX.md | COMPLETE |
| T00.02.003 | Create MASTER_PLAN.md | COMPLETE |
| T00.02.004 | Create PHASES.md | COMPLETE |
| T00.02.005 | Create PROJECT_STATUS.md | COMPLETE |
| T00.02.006 | Create DECISION_LOG.md | COMPLETE |
| T00.02.007 | Create RISK_REGISTER.md | COMPLETE |
| T00.02.008 | Create NEXT_ACTIONS.md | COMPLETE |
| T00.02.009 | Create BLOCKERS.md | COMPLETE |
| T00.02.010 | Create IMPLEMENTATION_LOG.md | COMPLETE |
| T00.02.011 | Create DEPENDENCY_GRAPH.md | COMPLETE |
| T00.02.012 | Create MILESTONES.md (this file) | IN_PROGRESS |
| T00.02.013 | Create ROADMAP.md | NOT_STARTED |
| T00.02.014 | Create TRACEABILITY.md | NOT_STARTED |

---

## M00.03 — Architecture Decision Records

**Status:** NOT_STARTED  
**Objective:** Document all major architectural decisions as ADRs.

| Task | Description | Status |
|------|-------------|--------|
| T00.03.001 | ADR-0001: Rust for core daemon | NOT_STARTED |
| T00.03.002 | ADR-0002: Python for AI orchestration | NOT_STARTED |
| T00.03.003 | ADR-0003: Tauri + React for desktop UI | NOT_STARTED |
| T00.03.004 | ADR-0004: Kotlin + Compose for Android | NOT_STARTED |
| T00.03.005 | ADR-0005: gRPC + Protobuf for IPC | NOT_STARTED |
| T00.03.006 | ADR-0006: llama.cpp as LLM runtime | NOT_STARTED |

---

## M00.04 — Repository Skeleton

**Status:** NOT_STARTED  
**Objective:** Create the full directory structure without implementation code.

| Task | Description | Status |
|------|-------------|--------|
| T00.04.001 | Create all top-level directories | NOT_STARTED |
| T00.04.002 | Create README.md | NOT_STARTED |
| T00.04.003 | Create .gitignore | NOT_STARTED |
| T00.04.004 | Create .editorconfig | NOT_STARTED |

---

## M00.05 — Build Environment Documentation

**Status:** NOT_STARTED  
**Objective:** Document all prerequisites for development.

| Task | Description | Status |
|------|-------------|--------|
| T00.05.001 | Document Rust toolchain requirements | NOT_STARTED |
| T00.05.002 | Document Python requirements | NOT_STARTED |
| T00.05.003 | Document Node.js/npm requirements | NOT_STARTED |
| T00.05.004 | Document system dependencies (protoc, FFmpeg, etc.) | NOT_STARTED |
| T00.05.005 | Create setup script (scripts/setup.ps1 + setup.sh) | NOT_STARTED |

---

# Phase 01 — Repository Foundation & Build System

## M01.01 — Rust Workspace

**Status:** NOT_STARTED  
**Objective:** Create the Rust monorepo workspace with all crate stubs.  
**Files Expected:**
- `Cargo.toml` (workspace)
- `core/supervisor/Cargo.toml`
- `core/orchestrator/Cargo.toml`
- `core/event-bus/Cargo.toml`
- `core/policy/Cargo.toml`
- `core/task-engine/Cargo.toml`
- `core/ipc/Cargo.toml`
- `core/config/Cargo.toml`
- `crates/protocol/Cargo.toml`
- `crates/security/Cargo.toml`
- `crates/logging/Cargo.toml`
- `crates/platform/Cargo.toml`
- `crates/filesystem/Cargo.toml`
- `services/speech/Cargo.toml`
- `services/mesh/Cargo.toml`
- `platforms/windows/Cargo.toml`
- `platforms/linux/Cargo.toml`

**Acceptance Criteria:** `cargo build --workspace` succeeds (0 errors)

| Task | Description | Status |
|------|-------------|--------|
| T01.01.001 | Create root Cargo.toml workspace | NOT_STARTED |
| T01.01.002 | Create all Rust crate stubs with lib.rs + Cargo.toml | NOT_STARTED |
| T01.01.003 | Verify cargo build passes | NOT_STARTED |
| T01.01.004 | Configure clippy and rustfmt | NOT_STARTED |

---

## M01.02 — Python Workspace

**Status:** NOT_STARTED  
**Objective:** Create Python workspace with all package stubs.

| Task | Description | Status |
|------|-------------|--------|
| T01.02.001 | Create pyproject.toml (workspace) | NOT_STARTED |
| T01.02.002 | Create jarvis_ai package stub | NOT_STARTED |
| T01.02.003 | Create jarvis_rag package stub | NOT_STARTED |
| T01.02.004 | Create jarvis_eval package stub | NOT_STARTED |
| T01.02.005 | Configure black, isort, mypy, pytest | NOT_STARTED |

---

## M01.03 — Tauri / Desktop UI Skeleton

**Status:** NOT_STARTED  
**Objective:** Create working Tauri app skeleton.

| Task | Description | Status |
|------|-------------|--------|
| T01.03.001 | Initialize Tauri + React app | NOT_STARTED |
| T01.03.002 | Create basic layout (chat + status) | NOT_STARTED |
| T01.03.003 | Verify `npm run dev` works | NOT_STARTED |
| T01.03.004 | Configure ESLint + Prettier | NOT_STARTED |

---

## M01.04 — CI Pipeline

**Status:** NOT_STARTED  
**Objective:** Automated build and test on every push.

| Task | Description | Status |
|------|-------------|--------|
| T01.04.001 | Create CI config (GitHub Actions or local) | NOT_STARTED |
| T01.04.002 | CI: Rust (fmt + clippy + test) | NOT_STARTED |
| T01.04.003 | CI: Python (black + mypy + pytest) | NOT_STARTED |
| T01.04.004 | CI: TypeScript (lint + build) | NOT_STARTED |

---

## M01.05 — Developer Scripts

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T01.05.001 | `scripts/build.ps1` (Windows build) | NOT_STARTED |
| T01.05.002 | `scripts/build.sh` (Linux build) | NOT_STARTED |
| T01.05.003 | `scripts/test.ps1` / `test.sh` | NOT_STARTED |
| T01.05.004 | `scripts/dev.ps1` / `dev.sh` (launch dev mode) | NOT_STARTED |

---

# Phase 02 — Protocol Layer & IPC Infrastructure

## M02.01 — Protobuf Definitions (Core Messages)

**Status:** NOT_STARTED  
**Files Expected:**
- `proto/jarvis/core/v1/command.proto`
- `proto/jarvis/core/v1/response.proto`
- `proto/jarvis/core/v1/event.proto`
- `proto/jarvis/core/v1/task.proto`
- `proto/jarvis/core/v1/tool.proto`
- `proto/jarvis/core/v1/permission.proto`

| Task | Description | Status |
|------|-------------|--------|
| T02.01.001 | Define command message | NOT_STARTED |
| T02.01.002 | Define response message | NOT_STARTED |
| T02.01.003 | Define event message | NOT_STARTED |
| T02.01.004 | Define task state message | NOT_STARTED |
| T02.01.005 | Define tool call/result messages | NOT_STARTED |
| T02.01.006 | Define permission request/grant messages | NOT_STARTED |
| T02.01.007 | Generate Rust types (prost) | NOT_STARTED |
| T02.01.008 | Generate Python types (grpcio-tools) | NOT_STARTED |

---

## M02.02 — Protobuf Service Definitions

**Status:** NOT_STARTED  
**Files Expected:**
- `proto/jarvis/services/v1/orchestrator.proto`
- `proto/jarvis/services/v1/ai.proto`
- `proto/jarvis/services/v1/speech.proto`
- `proto/jarvis/services/v1/vision.proto`
- `proto/jarvis/services/v1/tool.proto`
- `proto/jarvis/services/v1/memory.proto`

| Task | Description | Status |
|------|-------------|--------|
| T02.02.001 | Define OrchestratorService | NOT_STARTED |
| T02.02.002 | Define AIService | NOT_STARTED |
| T02.02.003 | Define SpeechService | NOT_STARTED |
| T02.02.004 | Define VisionService | NOT_STARTED |
| T02.02.005 | Define ToolService | NOT_STARTED |
| T02.02.006 | Define MemoryService | NOT_STARTED |

---

## M02.03 — Rust IPC Transport

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T02.03.001 | Define IpcTransport trait (Rust) | NOT_STARTED |
| T02.03.002 | Implement NamedPipeTransport (Windows) | NOT_STARTED |
| T02.03.003 | Implement UnixSocketTransport (Linux) | NOT_STARTED |
| T02.03.004 | IPC integration tests | NOT_STARTED |

---

## M02.04 — gRPC Service Stubs

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T02.04.001 | Rust gRPC server skeleton (tonic) | NOT_STARTED |
| T02.04.002 | Python gRPC client skeleton | NOT_STARTED |
| T02.04.003 | Roundtrip test (Python → Rust → Python) | NOT_STARTED |

---

## M02.05 — Event Bus

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T02.05.001 | In-process event bus (tokio broadcast) | NOT_STARTED |
| T02.05.002 | Event types (TaskStarted, ToolCalled, etc.) | NOT_STARTED |
| T02.05.003 | Event subscription test | NOT_STARTED |

---

# Phase 03 — Supervisor & Core Runtime

## M03.01 — Supervisor Skeleton

**Status:** NOT_STARTED  
**Objective:** A process that can start, monitor, and restart child processes.

| Task | Description | Status |
|------|-------------|--------|
| T03.01.001 | Supervisor struct and main loop | NOT_STARTED |
| T03.01.002 | Process spawning (tokio::process) | NOT_STARTED |
| T03.01.003 | Health check polling | NOT_STARTED |
| T03.01.004 | Restart on crash (with backoff) | NOT_STARTED |
| T03.01.005 | Graceful shutdown | NOT_STARTED |
| T03.01.006 | Supervisor unit tests | NOT_STARTED |

---

## M03.02 — Service Health Model

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.02.001 | ServiceHealth enum (STARTING/READY/DEGRADED/FAILED/STOPPING/STOPPED) | NOT_STARTED |
| T03.02.002 | Health registry | NOT_STARTED |
| T03.02.003 | Health check RPC | NOT_STARTED |
| T03.02.004 | Integration test: crash → restart → READY | NOT_STARTED |

---

## M03.03 — Task Model

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.03.001 | Task struct (id, state, created_at, updated_at, metadata) | NOT_STARTED |
| T03.03.002 | TaskState enum (PENDING/RUNNING/PAUSED/COMPLETED/FAILED/CANCELLED) | NOT_STARTED |
| T03.03.003 | TaskId (UUID-based) | NOT_STARTED |
| T03.03.004 | Task serialization (serde) | NOT_STARTED |

---

## M03.04 — Task Lifecycle State Machine

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.04.001 | State transition rules | NOT_STARTED |
| T03.04.002 | State machine implementation | NOT_STARTED |
| T03.04.003 | State transition event emission | NOT_STARTED |
| T03.04.004 | State machine unit tests | NOT_STARTED |

---

## M03.05 — Task Persistence

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.05.001 | SQLite schema for tasks | NOT_STARTED |
| T03.05.002 | TaskRepository (CRUD) | NOT_STARTED |
| T03.05.003 | WAL mode + integrity checks | NOT_STARTED |
| T03.05.004 | Persistence integration tests | NOT_STARTED |

---

## M03.06 — Task Cancellation & Timeouts

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.06.001 | CancellationToken (tokio_util) | NOT_STARTED |
| T03.06.002 | Task timeout handling | NOT_STARTED |
| T03.06.003 | Cancellation propagation to child tasks | NOT_STARTED |

---

## M03.07 — Crash Recovery

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.07.001 | On startup: load interrupted tasks | NOT_STARTED |
| T03.07.002 | Classify tasks: safe to resume vs. needs review | NOT_STARTED |
| T03.07.003 | Recovery integration test | NOT_STARTED |

---

## M03.08 — Structured Logging

**Status:** NOT_STARTED

| Task | Description | Status |
|------|-------------|--------|
| T03.08.001 | tracing subscriber setup | NOT_STARTED |
| T03.08.002 | request_id, task_id, trace_id in all spans | NOT_STARTED |
| T03.08.003 | JSON log output for production | NOT_STARTED |
| T03.08.004 | Log file rotation | NOT_STARTED |

---

*(Phases 04–22 milestones will be detailed when those phases are approaching. The above phases are the immediate implementation target.)*

---

*Last updated: 2026-08-17*
