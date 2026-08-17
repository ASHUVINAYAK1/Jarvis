# ADR-0001: Use Rust for the Core Daemon and Security-Sensitive Components

**Status:** ACCEPTED  
**Date:** 2026-08-17  
**Author:** JARVIS Architecture

---

## Context

JARVIS requires a privileged background daemon (`jarvisd`) that:
- Manages other processes (supervisor)
- Exposes IPC endpoints
- Enforces security policy
- Manages system-level tool execution
- Handles platform APIs (Windows API, Linux syscalls)
- Processes authentication and credentials

This daemon will run with elevated or user-level privileges on Windows and Linux. Any memory safety vulnerability in this component could lead to privilege escalation or data corruption.

---

## Decision

Use **Rust** as the implementation language for:
- The core daemon (`jarvisd`)
- The supervisor process
- The IPC transport layer
- The policy/permission engine
- The tool execution runtime
- Platform adapters (Windows, Linux)
- The cryptographic/credential boundary
- Performance-sensitive components (audio capture, IPC)

---

## Alternatives Considered

| Alternative | Reason Rejected |
|------------|----------------|
| **Go** | Good concurrency but weaker memory safety guarantees in unsafe code; CGo FFI overhead for Windows APIs; larger runtime |
| **C++** | Maximum control and Windows API access but memory unsafety is unacceptable for a privileged, long-running process |
| **Node.js** | Poor systems integration; no memory safety; not suitable for privileged daemon |
| **Java/JVM** | Large runtime; GC pauses unacceptable for latency-sensitive audio/IPC; weak Windows API integration |
| **Python** | Not suitable for privileged systems code; GIL; not memory-safe |
| **Zig** | Promising but immature ecosystem; less tooling for Windows API |

---

## Consequences

**Positive:**
- Memory safety without GC for the most privileged component
- Excellent async runtime (Tokio) for concurrent I/O
- Strong FFI for Windows APIs (`windows-rs`) and Linux syscalls (`nix`)
- gRPC via `tonic`; Protobuf via `prost`
- Growing ecosystem for security-sensitive software

**Negative:**
- Longer compile times
- Steeper learning curve than Python/Go
- Some crates still maturing (especially Windows-specific crates)
- Cannot use Python AI ecosystem directly (must bridge via IPC)

**Boundary:** Python AI orchestration communicates with the Rust daemon via gRPC. Python never gets direct OS privileges.

---

## References

- Doc 0 (Master Blueprint), Section 8.1
- Doc 1 (Core Architecture), Section 8
- Doc 7 (Monorepo Architecture)
- Doc 21 (Roadmap), Section 4
