# JARVIS

> A local-first, multi-platform, AI-powered personal computer companion.

[![Phase](https://img.shields.io/badge/Phase-00%20Discovery-blue)](#project-status)
[![Status](https://img.shields.io/badge/Status-Planning-yellow)](#project-status)
[![Platforms](https://img.shields.io/badge/Platforms-Windows%20%7C%20Linux%20%7C%20Android-green)](#platforms)

---

## Vision

JARVIS is not a chatbot. It is an **agentic operating-layer companion** — a real software system capable of:

- Understanding natural language and voice commands
- Breaking complex requests into multi-step plans
- Opening and controlling desktop applications
- Automating browsers (Chrome, Firefox, Edge)
- Managing files and documents
- Maintaining persistent memory across sessions
- Communicating through natural voice (local STT + TTS)
- Operating across Windows, Linux, and Android
- Running **entirely locally** — no cloud required

Inspired by JARVIS from Iron Man, built as a real, installable system.

---

## Architecture

```
User (Voice / Text)
 ↓
JARVIS Daemon (Rust) ← Core orchestrator
 ↓
Agent / Planner (Python) ← AI reasoning
 ↓
Policy Engine (Rust) ← Security boundary
 ↓
Tool Runtime (Rust + Python)
 ↓
Platform Adapter (Windows / Linux / Android)
 ↓
OS / Browser / Application
 ↓
Verification → Memory → TTS Response
```

**The LLM proposes. The Core decides. The Policy authorizes. The Tool executes.**

See [`project/MASTER_PLAN.md`](project/MASTER_PLAN.md) for the full architecture.

---

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Windows 10/11 x64 | 🎯 Primary | First implementation target |
| Ubuntu 22.04/24.04 LTS | 📋 Planned | Phase 15 |
| Android 10+ | 📋 Planned | Phase 16 |

---

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Core Daemon | **Rust** (Tokio, Tonic) |
| AI Orchestration | **Python** (LangGraph, Playwright) |
| Desktop UI | **Tauri + React/TypeScript** |
| Android | **Kotlin + Jetpack Compose** |
| Protocol | **gRPC + Protocol Buffers** |
| Database | **SQLite** (sqlx) |
| LLM Runtime | **llama.cpp** + Ollama |
| STT | **whisper.cpp** |
| Wake Word | **openWakeWord** |
| TTS | **Piper** |
| Browser | **Playwright** |

---

## Project Status

**Current Phase:** Phase 00 — Discovery & Setup  
**Overall Progress:** ~2%  
**Current Task:** Creating project infrastructure

See [`project/PROJECT_STATUS.md`](project/PROJECT_STATUS.md) for detailed status.

---

## Repository Structure

```
jarvis/
├── apps/          # User-facing applications (desktop, android)
├── core/          # Rust: Core daemon (jarvisd)
├── services/      # Independent service processes (AI, speech, vision, browser)
├── crates/        # Rust shared libraries
├── python/        # Python packages (AI, RAG, evaluation)
├── platforms/     # Platform-specific adapters
├── proto/         # Protobuf definitions
├── skills/        # Built-in skills (filesystem, browser, apps)
├── installers/    # Platform packaging
├── tests/         # Test suites
├── scripts/       # Developer scripts
├── docs/          # Architecture documentation
└── project/       # Project management (plans, status, logs)
```

---

## Development Setup

### Prerequisites

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Rust | 1.75+ | Core daemon |
| Python | 3.11+ | AI services |
| Node.js | 20+ | Desktop UI (Tauri) |
| protoc | 24+ | Protobuf compilation |
| Ollama | latest | LLM development |

See [`docs/development-setup.md`](docs/development-setup.md) for full setup instructions.

### Quick Start

```powershell
# Windows
.\scripts\setup.ps1
.\scripts\dev.ps1
```

```bash
# Linux
./scripts/setup.sh
./scripts/dev.sh
```

---

## Planning Documents

| Document | Purpose |
|----------|---------|
| [`project/MASTER_PLAN.md`](project/MASTER_PLAN.md) | Complete system architecture and technology decisions |
| [`project/PHASES.md`](project/PHASES.md) | 22-phase implementation roadmap |
| [`project/MILESTONES.md`](project/MILESTONES.md) | Detailed milestone definitions |
| [`project/ROADMAP.md`](project/ROADMAP.md) | Visual roadmap |
| [`project/PROJECT_STATUS.md`](project/PROJECT_STATUS.md) | Current implementation status |
| [`project/DOCUMENT_INDEX.md`](project/DOCUMENT_INDEX.md) | Inventory of all 23 spec documents |
| [`project/DECISION_LOG.md`](project/DECISION_LOG.md) | Architectural decisions |
| [`project/RISK_REGISTER.md`](project/RISK_REGISTER.md) | Risk tracking |
| [`project/TRACEABILITY.md`](project/TRACEABILITY.md) | Requirements traceability |
| [`docs/adr/`](docs/adr/) | Architecture Decision Records |

---

## Specification Documents

This project is built from 23 detailed specification documents covering every subsystem. See [`project/DOCUMENT_INDEX.md`](project/DOCUMENT_INDEX.md) for the complete catalogue.

---

## Security

JARVIS is designed with security as a first-class concern:

- The LLM **cannot** directly execute shell commands, access credentials, or control the OS
- All tool calls pass through schema validation → policy engine → audit log
- Credentials are stored in OS keystores (Windows Credential Manager / Linux Secret Service / Android Keystore)
- Sensitive actions require explicit user approval
- Configurable autonomy levels (0 = chat only → 5 = high autonomy)

---

## License

To be determined.

---

*JARVIS — Built with engineering discipline, one milestone at a time.*
