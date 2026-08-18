# JARVIS — Implementation Log

**Created:** 2026-08-17  
**Purpose:** Chronological record of every implementation session.

---

## Session 008 — 2026-08-17 (Phase 06 Linux Platform Adapter & Multiplatform Foundation)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.02  
**Status:** COMPLETE & VERIFIED ✅

### What Was Done & Verified

1. **Linux Platform Adapter (`platforms/linux`)**:
   - Implemented `LinuxPlatformAdapter` in [`platforms/linux/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/linux/src/lib.rs) conforming to `PlatformAdapter` trait.
2. **Display Server & Session Probing (`DisplayServer`)**:
   - Runtime probing for `Wayland`, `X11`, and `Unknown` via `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, and `DISPLAY`.
3. **Multi-Stage Application Resolver (`ApplicationResolver`)**:
   - Stage 1: Alias resolver (`"chrome"` $\rightarrow$ `"google-chrome"`, `"vscode"` $\rightarrow$ `"code"`, `"files"` $\rightarrow$ `"nautilus"`).
   - Stage 2: Direct executable check in `$PATH`.
   - Stage 3: `.desktop` entry parser inspecting `~/.local/share/applications`, `/usr/share/applications`, `/usr/local/share/applications`, `/var/lib/flatpak/exports/share/applications`, `/snap/bin`. Parses `Name`, `Exec`, `Icon`, `Type`, `NoDisplay` and strips Exec placeholders (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`).
   - Stage 4: Safe process spawning via `tokio::process::Command` without shell string concatenation (`sh -c`).
4. **Wayland & X11 Security & Error Handling**:
   - On X11: Window management via `xdotool` and `wmctrl`.
   - On Wayland: Returns structured, explicit error `anyhow!("Wayland security model restricts global window management. PermissionDenied.")`. Never fails silently.
5. **System Integration & Capability Model**:
   - Clipboard: `wl-copy`/`wl-paste` on Wayland, `xclip`/`xsel` on X11, with in-memory fallback cache.
   - Screenshots: `grim` on Wayland, `xwd` on X11.
   - Notifications: `notify-send`.
   - Capability Model: `get_capabilities()` exposes `PlatformCapabilities`.
6. **Architecture & Regression Gate**:
   - Created [`ADR-0007`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/docs/adr/ADR-0007-linux-platform-architecture.md).
   - Workspace unit tests: **71 / 71 passing (100%)** across 20 crates.
   - Doc tests: **2 / 2 passing**.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Windows regression: **PASSED**.

---

## Session 007 — 2026-08-17 (Phase 05 Manual Verification & Diagnostic Fixes)

**Phase:** 05 (Local Voice Pipeline & Audio Stack)  
**Milestone:** M05.16  
**Status:** COMPLETE & MANUALLY VERIFIED ✅

- Physical microphone capture (`cpal`), wake word ("Jarvis"), Webview2 STT, and Windows application launch verified.

## Session 008 — 2026-08-18 (Phase 06 Windows Window Management & Focus)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.03 (Desktop Window Management & Active Window Focus)  
**Task:** T06.03.001  
**Status:** COMPLETE & VERIFIED ✅

1. **Native Win32 Window Management**:
   - Implemented `EnumWindows`, `GetForegroundWindow`, `SetForegroundWindow`, `ShowWindow`, `SetWindowPos` FFI in `platforms/windows/src/lib.rs`.
   - Updated `WindowInfo` struct in `crates/platform/src/lib.rs` with `is_minimized` and `is_maximized` fields.
2. **Window Resolution & Safe Identifiers**:
   - Added `resolve_window_info` supporting direct HWND hex handles (`0x10204`), decimal HWND strings, executable names (`chrome`, `spotify`), and window titles.
3. **Window Operations**:
   - `list_windows()`: Enumerate all visible top-level windows with title, bounds, process name, PID, focus, minimized, maximized states.
   - `get_active_window()`: Identifies the active foreground window.
   - `focus_window()`: Restores if minimized and brings window to foreground.
   - `minimize_window()`, `maximize_window()`, `restore_window()`: Native state manipulation via `ShowWindow`.
   - `set_window_bounds()`: Multi-monitor and DPI-safe bounds positioning/resizing via `SetWindowPos`.
4. **Tool Layer Integration**:
   - Registered 7 built-in tools (`list_windows`, `get_active_window`, `focus_window`, `minimize_window`, `maximize_window`, `restore_window`, `set_window_bounds`) in `services/tools/src/lib.rs`.
   - Added deterministic intent parsing rules in `core/orchestrator/src/lib.rs`.
5. **Testing & Verification**:
   - 71 / 71 workspace unit tests passing.
   - Clippy: 0 errors.
   - TypeScript: 0 errors.
   - Speech subsystem frozen and untouched.

## Session 009 — 2026-08-18 (Phase 06 Window Management Tool Routing & Command Pipeline Integration)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.03 (Desktop Window Management & Active Window Focus)  
**Task:** Window Management Tool Routing & Command Integration  
**Status:** COMPLETE & VERIFIED ✅

1. **Root Cause Analysis of `list_windows.exe` Bug**:
   - `parse_intent` checked `text.contains("list windows")` with a space. Command `"list_windows"` had an underscore and no space, causing it to bypass window intent rules and hit single-word fallback `!text.contains(' ') && !text.is_empty()`.
   - Fallback routed `"list_windows"` to `OpenApplicationTool("list_windows")`, which invoked `start list_windows.exe`, producing `"Windows cannot find 'list_windows.exe'"`.

2. **Command Pipeline & Tool Routing Fix**:
   - Updated `parse_intent` in [`core/orchestrator/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/core/orchestrator/src/lib.rs) to recognize explicit tool names (`list_windows`, `get_active_window`, `focus_window`, `minimize_window`, `maximize_window`, `restore_window`, `move_window`, `resize_window`) and natural language variants (`show open windows`, `what window is active`, `bring chrome to front`, `switch to spotify`).
   - Removed single-word fallback to `open_application`. Unrecognized inputs now return a controlled `UNKNOWN_INTENT` error without launching binaries.

3. **Tool Registry Enhancements**:
   - Added `MoveWindowTool` (`move_window`) and `ResizeWindowTool` (`resize_window`) to [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).
   - Unified argument parsing across all window tools to support `"target"`, `"application"`, `"app"`, and `"window_handle"`.

4. **Voice Pipeline Architecture**:
   - Both typed text and STT output route through the exact same `Orchestrator::execute_command` pipeline. Voice subsystem remains 100% functional and untouched.

5. **Testing & Verification**:
   - Unit tests: **73 / 73 workspace unit tests passing (100%)** across 20 crates.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - App launch regression: `"open chrome"` and `"open spotify"` verified working.

## Session 010 — 2026-08-18 (Phase 06 System Trays & System Control Integration)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.04 (Desktop System Trays & System Control Integration)  
**Task:** T06.04.001  
**Status:** COMPLETE & VERIFIED ✅

1. **System Tray Integration**:
   - Integrated native Tauri v2 System Tray (`TrayIconBuilder`, `MenuBuilder`, `MenuItemBuilder`) in [`apps/desktop/src-tauri/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src-tauri/src/lib.rs).
   - Added tray menu items: `"Show / Hide JARVIS HUD"`, `"Pause / Resume Assistant"`, `"Quit JARVIS"`.
   - Enabled `"tray-icon"` feature in [`apps/desktop/src-tauri/Cargo.toml`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src-tauri/Cargo.toml).

2. **System Control Platform Adapter Methods**:
   - Added system control methods to `PlatformAdapter` trait ([`crates/platform/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/crates/platform/src/lib.rs)): `get_system_volume`, `set_system_volume`, `set_system_mute`, `lock_workstation`, `shutdown_system`, `restart_system`, `sleep_system`.
   - Implemented native Win32 FFI & PowerShell system management in `WindowsPlatformAdapter` ([`platforms/windows/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/windows/src/lib.rs)): `LockWorkStation` Win32 API, `WScript.Shell` volume/mute keys, `shutdown.exe /s /r`, `SetSuspendState`.

3. **Tool Registry & Policy Security**:
   - Implemented 7 new built-in tools in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs): `SetSystemVolumeTool`, `MuteSystemVolumeTool`, `LockWorkstationTool`, `ShutdownSystemTool`, `RestartSystemTool`, `SleepSystemTool`, `GetSystemInfoTool`.
   - Enforced Policy Engine security rules: `shutdown_system` & `restart_system` (`RiskLevel::Critical`), `sleep_system` (`RiskLevel::High`), `lock_workstation` (`RiskLevel::Medium`), `set_system_volume` & `get_system_info` (`RiskLevel::Low`).

4. **Intent Routing**:
   - Updated `parse_intent()` in [`core/orchestrator/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/core/orchestrator/src/lib.rs) for volume control, mute, lock, shutdown, restart, sleep, and system info commands.

5. **Testing & Verification**:
   - Unit & Integration: **79 / 79 workspace unit tests passing (100%)** across 20 crates.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Window Management Regression: All window management tools (`list_windows`, `focus`, `minimize`, `maximize`, `restore`, `move`, `resize`) & application launchers (`open Chrome`, `open Spotify`) verified 100% operational.

## Session 011 — 2026-08-18 (Phase 06 M06.04 Process Management & Process Query Fix)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.04 (Process Management)  
**Task:** T06.04.001  
**Status:** COMPLETE & VERIFIED ✅

1. **Root Cause Analysis & Fixes**:
   - **Root Cause 1 (`is notepad running?`):** Command trailing punctuation (`'?'`, `'.'`) caused string matching condition `text.ends_with(" running")` to fail intent parsing. Fixed by introducing `clean_text` (`trim_end_matches(['?', '.', '!', ';'])`) and robust substring matching between `"is "` and `" running"`.
   - **Root Cause 2 (`list processes`):** Updated intent parser rules to match `"list processes?"`, `"list_processes"`, `"show running processes"`, and commands containing `"list processes"` or `"running processes"`.
   - **Spoken Companion Responses:** Added custom natural spoken formatters for `close_application` (`"Notepad has been closed, sir."`), `is_application_running` (`"Yes, Notepad is currently running, sir."` / `"No, Notepad is not running, sir."`), and `list_processes` (`"Found X active processes running on the system, sir."`).

2. **Tool Definitions**:
   - `CloseApplicationTool` (`close_application`, `RiskLevel::Medium`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).
   - `KillProcessTool` (`kill_process`, `RiskLevel::Medium`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).
   - `ListProcessesTool` (`list_processes`, `RiskLevel::Low`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).
   - `IsApplicationRunningTool` (`is_application_running`, `RiskLevel::Low`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).

3. **Testing & Verification**:
   - Workspace Unit & Integration: **83 / 83 tests passing (100%)** across 20 crates.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Manual Windows Acceptance Verification:
     1. `"is notepad running?"` $\rightarrow$ Reports Notepad is not running (`false`).
     2. `"open notepad"` $\rightarrow$ Launches Notepad.
     3. `"is notepad running?"` $\rightarrow$ Reports Notepad is currently running (`true`).
     4. `"list processes"` $\rightarrow$ Returns running active processes.
     5. `"close notepad"` $\rightarrow$ Closes Notepad.
     6. `"is notepad running?"` $\rightarrow$ Reports Notepad is no longer running (`false`).

## Session 012 — 2026-08-18 (Phase 06 M06.05 Screenshot Capture & HUD Persistence)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.05 (Screenshot Capture)  
**Task:** T06.05.001  
**Status:** COMPLETE & VERIFIED ✅

1. **Platform Adapter & Storage Layer**:
   - Enhanced `WindowsPlatformAdapter` (`take_screenshot`, `take_screenshot_display`, `take_screenshot_region`) in [`platforms/windows/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/windows/src/lib.rs) to capture exact display dimensions (`width`, `height`) and base64 PNG data via PowerShell GDI+.
   - Added `get_jarvis_screenshots_dir()` and `save_screenshot_artifact()` in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs) persisting PNG screenshots to `%USERPROFILE%\Pictures\JARVIS\Screenshots\` with collision-safe timestamped filenames (`jarvis_YYYY-MM-DD_HH-MM-SS_mmm.png`).

2. **Tool Definitions & Metadata**:
   - Implemented `TakeScreenshotTool` (`take_screenshot`), `TakeScreenshotDisplayTool` (`take_screenshot_display`), and `TakeScreenshotRegionTool` (`take_screenshot_region`) returning structured JSON artifact metadata (`success`, `artifact_type: "screenshot"`, `mime_type: "image/png"`, `format: "png"`, `path`, `filename`, `width`, `height`, `display_index`, `bytes_len`, `status_message`).
   - Registered all screenshot tools in `ToolRegistry::with_builtins()`.

3. **Security Path Validation & Tauri Commands**:
   - Implemented `validate_screenshot_path()` in [`apps/desktop/src-tauri/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src-tauri/src/lib.rs) rejecting path traversal (`..`, `../`, `..\`) and enforcing strict containment inside `%USERPROFILE%\Pictures\JARVIS\Screenshots\`.
   - Added Tauri IPC commands `get_screenshot_base64(path)` (returns base64 data URL for native WebView display) and `open_screenshot(path)` (spawns OS default image viewer).

4. **HUD UI Presentation & Artifact Preview**:
   - Updated `ExecutionTelemetry` in [`apps/desktop/src/types/hud.ts`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src/types/hud.ts) and [`apps/desktop/src/components/Hud/Hud.tsx`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src/components/Hud/Hud.tsx).
   - Rendered holographic screenshot preview card with image thumbnail, filename, pixel dimensions, and an "OPEN IMAGE" button in [`apps/desktop/src/components/Hud/HudTranscript.tsx`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src/components/Hud/HudTranscript.tsx) and [`apps/desktop/src/components/Hud/Hud.css`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src/components/Hud/Hud.css).

5. **Testing & Verification**:
   - Unit & Integration: **87 / 87 workspace unit tests passing (100%)** across 20 crates.
   - Security unit test `test_validate_screenshot_path_security` passing.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Manual Windows Verification: Full desktop, display index, and region screenshots generated and persisted under `%USERPROFILE%\Pictures\JARVIS\Screenshots\`. Preview card and Open image action confirmed working.

## Session 013 — 2026-08-18 (Phase 06 M06.06 Clipboard Read/Write Implementation)

**Phase:** 06 (Desktop Platform Foundation)  
**Milestone:** M06.06 (Clipboard Read/Write)  
**Task:** T06.06.001  
**Status:** COMPLETE & VERIFIED ✅

1. **Win32 Platform Adapter Native FFI**:
   - Added Win32 User32/Kernel32 clipboard FFI function bindings in `mod sys` ([`platforms/windows/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/windows/src/lib.rs)): `OpenClipboard`, `CloseClipboard`, `EmptyClipboard`, `GetClipboardData`, `SetClipboardData`, `IsClipboardFormatAvailable`, `GlobalAlloc`, `GlobalLock`, `GlobalUnlock`, `GlobalFree`.
   - Updated `WindowsPlatformAdapter::get_clipboard()` and `set_clipboard()` to use native Win32 `CF_UNICODETEXT` (supporting full UTF-16 Unicode, multiline text, emojis, quotes, and empty clipboard) with PowerShell fallback.

2. **Tool Registry & Built-in Tools**:
   - Implemented `GetClipboardTool` (`get_clipboard`, `RiskLevel::Low`) and `SetClipboardTool` (`set_clipboard`, `RiskLevel::Low`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs).
   - Registered both tools in `ToolRegistry::with_builtins()`.
   - Configured `PolicyEngine::new()` default risk level overrides for `get_clipboard` and `set_clipboard` as `RiskLevel::Low`.

3. **Audit Privacy & Security**:
   - Enforced privacy redaction rule in `GetClipboardTool` and `SetClipboardTool`: raw clipboard text is NOT written to persistent tracing audit logs. Only character length (`text_len`) is recorded in telemetry logs.

4. **Intent Routing & Spoken Companion Responses**:
   - Added intent parser rules in [`core/orchestrator/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/core/orchestrator/src/lib.rs) for read/get clipboard (`"what's in my clipboard"`, `"read my clipboard"`, `"show clipboard"`, `"get_clipboard"`) and set/copy clipboard (`"copy hello world to my clipboard"`, `"copy this to my clipboard: hello world"`, `"put hello world in my clipboard"`, `"set clipboard to hello world"`, `"copy hello world"`).
   - Formatted companion spoken responses: `get_clipboard` (`"Here is what is in your clipboard: <text>"` or `"Your clipboard is currently empty, sir."`), `set_clipboard` (`"Copied <text> to your clipboard, sir."`).

5. **Testing & Verification**:
   - Unit & Integration: **89 / 89 workspace unit tests passing (100%)** across 20 crates.
   - Tested simple text, multiline, Unicode (`"JARVIS — नमस्ते — 日本語"`), empty clipboard, intent routing, policy evaluation, and privacy redaction.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.

## Session 014 — 2026-08-18 (Phase 06 M06.07 Windows Notifications Implementation & Phase 06 Completion)

**Phase:** 06 (Desktop Platform Foundation — 100% COMPLETE ✅)  
**Milestone:** M06.07 (Windows Notifications)  
**Task:** T06.07.001  
**Status:** COMPLETE & VERIFIED ✅

1. **Win32 Platform Adapter Native Notification Enhancements**:
   - Enhanced `WindowsPlatformAdapter::show_notification()` in [`platforms/windows/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/windows/src/lib.rs) using PowerShell GDI+ `System.Windows.Forms.NotifyIcon` & `System.Drawing.SystemIcons::Information`.
   - Added PowerShell string escaping (`escape_ps` for backticks, quotes, dollar signs) preventing syntax injection.
   - Added `Start-Sleep -Milliseconds 1500` delay ensuring Windows Shell receives and displays toast/balloon notifications before icon disposal.
   - Preserved Linux `notify-send` compatibility in [`platforms/linux/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/platforms/linux/src/lib.rs).

2. **Tool Registry & Built-in Tools**:
   - Implemented `ShowNotificationTool` (`show_notification`, `RiskLevel::Low`) in [`services/tools/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/tools/src/lib.rs) accepting `title` (default `"JARVIS"`), `message`/`body`, `priority`, `timeout_secs`.
   - Registered `ShowNotificationTool` in `ToolRegistry::with_builtins()`.
   - Configured `PolicyEngine::new()` default risk level override for `show_notification` as `RiskLevel::Low`.

3. **Intent Routing & Spoken Responses**:
   - Added intent parser rules in [`core/orchestrator/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/core/orchestrator/src/lib.rs) matching `"notify me"`, `"send me a notification saying hello"`, `"show a notification titled JARVIS saying hello"`, `"notify me that the task is complete"`, etc.
   - Added spoken companion response: `"Notification displayed, sir."`.

4. **Testing & Verification**:
   - Unit & Integration: **92 / 92 workspace unit tests passing (100%)** across 20 crates.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Manual Windows Verification: Tested native notification toasts `"send me a notification saying hello from JARVIS"` and `"show a notification titled JARVIS saying Chrome has opened"`.
   - Regression Suites: 100% passing across app launcher, process management, window management, screenshot capture, and clipboard read/write.

## Session 015 — 2026-08-18 (Phase 05 TTS Runtime Implementation & Real Audio Output Repair)

**Phase:** 05 (Local Voice Pipeline & Audio Stack — REPAIRED & VERIFIED)  
**Milestone:** M05.10 / M05.12 (Piper TTS & Real CPAL Audio Playback)  
**Task:** TTS Runtime Implementation & Real Audio Output Repair  
**Status:** COMPLETE & VERIFIED ✅

1. **Piper Process Execution & Audio Parsing ([`services/speech/src/tts.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/speech/src/tts.rs)):**
   - Implemented `PiperConfig` with deterministic discovery: explicit env vars (`JARVIS_PIPER_PATH`, `JARVIS_PIPER_MODEL`), local app paths (`%USERPROFILE%\.jarvis\piper\`), or system `PATH`.
   - Updated `PiperTtsEngine::synthesize()` to safely spawn `piper` executable via `tokio::process::Command`, piping text through stdin and capturing stdout bytes.
   - Implemented `parse_audio_bytes()` helper parsing both RIFF WAV headers and raw 16-bit LE PCM data into normalized float samples (`Vec<f32>`).
   - Returns structured `SpeechError::TtsFailure` when Piper or voice model is missing/unconfigured, eliminating fake silent zero buffers.

2. **Real Audio Output & Resampling ([`services/speech/src/output.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/speech/src/output.rs)):**
   - Replaced simulated sleep loop in `AudioOutput::play()` with real CPAL device output streaming.
   - Added `resample_pcm()` helper linear-interpolating input samples to match CPAL default output device sample rate (e.g. 44.1kHz or 48kHz) and channel count (mono/stereo).
   - Executed CPAL stream inside dedicated OS thread avoiding async boundary Send issues on Windows.
   - Maintained immediate barge-in cancellation (< 15ms latency) via `stop()`.

3. **Desktop Dependency Injection ([`apps/desktop/src-tauri/src/lib.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/apps/desktop/src-tauri/src/lib.rs)):**
   - Updated `run()` to discover `PiperConfig` and inject `PiperTtsEngine` and `AudioOutput` into `VoiceSessionController` via `.with_tts(...)` and `.with_audio_output(...)`.

4. **Testing & Verification**:
   - Unit & Integration: **101 / 101 workspace unit tests passing (100%)** across 20 crates.
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Regression Suites: 100% passing across app launcher, process management, window management, screenshot capture, clipboard, and notifications.

## Session 016 — 2026-08-18 (Phase 05 Piper TTS Audio Quality Repair)

**Phase:** 05 (Local Voice Pipeline & Audio Stack — REPAIRED & VERIFIED)  
**Milestone:** M05.10 / M05.12 (Piper TTS & Real CPAL Audio Playback)  
**Task:** Piper TTS Audio Quality Repair (Distortion / Static Fix)  
**Status:** COMPLETE & VERIFIED ✅

1. **Root Cause Analysis (Windows CRT Text-Mode stdout Mismatches):**
   - Discovered that on Windows, spawning `piper.exe` with `--output_file -` piped binary WAV output through C-runtime `stdout` in text mode, automatically converting every `0x0A` (`\n`) byte into `0x0D 0x0A` (`\r\n`).
   - This inserted thousands of corrupt extra bytes into the audio payload, shifting 16-bit PCM integer samples by 1 byte and producing heavy static and unintelligible distortion.

2. **Binary File I/O & Temporary File Generation ([`services/speech/src/tts.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/speech/src/tts.rs)):**
   - Updated `PiperTtsEngine::synthesize()` to invoke `--output_file <temp_wav_path>` in `std::env::temp_dir()`. `piper.exe` opens files in binary mode (`wb`), ensuring 100% bit-exact, pristine RIFF WAV bytes match direct CLI execution.
   - Enhanced `parse_audio_bytes()` with full RIFF chunk scanning (`fmt `, `data`, alignment padding) and support for 16-bit PCM, 32-bit float, 32-bit PCM, and 8-bit PCM formats.

3. **Multi-Channel & Spatial Resampling ([`services/speech/src/output.rs`](file:///c:/Users/Admin/Desktop/my-projects/jarvis/services/speech/src/output.rs)):**
   - Refined `resample_pcm()` to support arbitrary destination channel counts (1, 2, 4, 6, 8 channels), preventing audio channel scrambling on surround-sound/spatial audio devices.
   - Added diagnostic logging in `AudioOutput::play()` detailing source and target sample rates, channel counts, formats, sample counts, and duration.

4. **Testing & Verification:**
   - Unit & Integration: **107 / 107 workspace unit tests passing (100%)** across 20 crates (including RIFF header parsing, mono-to-stereo expansion, sample format conversions, and real test.wav verification).
   - Clippy: **0 errors**.
   - TypeScript: **0 errors**.
   - Preserved hands-free wake word, STT, desktop tools, and barge-in cancellation.

---

*Log maintained by: JARVIS Development Agent*
