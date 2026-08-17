# ADR-0004: SQLite Task Persistence, Schema Migrations, and Crash Durability

- **Status:** Accepted
- **Date:** 2026-08-17
- **Deciders:** Principal Software Architect / Implementation Agent
- **Technical Context:** Document 1, Document 3, Document 22 (Task Engine & Persistence)

---

## Context and Problem Statement

JARVIS task state must survive process restarts, daemon crashes, ungraceful shutdowns, and long-running execution pauses. The task persistence layer must be decoupled from the core Task Engine state machine via an abstract repository trait, support versioned migrations, maintain atomic transaction integrity, and execute crash reconciliation without blocking the Tokio async runtime.

## Decision

1. **Repository Abstraction**: Define `TaskRepository` async trait in `core/task-engine` providing:
   - `save(&self, task: Task) -> Result<(), TaskError>`
   - `get(&self, id: &TaskId) -> Result<Option<Task>, TaskError>`
   - `list(&self) -> Result<Vec<Task>, TaskError>`
   - `get_by_state(&self, state: TaskState) -> Result<Vec<Task>, TaskError>`
   - `get_interrupted_tasks(&self) -> Result<Vec<Task>, TaskError>`
   - `reconcile_crashed_tasks(&self) -> Result<Vec<Task>, TaskError>`
   - `delete(&self, id: &TaskId) -> Result<bool, TaskError>`
2. **Dual Implementation**:
   - `InMemoryTaskRepository`: Lightweight `RwLock<HashMap<TaskId, Task>>` for fast, zero-dependency unit tests.
   - `SqliteTaskRepository`: Production SQLite persistence using `rusqlite` with embedded C library, WAL mode (`PRAGMA journal_mode = WAL;`), foreign keys enabled, and isolated blocking work executed via `tokio::task::spawn_blocking`.
3. **Database Schema & Versioned Migrations**:
   - Migration tracking table: `_migrations` (`version INTEGER PRIMARY KEY`, `name TEXT`, `applied_at_ms INTEGER`).
   - Migration `v001_initial_schema`:
     - `tasks` table with full lifecycle fields, timestamps, retry counts, and metadata JSON.
     - `task_steps` table with foreign key to `tasks(id)` ON DELETE CASCADE.
     - `task_history` table for immutable audit trails on state transitions.
     - Indices on `state`, `created_at_ms`, `origin_request_id`, and `trace_id`.
4. **Crash Recovery Reconciliation**:
   - On daemon startup, `reconcile_crashed_tasks()` discovers any task left in `Running` state when the process died, updates state to `Recovering`, logs diagnostic error context, and records history for recovery policy evaluation.

## Consequences

- **Positive:** Task state is 100% durable across process termination.
- **Positive:** Task Engine and Orchestrator remain completely decoupled from SQLite mechanics.
- **Positive:** Full compatibility with in-memory test mocks and durable production storage.
