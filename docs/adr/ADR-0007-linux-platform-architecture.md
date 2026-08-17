# ADR-0007: Linux Platform Architecture & Multiplatform Desktop Foundation

- **Status:** Accepted
- **Date:** 2026-08-17
- **Deciders:** Principal Software Architect / Implementation Agent
- **Technical Context:** Document 7, Document 9, Document 10 (Linux Platform & Multiplatform Architecture)

---

## Context and Problem Statement

JARVIS requires a multiplatform desktop platform architecture capable of running seamlessly on Linux distributions (Ubuntu 22.04+, 24.04+, GNOME, KDE, XFCE) alongside Windows 10/11. Linux presents heterogeneous display server architectures (X11 vs Wayland), diverse `.desktop` entry standards, multiple package formats (native, Snap, Flatpak), and distinct window management security restrictions. The system must support Linux application launching, process management, clipboard, screen capture, desktop notifications, and capability discovery without breaking the existing `PlatformAdapter` contract or introducing unsafe shell string interpolation.

## Decision

1. **Crate & Modular Architecture (`platforms/linux`)**:
   - Implements `LinuxPlatformAdapter` conforming to the platform-agnostic `PlatformAdapter` trait in `crates/platform`.
   - `DisplayServer`: Runtime session detection for `Wayland`, `X11`, and `Unknown` via environment variables (`XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, `DISPLAY`).
2. **Multi-Stage Application Resolver (`ApplicationResolver`)**:
   - Stage 1: Alias map lookup (`"chrome"` $\rightarrow$ `"google-chrome"`, `"vscode"` $\rightarrow$ `"code"`, `"files"` $\rightarrow$ `"nautilus"`).
   - Stage 2: Direct executable check in `$PATH` (`/usr/bin`, `/usr/local/bin`, `/bin`, `/snap/bin`).
   - Stage 3: `.desktop` entry parser inspecting `~/.local/share/applications`, `/usr/share/applications`, `/usr/local/share/applications`, `/var/lib/flatpak/exports/share/applications`. Safely parses `Name`, `Exec`, `Icon`, `Type`, `NoDisplay` and strips Exec placeholders (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`).
   - Stage 4: Safe process execution via `tokio::process::Command` without shell string concatenation (`sh -c`). Fallback to `xdg-open`.
3. **Wayland & X11 Security & Window Management**:
   - On X11: Window enumeration, focus, minimize, maximize, and set bounds are implemented via `xdotool` and `wmctrl`.
   - On Wayland: Global window manipulation returns explicit structured errors (`anyhow!("Wayland security model restricts global window management. PermissionDenied.")`) rather than failing silently.
4. **Desktop Integration & Capability Model**:
   - Clipboard: Dual backend via `wl-copy`/`wl-paste` on Wayland, `xclip`/`xsel` on X11, with in-memory fallback cache for headless environments.
   - Screen Capture: `grim` on Wayland, `xwd` on X11.
   - Notifications: Desktop notification delivery via `notify-send`.
   - Capability Model: `LinuxPlatformAdapter::get_capabilities()` exposes structured capability reporting (`application_launch`, `window_management`, `screen_capture`, `clipboard`, `notifications`, `global_hotkeys`, `process_management`, `url_open`, `file_open`).

## Consequences

- **Positive:** Clean integration into the existing `PlatformAdapter` abstraction without changing core orchestrator or agent logic.
- **Positive:** Safe application execution preventing shell injection vulnerabilities.
- **Positive:** Graceful Wayland error handling adhering to OS security boundaries.
- **Positive:** Preserves 100% Windows platform adapter compatibility.
