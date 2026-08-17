# JARVIS — Document Index

**Created:** 2026-08-17  
**Status:** COMPLETE — All 23 documents catalogued

---

## Overview

The JARVIS project specification consists of 23 documents totaling approximately 1.1 MB of text covering every aspect of the system architecture, implementation, and deployment.

---

## Document Inventory

| # | Filename | Title | Size | Stage |
|---|----------|-------|------|-------|
| 0 | `Project JARVIS — Local Multiplatform Personal AI Assistant_ Master Architecture & Development Blueprint.md` | Master Architecture & Development Blueprint | 75 KB | Phase 0 |
| 1 | `Project JARVIS — Document 1_ Core Architecture, Monorepo, Runtime, IPC, Agent Engine and Extensibility Specification.md` | Core Architecture, Monorepo, Runtime, IPC, Agent Engine & Extensibility | 69 KB | Phase 1–3 |
| 2 | `JARVIS_Document_2_Local_AI_Engine.md` | Local AI Engine: LLM + Vision + Speech + Wake Word + TTS + Model Management | 28 KB | Phase 4–5 |
| 3 | `JARVIS_Document_3_Agent_Core.md` | Agent Core: Planning, Tool Calling, Memory & Autonomous Task Execution | 41 KB | Phase 10 |
| 4 | `JARVIS_Document_4_Cross_Platform_OS_Application_Automation.md` | Cross-Platform OS & Application Automation | 42 KB | Phase 6–7 |
| 5 | `JARVIS_Document_5_Browser_and_Web_Agent.md` | Browser & Web Agent | 52 KB | Phase 9 |
| 6 | `JARVIS_Document_6_Memory_Personal_Knowledge_User_Profile.md` | Memory, Personal Knowledge & User Profile System | 49 KB | Phase 12 |
| 7 | `JARVIS_Document_7_Core_Monorepo_Complete_Architecture.md` | Core + Monorepo + Complete Architecture (Implementation-Level) | 62 KB | Phase 1–3 |
| 8 | `JARVIS_Document_8_Local_AI_LLM_Vision_Speech_Model_Management.md` | Local AI Engine: LLM + Vision + Speech + Model Management (Detailed) | 60 KB | Phase 4–5 |
| 9 | `JARVIS_Document_9_Windows_Implementation.md` | Windows Implementation: Desktop Companion, OS Automation, Startup, Security | 40 KB | Phase 14 |
| 10 | `JARVIS_Document_10_Ubuntu_Linux_Implementation.md` | Ubuntu/Linux Implementation: Desktop Companion, Native Automation | 42 KB | Phase 15 |
| 11 | `JARVIS_Document_11_Android_Implementation.md` | Android Implementation: Mobile Companion, Voice, Device Control | 45 KB | Phase 16 |
| 12 | `JARVIS_Document_12_Browser_Computer_Use_Engine.md` | Browser + Computer-Use Engine | 56 KB | Phase 9 |
| 13 | `JARVIS_Document_13_Agent_Planner_Workflow_Engine.md` | Agent / Planner / Workflow Engine | 63 KB | Phase 10 |
| 14 | `JARVIS_Document_14_Security_Permissions_Credential_Architecture.md` | Security, Permissions & Credential Architecture | 59 KB | Phase 11 |
| 15 | `JARVIS_Document_15_Memory_RAG_Personal_Knowledge_Architecture.md` | Memory + RAG + Personal Knowledge Architecture (Detailed) | 58 KB | Phase 12 |
| 16 | `JARVIS_Document_16_Application_Plugin_Skill_System.md` | Application / Plugin / Skill System | 61 KB | Phase 13 |
| 17 | `JARVIS_Document_17_Cross_Device_Communication_and_Synchronization(1).md` | Cross-Device Communication & Synchronization | 36 KB | Phase 17 |
| 18 | `JARVIS_Document_18_Testing_Evaluation_and_Reliability(1) (1).md` | Testing, Evaluation & Reliability Engineering | 35 KB | Phase 20 |
| 19 | `JARVIS_Document_19_Packaging_Startup_Updates_Production_Deployment(1).md` | Packaging, Startup, Updates & Production Deployment | 32 KB | Phase 21 |
| 20 | `JARVIS_Document_20_Security_Hardening_Threat_Model_Privacy_Safety(1).md` | Security Hardening, Threat Model, Privacy & Safety Architecture | 33 KB | Phase 11+20 |
| 21 | `JARVIS_Document_21_Complete_Development_Roadmap_Implementation_Order(1).md` | Complete Development Roadmap + Implementation Order | 37 KB | Phase 0 |
| 22 | `JARVIS_Document_22_API_IPC_Event_Bus_Service_Interfaces.md` | Complete API, IPC, Event Bus & Service Interface Specification | 36 KB | Phase 2 |

---

## Document Dependency Graph

```
Doc 0 (Blueprint)
  └── Doc 1 (Core Architecture)
        └── Doc 7 (Monorepo Architecture) ← authoritative monorepo
              ├── Doc 2 (AI Engine basics)
              │     └── Doc 8 (AI Engine detailed)
              ├── Doc 3 (Agent Core)
              │     └── Doc 13 (Planner/Workflow Engine detailed)
              ├── Doc 4 (OS Automation)
              │     ├── Doc 9 (Windows Implementation)
              │     └── Doc 10 (Linux Implementation)
              ├── Doc 5 (Browser/Web Agent)
              │     └── Doc 12 (Browser/Computer-Use Engine detailed)
              ├── Doc 6 (Memory basics)
              │     └── Doc 15 (Memory/RAG detailed)
              ├── Doc 11 (Android Implementation)
              ├── Doc 14 (Security/Permissions)
              │     └── Doc 20 (Security Hardening detailed)
              ├── Doc 16 (Plugin/Skill System)
              ├── Doc 17 (Cross-Device Mesh)
              ├── Doc 18 (Testing/Evaluation)
              ├── Doc 19 (Packaging/Deployment)
              ├── Doc 21 (Roadmap — AUTHORITATIVE ORDER)
              └── Doc 22 (API/IPC Interfaces)
```

---

## Key Technology Decisions (Extracted)

| Component | Technology | Authority |
|-----------|-----------|-----------|
| Core daemon | **Rust** | Doc 0, 1, 7, 21 (unanimous) |
| AI orchestration | **Python** | Doc 0, 1, 7, 21 (unanimous) |
| Desktop UI | **Tauri + React/TypeScript** | Doc 0, 1, 21 |
| Android | **Kotlin + Jetpack Compose** | Doc 0, 11 |
| Protocol | **Protobuf + gRPC** | Doc 1, 22 |
| Local IPC (Windows) | **Named Pipes** | Doc 22 |
| Local IPC (Linux) | **Unix Domain Sockets** | Doc 22 |
| Database | **SQLite** (encrypted where needed) | Doc 21 |
| LLM Runtime | **llama.cpp** (primary) + **Ollama** (dev) | Doc 2, 8, 21 |
| STT | **whisper.cpp** | Doc 0, 2, 8 |
| Wake Word | **openWakeWord** | Doc 0, 2, 8 |
| TTS | **Piper** | Doc 0, 2, 8 |
| Browser Automation | **Playwright** (CDP/WebDriver) | Doc 5, 12 |
| Embeddings/Vector DB | **sqlite-vss / Qdrant** | Doc 6, 15 |

---

## Conflicts and Reconciliation Notes

### Conflict 1: Doc 2 vs Doc 8
- **Issue:** Doc 2 and Doc 8 both define the "Local AI Engine." Doc 8 is the detailed version.
- **Resolution:** Doc 8 supersedes Doc 2 where they overlap. Both document the same subsystem at different detail levels.

### Conflict 2: Doc 6 vs Doc 15
- **Issue:** Doc 6 and Doc 15 both define the Memory architecture. Doc 15 is the detailed version.
- **Resolution:** Doc 15 supersedes Doc 6 where they overlap.

### Conflict 3: Doc 3 vs Doc 13
- **Issue:** Doc 3 defines the Agent Core; Doc 13 defines the Planner/Workflow Engine in detail.
- **Resolution:** Complementary documents. Doc 13 is the detailed implementation spec.

### Conflict 4: Doc 14 vs Doc 20
- **Issue:** Both address security. Doc 14 focuses on architecture; Doc 20 on hardening/threat model.
- **Resolution:** Both are required. Implement together in Phase 11.

---

*Last updated: 2026-08-17*
