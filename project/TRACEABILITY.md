# JARVIS — Traceability Matrix

**Created:** 2026-08-17  
**Purpose:** Trace every major requirement from source document through to implementation and test.

---

## Status Values

- `NOT_STARTED` — requirement not yet implemented
- `PLANNED` — assigned to a phase/milestone
- `IN_PROGRESS` — actively being implemented
- `IMPLEMENTED` — code exists
- `VERIFIED` — tested and acceptance criteria met
- `BLOCKED` — blocked by dependency or issue
- `DEFERRED` — postponed

---

## Core System Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Implementation | Test | Status |
|--------|-------------|-----------|-------|-----------|----------------|------|--------|
| R-C01 | Local-first operation (no mandatory cloud) | Doc 0, 1, 7 | All | All | — | — | PLANNED |
| R-C02 | LLM proposes; policy authorizes; tool executes | Doc 0, 1, 3, 7, 13, 14 | 3, 7, 11 | M03, M07, M11 | — | — | PLANNED |
| R-C03 | Request/Task/Trace ID on all operations | Doc 22 | 2 | M02.01 | — | — | PLANNED |
| R-C04 | Task persistence across crashes | Doc 1, 7, 21 | 3 | M03.05 | — | — | PLANNED |
| R-C05 | Task cancellation support | Doc 1, 7 | 3 | M03.06 | — | — | PLANNED |
| R-C06 | Crash recovery (restart + resume) | Doc 1, 7, 21 | 3 | M03.07 | — | — | PLANNED |
| R-C07 | Structured logging (all operations traceable) | Doc 7, 22 | 3 | M03.08 | — | — | PLANNED |
| R-C08 | Monorepo structure | Doc 0, 1, 7, 21 | 1 | M01.01 | — | — | PLANNED |
| R-C09 | Model provider abstraction (swappable LLMs) | Doc 0, 2, 8 | 4 | M04.01 | — | — | PLANNED |
| R-C10 | Platform adapter abstraction | Doc 1, 4, 7 | 6 | M06.01 | — | — | PLANNED |

---

## Voice Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-V01 | Wake word detection (local, always-on) | Doc 0, 2, 8 | 5 | M05.02 | PLANNED |
| R-V02 | Voice Activity Detection | Doc 2, 8 | 5 | M05.03 | PLANNED |
| R-V03 | Local speech-to-text (whisper.cpp) | Doc 0, 2, 8 | 5 | M05.04 | PLANNED |
| R-V04 | Local text-to-speech (Piper) | Doc 0, 2, 8 | 5 | M05.05 | PLANNED |
| R-V05 | Streaming TTS (speak before full generation) | Doc 0, 2, 8 | 5 | M05.06 | PLANNED |
| R-V06 | Barge-in / interruption support | Doc 2, 8 | 5 | M05.07 | PLANNED |

---

## AI/LLM Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-A01 | Local LLM inference (llama.cpp) | Doc 2, 8 | 4 | M04.02-03 | PLANNED |
| R-A02 | Model hierarchy (tiny/main/specialist) | Doc 0, 2, 8 | 4 | M04.08 | PLANNED |
| R-A03 | Tool/function calling | Doc 1, 3, 8 | 4 | M04.07 | PLANNED |
| R-A04 | Structured output (JSON mode) | Doc 3, 8 | 4 | M04.07 | PLANNED |
| R-A05 | Hardware-aware model selection | Doc 2, 8 | 4 | M04.04 | PLANNED |
| R-A06 | Streaming LLM output | Doc 2, 8 | 4 | M04.06 | PLANNED |
| R-A07 | Vision model (screenshot understanding) | Doc 2, 8 | 8 | M08.01 | PLANNED |
| R-A08 | OCR capability | Doc 2, 8, 12 | 8 | M08.03 | PLANNED |

---

## Agent / Planning Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-P01 | Intent router (deterministic vs AI) | Doc 3, 13 | 10 | M10.01 | PLANNED |
| R-P02 | Multi-step planning | Doc 3, 13 | 10 | M10.02 | PLANNED |
| R-P03 | Executor (step → tool call) | Doc 3, 13 | 10 | M10.03 | PLANNED |
| R-P04 | Verifier (did action succeed?) | Doc 3, 13 | 10 | M10.04 | PLANNED |
| R-P05 | Agent loop limits (max_steps, max_time) | Doc 13 | 10 | M10.05 | PLANNED |
| R-P06 | Human-in-the-loop (ASK_USER) | Doc 0, 3, 13 | 10 | M10.06 | PLANNED |
| R-P07 | Task persistence through agent loop | Doc 3, 13 | 10 | M10.07 | PLANNED |
| R-P08 | Agent recovery after interruption | Doc 3, 13 | 10 | M10.08 | PLANNED |
| R-P09 | Autonomy levels 0–5 | Doc 13, 14 | 11 | M11.07 | PLANNED |
| R-P10 | Narrate what JARVIS is doing | Doc 0, 3 | 10 | M10.03 | PLANNED |

---

## Security Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-S01 | LLM cannot directly execute shell | Doc 0, 1, 14, 20 | 11 | M11.02 | PLANNED |
| R-S02 | All tool calls schema-validated | Doc 14, 20 | 7, 11 | M07.02 | PLANNED |
| R-S03 | Policy engine (authorize before execute) | Doc 14, 20 | 11 | M11.02 | PLANNED |
| R-S04 | Approval workflow for sensitive actions | Doc 0, 14, 20 | 11 | M11.03 | PLANNED |
| R-S05 | Credentials in OS keystore only | Doc 14, 20 | 11 | M11.04 | PLANNED |
| R-S06 | Audit log (all tool calls) | Doc 14, 20 | 7, 11 | M07.04 | PLANNED |
| R-S07 | Prompt injection defenses | Doc 14, 20 | 11 | M11.06 | PLANNED |
| R-S08 | ASK_USER before consequential external actions | Doc 0, 14 | 11 | M11.03 | PLANNED |

---

## Platform Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-W01 | Windows application launching | Doc 4, 9 | 6 | M06.02 | platforms/windows | test_app_alias_resolution | VERIFIED |
| R-W02 | Windows window management | Doc 4, 9 | 6 | M06.03 | platforms/windows | test_get_platform_info | VERIFIED |
| R-W03 | Windows UI Automation (accessibility tree) | Doc 4, 9 | 8, 14 | M08.04 | — | — | PLANNED |
| R-W04 | Windows keyboard/mouse control | Doc 4, 9 | 14 | M14.02 | — | — | PLANNED |
| R-W05 | Windows auto-start on login | Doc 9 | 14 | M14.03 | — | — | PLANNED |
| R-W06 | Windows Credential Manager | Doc 9, 14 | 11, 14 | M11.04 | — | — | PLANNED |
| R-L01 | Linux application launching | Doc 4, 10 | 6 | M06.02 | platforms/linux | test_application_resolver_aliases | VERIFIED |
| R-L02 | Linux Wayland + X11 support | Doc 4, 10 | 6 | M06.02 | platforms/linux | test_display_server_detection | VERIFIED |
| R-L03 | Linux AT-SPI accessibility | Doc 10 | 15 | M15.04 | — | — | PLANNED |
| R-L04 | Linux systemd startup service | Doc 10 | 15 | M15.05 | — | — | PLANNED |
| R-A01 | Android voice interface | Doc 11 | 16 | M16.02 | PLANNED |
| R-A02 | Android foreground service | Doc 11 | 16 | M16.03 | PLANNED |
| R-A03 | Android PC connection/pairing | Doc 11 | 16 | M16.04 | PLANNED |

---

## Browser Automation Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-B01 | Browser session management | Doc 5, 12 | 9 | M09.01 | PLANNED |
| R-B02 | Navigation and tab control | Doc 5, 12 | 9 | M09.02 | PLANNED |
| R-B03 | DOM element finding | Doc 5, 12 | 9 | M09.03 | PLANNED |
| R-B04 | Form filling | Doc 5, 12 | 9 | M09.04 | PLANNED |
| R-B05 | Login state detection | Doc 5, 12 | 9 | M09.06 | PLANNED |
| R-B06 | CAPTCHA human handoff | Doc 5, 12 | 9, 18 | M09 | PLANNED |
| R-B07 | Vision fallback for inaccessible elements | Doc 5, 12 | 9 | M09.05 | PLANNED |

---

## Memory Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-M01 | Short-term context buffer | Doc 6, 15 | 12 | M12.02 | PLANNED |
| R-M02 | Episodic memory (events/tasks) | Doc 6, 15 | 12 | M12.03 | PLANNED |
| R-M03 | Semantic memory (facts) | Doc 6, 15 | 12 | M12.03 | PLANNED |
| R-M04 | Vector search / RAG | Doc 6, 15 | 12 | M12.04-05 | PLANNED |
| R-M05 | User profile | Doc 6, 15 | 12 | M12.06 | PLANNED |
| R-M06 | Memory privacy controls (delete) | Doc 6, 15 | 12 | M12.07 | PLANNED |
| R-M07 | Cross-device memory sync | Doc 6, 15, 17 | 17 | M17.05 | PLANNED |

---

## Cross-Device Requirements

| Req ID | Requirement | Source Doc | Phase | Milestone | Status |
|--------|-------------|-----------|-------|-----------|--------|
| R-D01 | Device discovery (LAN) | Doc 17 | 17 | M17.01 | PLANNED |
| R-D02 | Secure device pairing | Doc 17 | 17 | M17.02 | PLANNED |
| R-D03 | Encrypted device transport | Doc 17 | 17 | M17.03 | PLANNED |
| R-D04 | Task migration between devices | Doc 17 | 17 | M17.04 | PLANNED |
| R-D05 | Remote confirmation from Android | Doc 17 | 17 | M17.06 | PLANNED |

---

*Last updated: 2026-08-17*
