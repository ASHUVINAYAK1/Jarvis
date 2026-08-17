//! JARVIS Task Engine — SQLite Persistence & Migrations
//!
//! Implements `SqliteTaskRepository` with atomic transactions, versioned migrations,
//! crash recovery reconciliation, and audit history logging.
//!
//! # Architecture
//!
//! ```text
//! TaskEngine
//!     ↓
//! TaskRepository (Trait)
//!     ├── InMemoryTaskRepository (Fast in-process testing)
//!     └── SqliteTaskRepository   (Durable production persistence)
//!           ├── tasks            (Core lifecycle table)
//!           ├── task_steps       (Decomposed execution steps)
//!           ├── task_history     (Immutable state-transition audit log)
//!           └── _migrations      (Deterministic schema versioning)
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 3, Milestones M03.03 → M03.07

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::{
    StepState, Task, TaskError, TaskId, TaskPriority, TaskRepository, TaskState, TaskStep,
};

/// Version identifier for database schema migrations.
pub const CURRENT_SCHEMA_VERSION: i32 = 1;

/// SQLite implementation of `TaskRepository`.
#[derive(Clone)]
pub struct SqliteTaskRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteTaskRepository {
    /// Open an in-memory SQLite repository (ideal for isolated tests).
    pub fn open_in_memory() -> Result<Self, TaskError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| TaskError::Database(format!("Failed to open in-memory SQLite: {}", e)))?;

        let repo = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        repo.init_pragmas_sync()?;
        repo.apply_migrations_sync()?;

        Ok(repo)
    }

    /// Open or create a SQLite repository at the specified file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TaskError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TaskError::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        let conn = Connection::open(p).map_err(|e| {
            TaskError::Database(format!("Failed to open SQLite database at {:?}: {}", p, e))
        })?;

        let repo = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        repo.init_pragmas_sync()?;
        repo.apply_migrations_sync()?;

        info!(path = ?p, "SQLite TaskRepository opened and migrated successfully");
        Ok(repo)
    }

    fn init_pragmas_sync(&self) -> Result<(), TaskError> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
        guard
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|e| TaskError::Database(format!("Failed to set database pragmas: {}", e)))?;
        Ok(())
    }

    fn apply_migrations_sync(&self) -> Result<(), TaskError> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
        let tx = guard.transaction().map_err(|e| {
            TaskError::Database(format!("Failed to start migration transaction: {}", e))
        })?;

        // 1. Create migrations tracking table
        tx.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at_ms INTEGER NOT NULL
            );",
            [],
        )
        .map_err(|e| TaskError::Database(format!("Failed to create _migrations table: {}", e)))?;

        // 2. Query current migration version
        let current_version: Option<i32> = tx
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .optional()
            .map_err(|e| TaskError::Database(format!("Failed to query migration version: {}", e)))?
            .flatten();

        let applied = current_version.unwrap_or(0);

        // 3. Migration v001: Initial Task Engine Schema
        if applied < 1 {
            info!("Applying database migration v001_initial_schema");

            tx.execute_batch(
                "CREATE TABLE tasks (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    original_command TEXT NOT NULL,
                    state TEXT NOT NULL,
                    priority INTEGER NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    started_at_ms INTEGER,
                    completed_at_ms INTEGER,
                    current_step_index INTEGER NOT NULL DEFAULT 0,
                    error_message TEXT,
                    result_summary TEXT,
                    origin_request_id TEXT,
                    trace_id TEXT,
                    parent_task_id TEXT,
                    max_steps INTEGER NOT NULL DEFAULT 50,
                    max_duration_secs INTEGER NOT NULL DEFAULT 300,
                    retry_count INTEGER NOT NULL DEFAULT 0,
                    max_retries INTEGER NOT NULL DEFAULT 3,
                    metadata_json TEXT NOT NULL DEFAULT '{}'
                );

                CREATE TABLE task_steps (
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    step_index INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    tool_name TEXT,
                    arguments_json TEXT,
                    result_json TEXT,
                    state TEXT NOT NULL,
                    started_at_ms INTEGER,
                    completed_at_ms INTEGER,
                    error TEXT,
                    PRIMARY KEY (task_id, step_index)
                );

                CREATE TABLE task_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    state TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    details TEXT,
                    timestamp_ms INTEGER NOT NULL
                );

                CREATE INDEX idx_tasks_state ON tasks(state);
                CREATE INDEX idx_tasks_created_at ON tasks(created_at_ms);
                CREATE INDEX idx_tasks_origin_req ON tasks(origin_request_id);
                CREATE INDEX idx_tasks_trace_id ON tasks(trace_id);
                CREATE INDEX idx_task_history_task_id ON task_history(task_id);",
            )
            .map_err(|e| TaskError::Database(format!("Failed to execute migration v001: {}", e)))?;

            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at_ms) VALUES (1, 'v001_initial_schema', ?1)",
                params![Utc::now().timestamp_millis()],
            )
            .map_err(|e| TaskError::Database(format!("Failed to record migration v001: {}", e)))?;
        }

        tx.commit()
            .map_err(|e| TaskError::Database(format!("Failed to commit migrations: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository {
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    async fn save(&self, task: Task) -> Result<(), TaskError> {
        let task_clone = task.clone();
        let conn_arc = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<(), TaskError> {
            let mut guard = conn_arc.lock().map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let tx = guard
                .transaction()
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let meta_json = serde_json::to_string(&task_clone.metadata)
                .map_err(|e| TaskError::Serialization(e.to_string()))?;

            tx.execute(
                "INSERT INTO tasks (
                    id, name, description, original_command, state, priority,
                    created_at_ms, updated_at_ms, started_at_ms, completed_at_ms,
                    current_step_index, error_message, result_summary,
                    origin_request_id, trace_id, parent_task_id,
                    max_steps, max_duration_secs, retry_count, max_retries, metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    state = excluded.state,
                    priority = excluded.priority,
                    updated_at_ms = excluded.updated_at_ms,
                    started_at_ms = excluded.started_at_ms,
                    completed_at_ms = excluded.completed_at_ms,
                    current_step_index = excluded.current_step_index,
                    error_message = excluded.error_message,
                    result_summary = excluded.result_summary,
                    retry_count = excluded.retry_count,
                    metadata_json = excluded.metadata_json;",
                params![
                    task_clone.id.as_str(),
                    task_clone.name,
                    task_clone.description,
                    task_clone.original_command,
                    task_clone.state.to_string(),
                    task_clone.priority as i32,
                    task_clone.created_at.timestamp_millis(),
                    task_clone.updated_at.timestamp_millis(),
                    task_clone.started_at.map(|t| t.timestamp_millis()),
                    task_clone.completed_at.map(|t| t.timestamp_millis()),
                    task_clone.current_step_index as i64,
                    task_clone.error_message,
                    task_clone.result_summary,
                    task_clone.origin_request_id,
                    task_clone.trace_id,
                    task_clone.parent_task_id.map(|p| p.as_str()),
                    task_clone.max_steps as i64,
                    task_clone.max_duration_secs as i64,
                    task_clone.retry_count as i64,
                    task_clone.max_retries as i64,
                    meta_json,
                ],
            )
            .map_err(|e| TaskError::Database(format!("Failed to insert/update task: {}", e)))?;

            // Persist steps
            for step in &task_clone.steps {
                tx.execute(
                    "INSERT INTO task_steps (
                        task_id, step_index, description, tool_name,
                        arguments_json, result_json, state, started_at_ms, completed_at_ms, error
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    ON CONFLICT(task_id, step_index) DO UPDATE SET
                        description = excluded.description,
                        tool_name = excluded.tool_name,
                        arguments_json = excluded.arguments_json,
                        result_json = excluded.result_json,
                        state = excluded.state,
                        started_at_ms = excluded.started_at_ms,
                        completed_at_ms = excluded.completed_at_ms,
                        error = excluded.error;",
                    params![
                        task_clone.id.as_str(),
                        step.index as i64,
                        step.description,
                        step.tool_name,
                        step.arguments_json,
                        step.result_json,
                        format!("{:?}", step.state),
                        step.started_at.map(|t| t.timestamp_millis()),
                        step.completed_at.map(|t| t.timestamp_millis()),
                        step.error,
                    ],
                )
                .map_err(|e| TaskError::Database(format!("Failed to insert/update task step: {}", e)))?;
            }

            // Record audit history
            tx.execute(
                "INSERT INTO task_history (task_id, state, event_type, details, timestamp_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
                params![
                    task_clone.id.as_str(),
                    task_clone.state.to_string(),
                    "STATE_PERSISTED",
                    task_clone.result_summary.clone().or(task_clone.error_message.clone()),
                    Utc::now().timestamp_millis(),
                ],
            )
            .map_err(|e| TaskError::Database(format!("Failed to record task history: {}", e)))?;

            tx.commit()
                .map_err(|e| TaskError::Database(format!("Failed to commit task save: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| TaskError::Database(format!("Tokio spawn_blocking join error: {}", e)))?
    }

    #[instrument(skip(self), fields(task_id = %id))]
    async fn get(&self, id: &TaskId) -> Result<Option<Task>, TaskError> {
        let task_id_str = id.as_str();
        let conn_arc = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<Option<Task>, TaskError> {
            let guard = conn_arc
                .lock()
                .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;

            let row_res = guard
                .query_row(
                    "SELECT
                    id, name, description, original_command, state, priority,
                    created_at_ms, updated_at_ms, started_at_ms, completed_at_ms,
                    current_step_index, error_message, result_summary,
                    origin_request_id, trace_id, parent_task_id,
                    max_steps, max_duration_secs, retry_count, max_retries, metadata_json
                 FROM tasks WHERE id = ?1",
                    params![task_id_str],
                    |row| {
                        let id_str: String = row.get(0)?;
                        let name: String = row.get(1)?;
                        let description: String = row.get(2)?;
                        let original_command: String = row.get(3)?;
                        let state_str: String = row.get(4)?;
                        let priority_val: i32 = row.get(5)?;
                        let created_ms: i64 = row.get(6)?;
                        let updated_ms: i64 = row.get(7)?;
                        let started_ms: Option<i64> = row.get(8)?;
                        let completed_ms: Option<i64> = row.get(9)?;
                        let current_step: i64 = row.get(10)?;
                        let error_msg: Option<String> = row.get(11)?;
                        let result_sum: Option<String> = row.get(12)?;
                        let origin_req: Option<String> = row.get(13)?;
                        let trace_id: Option<String> = row.get(14)?;
                        let parent_id_str: Option<String> = row.get(15)?;
                        let max_steps: i64 = row.get(16)?;
                        let max_dur: i64 = row.get(17)?;
                        let retry_cnt: i64 = row.get(18)?;
                        let max_ret: i64 = row.get(19)?;
                        let meta_json: String = row.get(20)?;

                        Ok((
                            id_str,
                            name,
                            description,
                            original_command,
                            state_str,
                            priority_val,
                            created_ms,
                            updated_ms,
                            started_ms,
                            completed_ms,
                            current_step,
                            error_msg,
                            result_sum,
                            origin_req,
                            trace_id,
                            parent_id_str,
                            max_steps,
                            max_dur,
                            retry_cnt,
                            max_ret,
                            meta_json,
                        ))
                    },
                )
                .optional()
                .map_err(|e| TaskError::Database(format!("Query task failed: {}", e)))?;

            let (
                id_str,
                name,
                description,
                original_command,
                state_str,
                priority_val,
                created_ms,
                updated_ms,
                started_ms,
                completed_ms,
                current_step,
                error_msg,
                result_sum,
                origin_req,
                trace_id,
                parent_id_str,
                max_steps,
                max_dur,
                retry_cnt,
                max_ret,
                meta_json,
            ) = match row_res {
                Some(data) => data,
                None => return Ok(None),
            };

            let parsed_id = Uuid::parse_str(&id_str)
                .map(TaskId)
                .map_err(|e| TaskError::Serialization(e.to_string()))?;

            let state = parse_task_state(&state_str);
            let priority = parse_task_priority(priority_val);
            let metadata: HashMap<String, String> =
                serde_json::from_str(&meta_json).unwrap_or_default();

            // Load associated steps
            let mut stmt = guard
                .prepare(
                    "SELECT step_index, description, tool_name, arguments_json, result_json,
                        state, started_at_ms, completed_at_ms, error
                 FROM task_steps WHERE task_id = ?1 ORDER BY step_index ASC",
                )
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let steps_iter = stmt
                .query_map(params![task_id_str], |row| {
                    let idx: i64 = row.get(0)?;
                    let desc: String = row.get(1)?;
                    let tool: Option<String> = row.get(2)?;
                    let args: Option<String> = row.get(3)?;
                    let res: Option<String> = row.get(4)?;
                    let state_s: String = row.get(5)?;
                    let start_m: Option<i64> = row.get(6)?;
                    let comp_m: Option<i64> = row.get(7)?;
                    let err: Option<String> = row.get(8)?;

                    Ok(TaskStep {
                        index: idx as u32,
                        description: desc,
                        tool_name: tool,
                        arguments_json: args,
                        result_json: res,
                        state: parse_step_state(&state_s),
                        started_at: start_m.and_then(|m| DateTime::from_timestamp_millis(m)),
                        completed_at: comp_m.and_then(|m| DateTime::from_timestamp_millis(m)),
                        error: err,
                    })
                })
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let mut steps = Vec::new();
            for step_res in steps_iter {
                if let Ok(step) = step_res {
                    steps.push(step);
                }
            }

            Ok(Some(Task {
                id: parsed_id,
                name,
                description,
                original_command,
                state,
                priority,
                created_at: DateTime::from_timestamp_millis(created_ms).unwrap_or_else(Utc::now),
                updated_at: DateTime::from_timestamp_millis(updated_ms).unwrap_or_else(Utc::now),
                started_at: started_ms.and_then(|m| DateTime::from_timestamp_millis(m)),
                completed_at: completed_ms.and_then(|m| DateTime::from_timestamp_millis(m)),
                steps,
                current_step_index: current_step as u32,
                error_message: error_msg,
                result_summary: result_sum,
                metadata,
                origin_request_id: origin_req,
                trace_id,
                parent_task_id: parent_id_str.and_then(|s| Uuid::parse_str(&s).ok().map(TaskId)),
                max_steps: max_steps as u32,
                max_duration_secs: max_dur as u64,
                retry_count: retry_cnt as u32,
                max_retries: max_ret as u32,
            }))
        })
        .await
        .map_err(|e| TaskError::Database(format!("Tokio join error: {}", e)))?
    }

    async fn update(&self, task: Task) -> Result<(), TaskError> {
        self.save(task).await
    }

    async fn delete(&self, id: &TaskId) -> Result<bool, TaskError> {
        let task_id_str = id.as_str();
        let conn_arc = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<bool, TaskError> {
            let guard = conn_arc
                .lock()
                .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let rows = guard
                .execute("DELETE FROM tasks WHERE id = ?1", params![task_id_str])
                .map_err(|e| TaskError::Database(e.to_string()))?;
            Ok(rows > 0)
        })
        .await
        .map_err(|e| TaskError::Database(e.to_string()))?
    }

    async fn list(&self) -> Result<Vec<Task>, TaskError> {
        let conn_arc = self.conn.clone();

        let task_ids = tokio::task::spawn_blocking(move || -> Result<Vec<TaskId>, TaskError> {
            let guard = conn_arc
                .lock()
                .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let mut stmt = guard
                .prepare("SELECT id FROM tasks ORDER BY created_at_ms DESC")
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let ids_iter = stmt
                .query_map([], |row| {
                    let id_str: String = row.get(0)?;
                    Ok(id_str)
                })
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let mut ids = Vec::new();
            for res in ids_iter {
                if let Ok(id_str) = res {
                    if let Ok(uuid) = Uuid::parse_str(&id_str) {
                        ids.push(TaskId(uuid));
                    }
                }
            }
            Ok(ids)
        })
        .await
        .map_err(|e| TaskError::Database(e.to_string()))??;

        let mut tasks = Vec::new();
        for id in task_ids {
            if let Some(task) = self.get(&id).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    async fn get_by_state(&self, state: TaskState) -> Result<Vec<Task>, TaskError> {
        let state_str = state.to_string();
        let conn_arc = self.conn.clone();

        let task_ids = tokio::task::spawn_blocking(move || -> Result<Vec<TaskId>, TaskError> {
            let guard = conn_arc
                .lock()
                .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let mut stmt = guard
                .prepare("SELECT id FROM tasks WHERE state = ?1 ORDER BY created_at_ms DESC")
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let ids_iter = stmt
                .query_map(params![state_str], |row| {
                    let id_str: String = row.get(0)?;
                    Ok(id_str)
                })
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let mut ids = Vec::new();
            for res in ids_iter {
                if let Ok(id_str) = res {
                    if let Ok(uuid) = Uuid::parse_str(&id_str) {
                        ids.push(TaskId(uuid));
                    }
                }
            }
            Ok(ids)
        })
        .await
        .map_err(|e| TaskError::Database(e.to_string()))??;

        let mut tasks = Vec::new();
        for id in task_ids {
            if let Some(task) = self.get(&id).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    async fn get_interrupted_tasks(&self) -> Result<Vec<Task>, TaskError> {
        let conn_arc = self.conn.clone();

        let task_ids = tokio::task::spawn_blocking(move || -> Result<Vec<TaskId>, TaskError> {
            let guard = conn_arc.lock().map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let mut stmt = guard
                .prepare(
                    "SELECT id FROM tasks
                     WHERE state IN ('RUNNING', 'Running', 'PAUSED', 'Paused', 'AWAITING_APPROVAL', 'AwaitingApproval', 'RECOVERING', 'Recovering')
                     ORDER BY created_at_ms ASC",
                )
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let ids_iter = stmt
                .query_map([], |row| {
                    let id_str: String = row.get(0)?;
                    Ok(id_str)
                })
                .map_err(|e| TaskError::Database(e.to_string()))?;

            let mut ids = Vec::new();
            for res in ids_iter {
                if let Ok(id_str) = res {
                    if let Ok(uuid) = Uuid::parse_str(&id_str) {
                        ids.push(TaskId(uuid));
                    }
                }
            }
            Ok(ids)
        })
        .await
        .map_err(|e| TaskError::Database(e.to_string()))??;

        let mut tasks = Vec::new();
        for id in task_ids {
            if let Some(task) = self.get(&id).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    /// Crash reconciliation: scans for any tasks left in `Running` or `Recovering` state,
    /// marks them as `Recovering` or `Failed` with diagnostic history, and returns them.
    #[instrument(skip(self))]
    async fn reconcile_crashed_tasks(&self) -> Result<Vec<Task>, TaskError> {
        let interrupted = self.get_interrupted_tasks().await?;
        let mut reconciled = Vec::new();

        for mut task in interrupted {
            if task.state == TaskState::Running {
                warn!(task_id = %task.id, "Reconciling task interrupted by process crash");
                task.state = TaskState::Recovering;
                task.updated_at = Utc::now();
                task.error_message = Some(
                    "Task interrupted by ungraceful daemon termination. Awaiting recovery policy."
                        .to_string(),
                );
                self.save(task.clone()).await?;
                reconciled.push(task);
            }
        }

        Ok(reconciled)
    }

    async fn count(&self) -> Result<usize, TaskError> {
        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<usize, TaskError> {
            let guard = conn_arc
                .lock()
                .map_err(|_| TaskError::Database("Mutex poisoned".to_string()))?;
            let count: i64 = guard
                .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
                .map_err(|e| TaskError::Database(e.to_string()))?;
            Ok(count as usize)
        })
        .await
        .map_err(|e| TaskError::Database(e.to_string()))?
    }
}

// ============================================================
// Helpers for SQLite string conversions
// ============================================================

fn parse_task_state(s: &str) -> TaskState {
    match s.to_uppercase().as_str() {
        "PENDING" => TaskState::Pending,
        "RUNNING" => TaskState::Running,
        "PAUSED" => TaskState::Paused,
        "AWAITING_APPROVAL" | "AWAITINGAPPROVAL" => TaskState::AwaitingApproval,
        "COMPLETED" => TaskState::Completed,
        "FAILED" => TaskState::Failed,
        "CANCELLED" => TaskState::Cancelled,
        "RECOVERING" => TaskState::Recovering,
        _ => TaskState::Pending,
    }
}

fn parse_task_priority(p: i32) -> TaskPriority {
    match p {
        0 => TaskPriority::Low,
        1 => TaskPriority::Normal,
        2 => TaskPriority::High,
        3 => TaskPriority::Critical,
        _ => TaskPriority::Normal,
    }
}

fn parse_step_state(s: &str) -> StepState {
    match s {
        "Running" | "RUNNING" => StepState::Running,
        "Completed" | "COMPLETED" => StepState::Completed,
        "Failed" | "FAILED" => StepState::Failed,
        "Skipped" | "SKIPPED" => StepState::Skipped,
        _ => StepState::Pending,
    }
}
