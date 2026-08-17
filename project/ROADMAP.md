# JARVIS — Roadmap

**Created:** 2026-08-17  
**Type:** High-Level Visual Roadmap

---

## Vertical Slice Progress Targets

```
VERTICAL SLICE 1 — CLI Tool Execution (No Voice, No LLM)
Target: End of Phase 7
Test: jarvis open_application chrome → Chrome opens → SUCCESS

VERTICAL SLICE 2 — Full Voice + LLM Command
Target: End of Phase 10
Test: "JARVIS, open Chrome" → Wake → STT → LLM → Tool → Chrome → TTS → "Chrome is open"

VERTICAL SLICE 3 — Browser Automation
Target: End of Phase 10 (with browser tools integrated)
Test: "JARVIS, search YouTube for jazz" → Browser opens → navigates → searches → confirms

VERTICAL SLICE 4 — Memory-Aware Conversation
Target: End of Phase 12
Test: "JARVIS, what did we discuss yesterday?" → Memory retrieval → accurate response

VERTICAL SLICE 5 — Autonomous Multi-Step Workflow
Target: End of Phase 13
Test: "JARVIS, find me 5 SDE jobs on LinkedIn" → browses → filters → reports → awaits approval

VERTICAL SLICE 6 — Cross-Device Operation
Target: End of Phase 17
Test: Say on Android: "JARVIS, open my project folder on the PC" → PC opens folder → Android confirms
```

---

## Phase Roadmap

```
2026 Q3 ─────────────────────────────────────────────────────────────────────────────────►

Phase 00 ████░░░░░░ (40%)
  Discovery, Architecture, Project Setup
  Milestone: M00.01 DONE, M00.02 IN_PROGRESS, M00.03-05 remaining

Phase 01 ░░░░░░░░░░
  Repository Foundation & Build System
  Rust workspace, Python workspace, Tauri skeleton, CI

Phase 02 ░░░░░░░░░░
  Protocol Layer & IPC
  Protobuf, gRPC, Event Bus, Named Pipes / Unix Sockets

Phase 03 ░░░░░░░░░░
  Supervisor & Core Runtime
  jarvisd, task lifecycle, persistence, crash recovery

Phase 04 ░░░░░░░░░░
  Local AI Runtime
  Model gateway, Ollama adapter, llama.cpp adapter, tool calling

Phase 05 ░░░░░░░░░░
  Voice Pipeline
  openWakeWord, whisper.cpp, Piper TTS, VAD

Phase 06 ░░░░░░░░░░
  Desktop Platform Foundation (Windows)
  PlatformAdapter trait, app launcher, window mgmt, screenshots

Phase 07 ░░░░░░░░░░
  Tool Runtime
  Tool trait, registry, execution pipeline, audit log
  ★ VERTICAL SLICE 1: CLI → Chrome opens

Phase 08 ░░░░░░░░░░
  Vision & Screen Understanding
  Vision model, OCR, Windows UI Automation

Phase 09 ░░░░░░░░░░
  Browser Automation Engine
  Playwright, DOM interaction, form filling, login detection

Phase 10 ░░░░░░░░░░
  Agent / Planner / Workflow Engine
  Intent router, planner, executor, verifier, agent loop
  ★ VERTICAL SLICE 2: Voice → LLM → Chrome → TTS
  ★ VERTICAL SLICE 3: Browser automation

Phase 11 ░░░░░░░░░░
  Security, Permissions & Credentials
  Policy engine, approval workflow, OS keystore, audit log

Phase 12 ░░░░░░░░░░
  Memory, RAG & Personal Knowledge
  Memory classifier, SQLite+vectors, RAG pipeline, user profile
  ★ VERTICAL SLICE 4: Memory-aware conversation

Phase 13 ░░░░░░░░░░
  Skills / Plugin System
  Skill manifests, router, filesystem/apps/productivity skills
  ★ VERTICAL SLICE 5: LinkedIn job search workflow

Phase 14 ░░░░░░░░░░
  Windows Platform Integration (full)
  UI Automation, keyboard/mouse, startup, tray, notifications

Phase 15 ░░░░░░░░░░
  Ubuntu/Linux Platform Integration
  Linux platform adapter, Wayland/X11, systemd, AT-SPI

Phase 16 ░░░░░░░░░░
  Android Application
  Kotlin/Compose, voice, foreground service, PC connection

Phase 17 ░░░░░░░░░░
  Cross-Device Mesh
  mDNS discovery, TLS, task migration, memory sync
  ★ VERTICAL SLICE 6: Android → PC command

Phase 18 ░░░░░░░░░░
  Advanced Autonomous Workflows
  LinkedIn apply, multi-step, CAPTCHA handoff

Phase 19 ░░░░░░░░░░
  Reliability, Recovery & Observability
  Distributed tracing, checkpoints, health dashboard

Phase 20 ░░░░░░░░░░
  Testing, Evaluation & Security Validation
  Full test suite, AI evaluation datasets, security tests

Phase 21 ░░░░░░░░░░
  Packaging, Startup & Updates
  Windows installer, .deb/AppImage, APK, auto-update

Phase 22 ░░░░░░░░░░
  Production Hardening
  Final security + performance + UX polish
```

---

## Progress Legend

```
████ = Complete
▓▓▓▓ = In Progress
░░░░ = Not Started
★    = Vertical Slice Milestone
```

---

## Current Position

```
► We are here: Phase 00, Milestone M00.02
```

---

*Last updated: 2026-08-17*
