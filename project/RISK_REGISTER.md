# JARVIS — Risk Register

**Created:** 2026-08-17  
**Status:** ACTIVE — review each session

---

## Risk Scoring

- **Probability:** Low (L) / Medium (M) / High (H)
- **Impact:** Low (L) / Medium (M) / High (H)
- **Priority:** P1 (Critical) / P2 (High) / P3 (Medium) / P4 (Low)

---

## Active Risks

| ID | Risk | Prob | Impact | Priority | Mitigation | Status |
|----|------|------|--------|----------|-----------|--------|
| R01 | Local LLM quality insufficient for complex planning tasks | M | H | P1 | Model hierarchy (tiny/main/specialist); evaluation framework; fallback paths | MONITORING |
| R02 | whisper.cpp latency too high for natural real-time conversation | M | H | P1 | Benchmark early; consider streaming VAD-triggered STT; model size tuning | MONITORING |
| R03 | Wayland security restricts input injection (Linux) | H | M | P2 | Wayland-compatible tools (wdotool); compositor permissions; accessibility APIs as primary | MONITORING |
| R04 | Android kills foreground service under memory pressure | H | M | P2 | Implement proper foreground notification; use WorkManager for deferrable tasks | MONITORING |
| R05 | Windows UI Automation gaps (apps not implementing accessibility) | M | M | P2 | Vision model fallback for inaccessible apps; OCR as tertiary fallback | MONITORING |
| R06 | Prompt injection via web content / documents | H | H | P1 | Sandboxed context; injection detection; policy layer doesn't trust LLM arguments | ACTIVE |
| R07 | LLM hallucinates tool arguments (wrong paths, URLs, etc.) | M | H | P1 | Schema validation before execution; policy check; audit log; structured outputs | ACTIVE |
| R08 | Browser automation fragility (website changes break selectors) | H | M | P2 | Semantic element finding; accessibility tree preferred over CSS selectors; vision fallback | MONITORING |
| R09 | CAPTCHA blocks autonomous workflows | H | M | P2 | Human-in-the-loop checkpoint; user notification; task pause/resume | PLANNED |
| R10 | GPU VRAM insufficient for target model on user hardware | M | H | P2 | Hardware detection + model selection; CPU-only fallback models; quantization | PLANNED |
| R11 | Privacy violation via uncontrolled memory retention | M | H | P1 | Explicit memory classification; user consent; delete controls; audit trail | PLANNED |
| R12 | Credential exposure in logs/prompts/memory | L | H | P1 | OS keystore only; never log credentials; structured credential request flow | ACTIVE |
| R13 | Model licensing restrictions for commercial/redistribution | M | M | P2 | Use only models with confirmed open licenses; track licenses | MONITORING |
| R14 | Cross-device pairing security (rogue device joins mesh) | L | H | P2 | Certificate pinning; QR-code pairing; device approval flow | PLANNED |
| R15 | Agent loop runs indefinitely (no exit condition) | M | H | P1 | max_steps, max_runtime, max_retries limits; circuit breaker | ACTIVE |
| R16 | Autonomous action causes irreversible side effects | M | H | P1 | ASK_USER before consequential actions; autonomy levels; dry-run mode | ACTIVE |
| R17 | Plugin/skill supply-chain attack | L | H | P2 | Plugin manifest signing; sandboxed execution; permission declarations | PLANNED |
| R18 | SQLite database corruption on crash | L | M | P3 | WAL mode; integrity checks on startup; backup rotation | PLANNED |
| R19 | Windows startup integration causes boot delays | L | M | P3 | Lazy startup; measure boot impact; user control over startup | PLANNED |
| R20 | Android microphone/camera permission denied by user | M | M | P3 | Graceful degradation; push-to-talk fallback; clear permission explanations | PLANNED |

---

## Resolved Risks

*(none yet)*

---

## Risk Review Schedule

- Review R01-R06 every session (critical risks)
- Review all risks every 3 sessions

---

*Last updated: 2026-08-17*
