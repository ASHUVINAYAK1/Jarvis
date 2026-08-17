# JARVIS — Phase Plan

**Version:** 1.0  
**Created:** 2026-08-17  
**Derived from:** Doc 0 (Blueprint), Doc 21 (Roadmap), Doc 22 (IPC), Doc 1 & 7 (Architecture)

---

## Phase Dependency Order

```
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
                                   ↓              ↓
                             Phase 6            Phase 5
                                   ↓
                             Phase 7 → Phase 8 → Phase 9 → Phase 10
                                                              ↓
                                                        Phase 11 ──→ Phase 12
                                                              ↓         ↓
                                                        Phase 13 ←──────┘
                                                              ↓
                             Phase 14 + Phase 15 + Phase 16 (platform)
                                              ↓
                                        Phase 17 (mesh)
                                              ↓
                                        Phase 18 (autonomy)
                                              ↓
                                   Phase 19 (testing/eval)
                                              ↓
                                   Phase 20 (packaging)
                                              ↓
                                   Phase 21 (hardening)
```

---

## Phase 0 — Discovery, Architecture & Project Setup

**Status:** IN_PROGRESS  
**Objective:** Establish all project infrastructure, read all documents, create project management system, establish build environment.

**Input Documents:** All 23  
**Outputs:**
- `project/` directory with all management files
- `docs/architecture/` with ADRs
- `README.md`
- Initial repo structure (no executable code)

**Milestones:**
- M00.01 — Document inventory and reading ✓
- M00.02 — Project management files created ✓
- M00.03 — Architecture Decision Records
- M00.04 — Repository skeleton scaffolding
- M00.05 — Build environment documentation

**Exit Criteria:**
- All 23 documents catalogued
- All project tracking files created
- ADRs documented for major technology choices
- Repository skeleton created
- README complete

---

## Phase 1 — Repository Foundation & Build System

**Status:** NOT_STARTED  
**Objective:** Create the monorepo structure with functioning build system for all languages (Rust, Python, TypeScript/Tauri, Kotlin).

**Input Documents:** Doc 1, Doc 7, Doc 21  
**Dependencies:** Phase 0  
**Outputs:**
- Rust workspace (`Cargo.toml`)
- Python workspace (`pyproject.toml` / `uv`)
- Node/TypeScript workspace (`package.json`)
- Basic CI (`GitHub Actions` or local equivalent)
- Formatters, linters for all languages
- Empty crate/package skeletons for all major components

**Milestones:**
- M01.01 — Rust workspace with all crate stubs
- M01.02 — Python workspace with all package stubs
- M01.03 — TypeScript/Tauri workspace
- M01.04 — CI pipeline (build + lint + test)
- M01.05 — Developer scripts (build.sh, test.sh, dev.sh)

**Exit Criteria:**
- `cargo build` succeeds across all crates
- `python -m pytest` runs (0 tests, 0 failures)
- `npm run build` succeeds for desktop UI skeleton
- CI passes on every component

---

## Phase 2 — Protocol Layer & IPC Infrastructure

**Status:** NOT_STARTED  
**Objective:** Define and implement all inter-service communication protocols. This is the backbone that every other service depends on.

**Input Documents:** Doc 22, Doc 1, Doc 7  
**Dependencies:** Phase 1  
**Outputs:**
- Protobuf definitions for all service messages
- gRPC service definitions
- Rust IPC client/server abstraction (Named Pipes + Unix Sockets)
- Event bus skeleton
- Request/response with request_id, trace_id, task_id
- Protocol versioning

**Milestones:**
- M02.01 — Proto definitions: core messages
- M02.02 — Proto definitions: service interfaces
- M02.03 — Rust IPC transport (Named Pipes / Unix Sockets)
- M02.04 — gRPC service stubs (Rust server)
- M02.05 — Event bus (in-process pub/sub)
- M02.06 — Protocol versioning and compatibility

**Exit Criteria:**
- Can send a typed message from one process to another
- Request ID, trace ID, task ID flow correctly
- Event bus can publish and subscribe
- Protocol test suite passes

---

## Phase 3 — Supervisor & Core Runtime

**Status:** NOT_STARTED  
**Objective:** Build the JARVIS daemon (`jarvisd`) process supervisor and core orchestrator.

**Input Documents:** Doc 1, Doc 7, Doc 21  
**Dependencies:** Phase 2  
**Outputs:**
- `jarvisd` binary (Rust)
- Process supervisor (start/stop/restart/health)
- Service registry
- Task model and lifecycle state machine
- Context manager
- Cancellation support
- Timeouts
- Structured logging (tracing)

**Milestones:**
- M03.01 — Supervisor skeleton (start/stop services)
- M03.02 — Service health model (STARTING/READY/DEGRADED/FAILED)
- M03.03 — Task model (Task struct, TaskId, lifecycle states)
- M03.04 — Task lifecycle state machine
- M03.05 — Task persistence (SQLite)
- M03.06 — Task cancellation and timeout
- M03.07 — Crash recovery (detect → restart → resume)
- M03.08 — Structured logging (request_id, task_id, trace_id)

**Exit Criteria:**
- Crash a service → supervisor restarts it → health returns READY
- Create a task → task persists to SQLite → daemon restarts → task state recovered
- Cancel a task → task transitions to CANCELLED
- All logs include request_id

---

## Phase 4 — Local AI Runtime

**Status:** NOT_STARTED  
**Objective:** Integrate the local LLM, model management, and model gateway.

**Input Documents:** Doc 2, Doc 8  
**Dependencies:** Phase 3  
**Outputs:**
- Model gateway (trait-based, swappable providers)
- Ollama adapter (dev/easy model management)
- llama.cpp adapter (production)
- Hardware detection (CPU/GPU/VRAM)
- Model selection logic
- Streaming LLM output
- Tool/function calling support
- Structured output (JSON mode)
- Model download/verification

**Milestones:**
- M04.01 — `ModelProvider` trait definition
- M04.02 — Ollama adapter implementation
- M04.03 — llama.cpp adapter implementation
- M04.04 — Hardware detection service
- M04.05 — Model registry and manifest
- M04.06 — Streaming generation
- M04.07 — Tool calling schema + parsing
- M04.08 — Model routing (tiny/main/specialist)

**Exit Criteria:**
- Can call LLM with a prompt and receive streaming response
- Tool calls parsed correctly from model output
- Model can be swapped without changing calling code
- Hardware info correctly detected

---

## Phase 5 — Voice Pipeline

**Status:** NOT_STARTED  
**Objective:** Implement the complete local voice pipeline: wake word → VAD → STT → TTS.

**Input Documents:** Doc 2, Doc 8  
**Dependencies:** Phase 3 (supervisor), Phase 4 (AI runtime for optional LLM-enhanced processing)  
**Outputs:**
- Microphone capture service
- openWakeWord integration
- VAD (Voice Activity Detection)
- whisper.cpp STT integration
- Piper TTS integration (streaming, sentence-by-sentence)
- Audio output service
- Barge-in / interruption support

**Milestones:**
- M05.01 — Microphone capture (cross-platform audio)
- M05.02 — Wake word detection (openWakeWord)
- M05.03 — VAD integration
- M05.04 — whisper.cpp STT (local transcription)
- M05.05 — Piper TTS (local synthesis)
- M05.06 — Streaming TTS (speak while generating)
- M05.07 — Barge-in / interruption handling

**Exit Criteria:**
- Say "Jarvis" → wake word detected
- Speak a sentence → transcribed locally
- Text input → spoken aloud via Piper
- Response begins playing before full text generated

---

## Phase 6 — Desktop Platform Foundation

**Status:** NOT_STARTED  
**Objective:** Implement the platform adapter for Windows (primary), establishing the cross-platform abstraction.

**Input Documents:** Doc 4, Doc 7, Doc 9  
**Dependencies:** Phase 3  
**Outputs:**
- `PlatformAdapter` trait (Rust)
- `WindowsPlatformAdapter` implementation
- Application launching / closing
- Window management (focus, resize, minimize/maximize)
- Process management
- Clipboard access
- Screen capture (screenshot)
- Basic notification support

**Milestones:**
- M06.01 — `PlatformAdapter` trait definition
- M06.02 — Windows application launcher
- M06.03 — Window management (WinAPI)
- M06.04 — Process management
- M06.05 — Screenshot capture (Windows)
- M06.06 — Clipboard read/write
- M06.07 — Windows notifications

**Exit Criteria:**
- `open_application("chrome")` → Chrome opens
- `take_screenshot()` → returns image data
- `get_window_list()` → returns current windows
- `focus_window(...)` → correct window focused

---

## Phase 7 — Tool Runtime

**Status:** NOT_STARTED  
**Objective:** Build the Tool Framework: schemas, registration, validation, execution, and audit.

**Input Documents:** Doc 1, Doc 3, Doc 7, Doc 21  
**Dependencies:** Phase 3, Phase 6  
**Outputs:**
- `Tool` trait (Rust)
- Tool registry
- Tool schema validation
- Tool execution pipeline
- Audit logging for every tool call
- First deterministic tools: `get_time`, `get_date`, `say`, `open_application`, `close_application`

**Milestones:**
- M07.01 — `Tool` trait + manifest schema
- M07.02 — Tool registry (register, discover, validate)
- M07.03 — Tool execution pipeline (validate → execute → result)
- M07.04 — Audit logger (ToolStarted, ToolCompleted, ToolFailed)
- M07.05 — Built-in deterministic tools (5 basic tools)
- M07.06 — Tool call error handling and retry

**Exit Criteria:**
- Register a tool → call it via the registry → result returned
- Failed tool → error recorded in audit log
- `open_application` tool → Chrome opens (integration with Phase 6)

---

## *** FIRST VERTICAL SLICE MILESTONE ***

At the end of Phase 7, the first vertical slice should work:

```
CLI: jarvis open_application chrome
→ Tool selected: open_application
→ Policy check: ALLOWED
→ Windows platform adapter: CreateProcess(chrome.exe)
→ Chrome opens
→ Result: SUCCESS
→ CLI: "Chrome is open."
```

No voice yet. No LLM yet. But the architecture works end-to-end.

---

## Phase 8 — Vision & Screen Understanding

**Status:** NOT_STARTED  
**Objective:** Integrate vision capabilities for screenshot understanding, OCR, and UI inspection.

**Input Documents:** Doc 2, Doc 8, Doc 12  
**Dependencies:** Phase 6 (screenshots), Phase 4 (model gateway)  
**Outputs:**
- Vision model provider (local VLM)
- Screenshot → text/description pipeline
- OCR integration (Tesseract + vision model)
- UI element detection from screenshots
- Windows UI Automation (accessibility tree reader)

**Milestones:**
- M08.01 — Vision model provider (moondream/LLaVA local)
- M08.02 — Screenshot → description pipeline
- M08.03 — OCR integration
- M08.04 — Windows UI Automation (accessibility tree)
- M08.05 — Combined structured+visual element identification

**Exit Criteria:**
- Take screenshot → vision model describes it
- Point to UI element → accessibility tree returns element info
- OCR text extraction from screenshot

---

## Phase 9 — Browser Automation Engine

**Status:** NOT_STARTED  
**Objective:** Build the complete browser automation subsystem using Playwright.

**Input Documents:** Doc 5, Doc 12  
**Dependencies:** Phase 7 (tools), Phase 8 (vision fallback)  
**Outputs:**
- Browser process management
- Playwright integration (Python service)
- Navigation, tab management
- DOM inspection and element finding
- Typing, clicking, form filling
- Screenshot capture (browser)
- Login state detection
- Download/upload support
- Browser tool implementations

**Milestones:**
- M09.01 — Browser session management
- M09.02 — Navigation and tab control
- M09.03 — DOM element finding and interaction
- M09.04 — Form filling pipeline
- M09.05 — Visual element fallback (when DOM insufficient)
- M09.06 — Login state detection
- M09.07 — File download/upload
- M09.08 — Browser audit and verification

**Exit Criteria:**
- Open browser → navigate to URL → find element → click/type → verify result
- Login state correctly detected for common sites
- Vision fallback works when DOM inaccessible

---

## Phase 10 — Agent / Planner / Workflow Engine

**Status:** NOT_STARTED  
**Objective:** Build the AI reasoning layer that connects LLM to tools via planning.

**Input Documents:** Doc 3, Doc 13  
**Dependencies:** Phase 4 (LLM), Phase 7 (tools), Phase 9 (browser)  
**Outputs:**
- Agent runtime (Python)
- Intent router (deterministic vs AI path)
- Planner (LLM-driven step generation)
- Executor (step execution via tools)
- Verifier (action verification)
- Agent loop with limits (max_steps, max_retries, timeouts)
- Human-in-the-loop hooks (ASK_USER)
- Task persistence and resumption

**Milestones:**
- M10.01 — Intent router (deterministic vs AI classification)
- M10.02 — Planner (goal → step decomposition via LLM)
- M10.03 — Executor (step → tool call → result)
- M10.04 — Verifier (did the action succeed?)
- M10.05 — Agent loop with safety limits
- M10.06 — Human checkpoint (ASK_USER, CAPTCHA pause)
- M10.07 — Task persistence through agent loop
- M10.08 — Agent recovery after interruption

**Exit Criteria:**
- "Open Chrome and navigate to google.com" → agent plans → executes → verifies → reports
- Agent loop respects max_steps limit
- Task interrupted → persisted → resumed correctly

---

## *** SECOND VERTICAL SLICE MILESTONE ***

At end of Phase 10:

```
User: "JARVIS, open Chrome."
→ Wake word (Phase 5)
→ STT: "open Chrome" (Phase 5)
→ Intent router → deterministic
→ Tool: open_application(chrome)
→ Policy check (Phase 11 — simplified)
→ Chrome opens
→ Verification
→ TTS: "Chrome is open, sir." (Phase 5)
```

This is the first real JARVIS interaction.

---

## Phase 11 — Security, Permissions & Credential Architecture

**Status:** NOT_STARTED  
**Objective:** Implement the full security and permissions layer.

**Input Documents:** Doc 14, Doc 20  
**Dependencies:** Phase 7 (tools must go through policy), Phase 10 (agent must go through policy)  
**Outputs:**
- Permission model (capabilities, scopes, approval levels)
- Policy engine (evaluate tool call against policy)
- Approval workflow (ASK_USER integration)
- Credential manager (OS keystore integration)
- Audit log (persistent, tamper-evident)
- Prompt injection defenses
- Autonomy level configuration (0–5)

**Milestones:**
- M11.01 — Permission model definitions
- M11.02 — Policy engine (evaluate + enforce)
- M11.03 — Approval workflow (block → ask user → proceed/deny)
- M11.04 — Windows Credential Manager integration
- M11.05 — Audit log (persistent SQLite)
- M11.06 — Prompt injection detection
- M11.07 — Autonomy level configuration

**Exit Criteria:**
- High-risk tool call blocked until user approves
- Credential retrieved from OS store without exposing to LLM
- Prompt injection attempt detected and blocked
- All tool calls recorded in audit log

---

## Phase 12 — Memory, RAG & Personal Knowledge

**Status:** NOT_STARTED  
**Objective:** Build the complete memory and retrieval system.

**Input Documents:** Doc 6, Doc 15  
**Dependencies:** Phase 4 (embeddings model), Phase 11 (privacy/security)  
**Outputs:**
- Memory classifier
- Short-term context buffer
- Episodic memory store (SQLite)
- Semantic memory with vector embeddings
- RAG pipeline (retrieval → context injection)
- User profile store
- Memory privacy controls
- Cross-device sync protocol

**Milestones:**
- M12.01 — Memory classifier (what to remember)
- M12.02 — Short-term context buffer
- M12.03 — Episodic memory (SQLite + embeddings)
- M12.04 — Vector search (sqlite-vss)
- M12.05 — RAG pipeline (retrieve → inject → LLM)
- M12.06 — User profile (preferences, facts)
- M12.07 — Memory privacy controls (delete, audit)
- M12.08 — Cross-device memory sync protocol

**Exit Criteria:**
- User states a preference → stored → retrieved in later session
- RAG retrieval improves LLM response quality (evaluation metric)
- User can delete specific memories

---

## Phase 13 — Skill / Plugin System

**Status:** NOT_STARTED  
**Objective:** Build the extensible skill and plugin architecture.

**Input Documents:** Doc 16  
**Dependencies:** Phase 10 (agent), Phase 11 (security), Phase 12 (memory)  
**Outputs:**
- Skill manifest schema
- Skill router
- Built-in skills: filesystem, applications, productivity
- Plugin loader (isolated execution)
- Skill permission model
- Skill development SDK

**Milestones:**
- M13.01 — Skill manifest and schema
- M13.02 — Skill router (match intent → skill)
- M13.03 — Filesystem skill
- M13.04 — Applications skill
- M13.05 — Productivity skill (calendar, notes)
- M13.06 — Plugin loader (sandboxed)
- M13.07 — Skill permission model

**Exit Criteria:**
- "JARVIS, list files in my documents folder" → filesystem skill executes → result spoken
- External plugin loaded and executed in sandbox
- Plugin cannot exceed its declared permissions

---

## Phase 14 — Windows Platform Integration

**Status:** NOT_STARTED  
**Objective:** Complete full Windows platform implementation.

**Input Documents:** Doc 9  
**Dependencies:** Phase 6 (platform foundation), Phase 13 (skills)  
**Outputs:**
- Complete Windows platform adapter
- Windows UI Automation (full accessibility tree)
- Keyboard/mouse control (approved operations)
- Windows startup integration (Task Scheduler / startup folder)
- Windows notification system
- Windows Credential Manager integration (production)
- Windows system tray application

**Milestones:**
- M14.01 — Full Windows UI Automation
- M14.02 — Keyboard/mouse control
- M14.03 — Windows startup (auto-start on login)
- M14.04 — System tray integration
- M14.05 — Windows-specific notifications
- M14.06 — Production credential management

---

## Phase 15 — Ubuntu/Linux Platform Integration

**Status:** NOT_STARTED  
**Objective:** Implement the Linux platform adapter.

**Input Documents:** Doc 10  
**Dependencies:** Phase 6 (platform abstraction), Phase 14 (Windows reference implementation)  
**Outputs:**
- Linux platform adapter (Rust)
- Wayland + X11 support
- AT-SPI accessibility integration
- Linux startup (systemd user unit / XDG autostart)
- D-Bus integration
- Linux notifications (libnotify)
- Linux Secret Service integration

**Milestones:**
- M15.01 — Linux platform adapter (application control)
- M15.02 — Wayland input (wdotool or compositor protocol)
- M15.03 — X11 fallback (xdotool)
- M15.04 — AT-SPI accessibility tree
- M15.05 — Linux startup (systemd user service)
- M15.06 — Linux Secret Service credential store

---

## Phase 16 — Android Application

**Status:** NOT_STARTED  
**Objective:** Build the Android companion app.

**Input Documents:** Doc 11  
**Dependencies:** Phase 17 (device mesh protocol — must be designed first)  
**Outputs:**
- Kotlin/Compose Android app
- Voice interface (wake word + STT + TTS on Android)
- Foreground service for background operation
- PC connection and pairing
- Remote PC control interface
- Local notification display
- Android Keystore credential storage
- Small local model (optional, for offline fallback)

**Milestones:**
- M16.01 — Android app skeleton (Compose)
- M16.02 — Voice interface (Android STT + TTS)
- M16.03 — Foreground service
- M16.04 — PC connection / pairing
- M16.05 — Remote task monitoring
- M16.06 — Android Keystore integration
- M16.07 — Local fallback AI (ONNX small model)

---

## Phase 17 — Cross-Device Mesh

**Status:** NOT_STARTED  
**Objective:** Implement secure device-to-device communication and state synchronization.

**Input Documents:** Doc 17  
**Dependencies:** Phase 14 (Windows), Phase 15 (Linux), Phase 16 (Android)  
**Outputs:**
- Device discovery (LAN mDNS)
- Device pairing (certificate-based)
- Encrypted transport (TLS 1.3)
- Task migration between devices
- Memory synchronization
- Clipboard sharing
- Remote confirmation relay

**Milestones:**
- M17.01 — Device discovery (mDNS/Bonjour)
- M17.02 — Device pairing (cert-pinning)
- M17.03 — Encrypted transport
- M17.04 — Task state sync
- M17.05 — Memory sync (selective, user-controlled)
- M17.06 — Remote confirmation from Android

---

## Phase 18 — Advanced Autonomous Workflows

**Status:** NOT_STARTED  
**Objective:** Build higher-order autonomous task capabilities including job search workflows.

**Input Documents:** Doc 3, Doc 5, Doc 13  
**Dependencies:** All preceding phases  
**Outputs:**
- LinkedIn job search workflow
- Multi-step form filling
- Application tracking memory
- CAPTCHA human-handoff
- Duplicate application prevention
- Workflow library

**Milestones:**
- M18.01 — Job search workflow (browse + filter)
- M18.02 — LinkedIn Easy Apply workflow
- M18.03 — Application state memory
- M18.04 — CAPTCHA detection + user handoff
- M18.05 — Multi-site job workflow

---

## Phase 19 — Reliability, Recovery & Observability

**Status:** NOT_STARTED  
**Objective:** Harden the system for production use.

**Input Documents:** Doc 18  
**Dependencies:** All core phases  
**Outputs:**
- Distributed tracing (all operations traceable)
- Performance metrics collection
- Crash recovery improvements
- Long-running task checkpoints
- Health dashboard
- Diagnostic CLI

---

## Phase 20 — Testing, Evaluation & Security Validation

**Status:** NOT_STARTED  
**Objective:** Comprehensive test suite and AI evaluation framework.

**Input Documents:** Doc 18, Doc 20  
**Dependencies:** All core phases  
**Outputs:**
- Unit test suite (all components)
- Integration test suite
- AI evaluation datasets (intent, tool selection, planning)
- Security test suite (prompt injection, privilege escalation)
- E2E test scenarios
- Performance benchmarks

---

## Phase 21 — Packaging, Startup & Updates

**Status:** NOT_STARTED  
**Objective:** Production deployment packaging for all platforms.

**Input Documents:** Doc 19  
**Dependencies:** All preceding phases  
**Outputs:**
- Windows installer (NSIS/WiX)
- Linux package (.deb + AppImage)
- Android APK/AAB
- Auto-update system
- Model distribution
- Production configuration

---

## Phase 22 — Production Hardening

**Status:** NOT_STARTED  
**Objective:** Final security, performance, and UX hardening before release.

---

## Summary

| Phase | Name | Priority | Est. Effort |
|-------|------|----------|-------------|
| 0 | Discovery & Setup | Critical | 1-2 sessions |
| 1 | Repository & Build | Critical | 1-2 sessions |
| 2 | Protocol & IPC | Critical | 2-3 sessions |
| 3 | Supervisor & Core | Critical | 3-4 sessions |
| 4 | Local AI Runtime | Critical | 3-4 sessions |
| 5 | Voice Pipeline | High | 2-3 sessions |
| 6 | Desktop Platform Foundation | Critical | 2-3 sessions |
| 7 | Tool Runtime | Critical | 2-3 sessions |
| 8 | Vision | High | 2-3 sessions |
| 9 | Browser Automation | High | 3-4 sessions |
| 10 | Agent / Planner | Critical | 4-5 sessions |
| 11 | Security & Permissions | Critical | 3-4 sessions |
| 12 | Memory & RAG | High | 3-4 sessions |
| 13 | Skills & Plugins | High | 3-4 sessions |
| 14 | Windows Integration | High | 2-3 sessions |
| 15 | Linux Integration | Medium | 2-3 sessions |
| 16 | Android | Medium | 4-5 sessions |
| 17 | Device Mesh | Medium | 3-4 sessions |
| 18 | Autonomous Workflows | Future | 3-4 sessions |
| 19 | Reliability | High | 2-3 sessions |
| 20 | Testing & Evaluation | High | 3-4 sessions |
| 21 | Packaging | Medium | 2-3 sessions |
| 22 | Production Hardening | High | 2-3 sessions |

---

*Last updated: 2026-08-17*
