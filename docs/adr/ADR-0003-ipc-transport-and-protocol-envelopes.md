# ADR-0003: IPC Transport Architecture & Protocol Envelope Design

- **Status:** Accepted
- **Date:** 2026-08-17
- **Deciders:** Principal Software Architect / Implementation Agent
- **Technical Context:** Document 22 (API, IPC, Event Bus, Service Interfaces)

---

## Context and Problem Statement

JARVIS is a modular distributed local system consisting of the Core daemon (Rust), Desktop HUD (Tauri/React), AI runtime (Python), Tool services, and future speech/vision runtimes. These services require a high-throughput, low-latency, bounded, and secure local Inter-Process Communication (IPC) transport that operates seamlessly across Windows (Named Pipes), Linux (Unix Domain Sockets), and In-Memory channels (unit testing and high-speed in-process routing).

## Decision

1. **Protocol Envelope**: Use a strongly-typed, framed wire envelope (`IpcEnvelope`) carrying a `RequestHeader` (`request_id`, `trace_id`, `task_id`, `deadline_ms`, `source`, `destination`), `message_type` (`Command`, `Response`, `Event`, `Health`), and versioning (`protocol_version: 1`).
2. **Framing Protocol**: Use a standard 4-byte Big-Endian length header preceding every payload with a strict 16MB ceiling (`MAX_FRAME_SIZE`) to protect against buffer overflow/DoS.
3. **Transport Layer**: Implement `IpcTransport` trait backed by:
   - **Windows**: `tokio::net::windows::named_pipe` (`\\.\pipe\jarvis_core_ipc`).
   - **Linux**: Unix Domain Sockets.
   - **In-Memory**: Tokio MPSC bidirectional channels (`MemoryTransport`) for deterministic, fast testing.
4. **Service Dispatch**: Implement `CoreIpcServer` hosting the `Orchestrator` and `CoreIpcClient` for typed request/response interaction.

## Consequences

- **Positive:** Full decoupling of core business logic from OS-specific transport mechanics.
- **Positive:** 100% deterministic testability using in-memory transports in automated CI without OS dependencies.
- **Positive:** Strong correlation traceability across all inter-service requests using `request_id` and `trace_id`.
