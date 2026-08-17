//! JARVIS Task Engine
//!
//! Defines the Task model, TaskState lifecycle state machine, and TaskRepository abstraction.
//!
//! # Architecture
//!
//! The Task is the fundamental unit of work in JARVIS. Every user request
//! that requires more than a trivial deterministic operation becomes a Task.
//!
//! ```text
//! User Request → PENDING → RUNNING → COMPLETED
//!                              ↓           ↑
//!                          PAUSED ────────┘
//!                              ↓
//!                     AWAITING_APPROVAL
//!                              ↓
//!                         CANCELLED
//!                              ↓
//!                           FAILED
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 3, Milestone M03.03 → M03.07

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::instrument;
use uuid::Uuid;

pub mod sqlite;
pub use sqlite::SqliteTaskRepository;

// ============================================================
// Task ID
// ============================================================

/// Globally unique identifier for a JARVIS task.
///
/// Tasks may persist across daemon restarts and device sessions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generate a new unique task ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

// ============================================================
// Task State
// ============================================================

/// Lifecycle states for a JARVIS task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    /// Task created but not yet started.
    Pending,
    /// Task is actively executing.
    Running,
    /// Task is paused, waiting for human input (credential, clarification, etc.).
    Paused,
    /// Task is waiting for the user to approve a potentially consequential action.
    AwaitingApproval,
    /// Task completed successfully.
    Completed,
    /// Task failed with an unrecoverable error.
    Failed,
    /// Task was cancelled by the user or system.
    Cancelled,
    /// Task is being recovered after a daemon restart.
    Recovering,
}

impl TaskState {
    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Returns true if the task is currently blocking on human input.
    pub fn is_waiting_for_human(&self) -> bool {
        matches!(
            self,
            TaskState::Paused | TaskState::AwaitingApproval
        )
    }

    /// Returns true if this transition is valid.
    pub fn can_transition_to(&self, next: &TaskState) -> bool {
        use TaskState::*;
        matches!(
            (self, next),
            (Pending, Running)
                | (Pending, Cancelled)
                | (Running, Paused)
                | (Running, AwaitingApproval)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Cancelled)
                | (Paused, Running)
                | (Paused, Cancelled)
                | (AwaitingApproval, Running)
                | (AwaitingApproval, Cancelled)
                // Recovery paths
                | (Failed, Recovering)
                | (Recovering, Running)
                | (Recovering, Failed)
                | (Recovering, Cancelled)
        )
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ============================================================
// Task Priority
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

// ============================================================
// Task Step
// ============================================================

/// A single step in a multi-step task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub index: u32,
    pub description: String,
    pub tool_name: Option<String>,
    pub arguments_json: Option<String>,
    pub result_json: Option<String>,
    pub state: StepState,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

// ============================================================
// Task
// ============================================================

/// The primary unit of work in JARVIS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    pub original_command: String,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<TaskStep>,
    pub current_step_index: u32,
    pub error_message: Option<String>,
    pub result_summary: Option<String>,
    pub metadata: HashMap<String, String>,
    pub origin_request_id: Option<String>,
    pub trace_id: Option<String>,
    pub parent_task_id: Option<TaskId>,
    pub max_steps: u32,
    pub max_duration_secs: u64,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl Task {
    pub fn new(command: impl Into<String>) -> Self {
        let now = Utc::now();
        let cmd = command.into();
        Self {
            id: TaskId::new(),
            name: cmd.chars().take(64).collect(),
            description: cmd.clone(),
            original_command: cmd,
            state: TaskState::Pending,
            priority: TaskPriority::Normal,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            steps: Vec::new(),
            current_step_index: 0,
            error_message: None,
            result_summary: None,
            metadata: HashMap::new(),
            origin_request_id: None,
            trace_id: None,
            parent_task_id: None,
            max_steps: 50,
            max_duration_secs: 300,
            retry_count: 0,
            max_retries: 3,
        }
    }

    #[instrument(skip(self), fields(task_id = %self.id, from = %self.state, to = %new_state))]
    pub fn transition(&mut self, new_state: TaskState) -> Result<(), TaskError> {
        if !self.state.can_transition_to(&new_state) {
            return Err(TaskError::InvalidTransition {
                from: self.state,
                to: new_state,
                task_id: self.id.clone(),
            });
        }

        let now = Utc::now();
        self.state = new_state;
        self.updated_at = now;

        match new_state {
            TaskState::Running if self.started_at.is_none() => {
                self.started_at = Some(now);
            }
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                self.completed_at = Some(now);
            }
            _ => {}
        }

        tracing::info!(
            task_id = %self.id,
            state = %new_state,
            "Task state transition"
        );

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), TaskError> {
        self.transition(TaskState::Running)
    }

    pub fn complete(&mut self, summary: Option<String>) -> Result<(), TaskError> {
        self.result_summary = summary;
        self.transition(TaskState::Completed)
    }

    pub fn fail(&mut self, error: impl Into<String>) -> Result<(), TaskError> {
        self.error_message = Some(error.into());
        self.transition(TaskState::Failed)
    }

    pub fn cancel(&mut self) -> Result<(), TaskError> {
        self.transition(TaskState::Cancelled)
    }

    pub fn pause(&mut self) -> Result<(), TaskError> {
        self.transition(TaskState::Paused)
    }

    pub fn request_approval(&mut self) -> Result<(), TaskError> {
        self.transition(TaskState::AwaitingApproval)
    }

    pub fn resume(&mut self) -> Result<(), TaskError> {
        self.transition(TaskState::Running)
    }

    pub fn exceeds_limits(&self) -> bool {
        if self.steps.len() as u32 >= self.max_steps {
            return true;
        }
        if let Some(started) = self.started_at {
            let elapsed = (Utc::now() - started).num_seconds() as u64;
            if elapsed >= self.max_duration_secs {
                return true;
            }
        }
        false
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// ============================================================
// Task Errors
// ============================================================

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Invalid task state transition from {from} to {to} for task {task_id}")]
    InvalidTransition {
        from: TaskState,
        to: TaskState,
        task_id: TaskId,
    },

    #[error("Task {task_id} not found")]
    NotFound { task_id: TaskId },

    #[error("Task {task_id} exceeded safety limits (max_steps or max_duration)")]
    LimitsExceeded { task_id: TaskId },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

// ============================================================
// Task Repository Trait
// ============================================================

/// Common abstract interface for task persistence.
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// Save or update a task.
    async fn save(&self, task: Task) -> Result<(), TaskError>;

    /// Retrieve a task by ID.
    async fn get(&self, id: &TaskId) -> Result<Option<Task>, TaskError>;

    /// Update an existing task.
    async fn update(&self, task: Task) -> Result<(), TaskError> {
        self.save(task).await
    }

    /// Delete a task by ID.
    async fn delete(&self, id: &TaskId) -> Result<bool, TaskError>;

    /// List all tasks in descending creation order.
    async fn list(&self) -> Result<Vec<Task>, TaskError>;

    /// Get all tasks in a specific lifecycle state.
    async fn get_by_state(&self, state: TaskState) -> Result<Vec<Task>, TaskError>;

    /// Get all tasks that were interrupted or active during an ungraceful shutdown.
    async fn get_interrupted_tasks(&self) -> Result<Vec<Task>, TaskError>;

    /// Crash recovery reconciliation: marks running tasks as Recovering and logs diagnostics.
    async fn reconcile_crashed_tasks(&self) -> Result<Vec<Task>, TaskError>;

    /// Total count of persisted tasks.
    async fn count(&self) -> Result<usize, TaskError>;
}

// ============================================================
// In-Memory Task Repository (Fast & Deterministic for Unit Tests)
// ============================================================

#[derive(Clone)]
pub struct InMemoryTaskRepository {
    tasks: Arc<RwLock<HashMap<TaskId, Task>>>,
}

impl InMemoryTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRepository for InMemoryTaskRepository {
    async fn save(&self, task: Task) -> Result<(), TaskError> {
        let mut guard = self.tasks.write().await;
        guard.insert(task.id.clone(), task);
        Ok(())
    }

    async fn get(&self, id: &TaskId) -> Result<Option<Task>, TaskError> {
        let guard = self.tasks.read().await;
        Ok(guard.get(id).cloned())
    }

    async fn delete(&self, id: &TaskId) -> Result<bool, TaskError> {
        let mut guard = self.tasks.write().await;
        Ok(guard.remove(id).is_some())
    }

    async fn list(&self) -> Result<Vec<Task>, TaskError> {
        let guard = self.tasks.read().await;
        let mut list: Vec<Task> = guard.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(list)
    }

    async fn get_by_state(&self, state: TaskState) -> Result<Vec<Task>, TaskError> {
        let guard = self.tasks.read().await;
        Ok(guard.values().filter(|t| t.state == state).cloned().collect())
    }

    async fn get_interrupted_tasks(&self) -> Result<Vec<Task>, TaskError> {
        let guard = self.tasks.read().await;
        Ok(guard
            .values()
            .filter(|t| {
                matches!(
                    t.state,
                    TaskState::Running | TaskState::Paused | TaskState::AwaitingApproval | TaskState::Recovering
                )
            })
            .cloned()
            .collect())
    }

    async fn reconcile_crashed_tasks(&self) -> Result<Vec<Task>, TaskError> {
        let mut guard = self.tasks.write().await;
        let mut reconciled = Vec::new();
        for task in guard.values_mut() {
            if task.state == TaskState::Running {
                task.state = TaskState::Recovering;
                task.updated_at = Utc::now();
                task.error_message = Some("Task interrupted by ungraceful daemon termination.".to_string());
                reconciled.push(task.clone());
            }
        }
        Ok(reconciled)
    }

    async fn count(&self) -> Result<usize, TaskError> {
        let guard = self.tasks.read().await;
        Ok(guard.len())
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_repository_crud() {
        let repo = InMemoryTaskRepository::new();
        let task = Task::new("open chrome");
        let task_id = task.id.clone();

        repo.save(task).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let retrieved = repo.get(&task_id).await.unwrap().unwrap();
        assert_eq!(retrieved.original_command, "open chrome");
        assert_eq!(retrieved.state, TaskState::Pending);

        repo.delete(&task_id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_in_memory_crud() {
        let repo = SqliteTaskRepository::open_in_memory().unwrap();
        let mut task = Task::new("test sqlite task").with_metadata("device", "desktop");
        let task_id = task.id.clone();

        repo.save(task.clone()).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let retrieved = repo.get(&task_id).await.unwrap().unwrap();
        assert_eq!(retrieved.original_command, "test sqlite task");
        assert_eq!(retrieved.state, TaskState::Pending);

        task.start().unwrap();
        repo.save(task.clone()).await.unwrap();

        let updated = repo.get(&task_id).await.unwrap().unwrap();
        assert_eq!(updated.state, TaskState::Running);
        assert!(updated.started_at.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_crash_durability_across_reopen() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("jarvis_test.db");

        let task_id = {
            let repo = SqliteTaskRepository::open(&db_path).unwrap();
            let mut task = Task::new("durable task");
            task.steps.push(TaskStep {
                index: 0,
                description: "Launch process".to_string(),
                tool_name: Some("open_application".to_string()),
                arguments_json: Some(r#"{"app":"chrome"}"#.to_string()),
                result_json: None,
                state: StepState::Running,
                started_at: Some(Utc::now()),
                completed_at: None,
                error: None,
            });
            task.start().unwrap();
            repo.save(task.clone()).await.unwrap();
            task.id
        }; // repo is dropped here, simulating daemon termination

        // Reopen database from disk
        let repo_reopened = SqliteTaskRepository::open(&db_path).unwrap();
        let loaded = repo_reopened.get(&task_id).await.unwrap().unwrap();

        assert_eq!(loaded.state, TaskState::Running);
        assert_eq!(loaded.steps.len(), 1);
        assert_eq!(loaded.steps[0].tool_name, Some("open_application".to_string()));

        // Perform crash reconciliation
        let reconciled = repo_reopened.reconcile_crashed_tasks().await.unwrap();
        assert_eq!(reconciled.len(), 1);
        assert_eq!(reconciled[0].state, TaskState::Recovering);

        let reloaded = repo_reopened.get(&task_id).await.unwrap().unwrap();
        assert_eq!(reloaded.state, TaskState::Recovering);
    }

    #[test]
    fn test_valid_transitions() {
        let mut task = Task::new("test command");
        assert_eq!(task.state, TaskState::Pending);

        task.start().unwrap();
        assert_eq!(task.state, TaskState::Running);
        assert!(task.started_at.is_some());

        task.complete(Some("Finished".to_string())).unwrap();
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.completed_at.is_some());
        assert_eq!(task.result_summary, Some("Finished".to_string()));
    }

    #[test]
    fn test_invalid_transition() {
        let mut task = Task::new("test command");
        task.complete(None).unwrap_err(); // Cannot jump Pending -> Completed directly
    }

    #[test]
    fn test_pause_and_resume() {
        let mut task = Task::new("test command");
        task.start().unwrap();
        task.pause().unwrap();
        assert_eq!(task.state, TaskState::Paused);
        assert!(task.state.is_waiting_for_human());

        task.resume().unwrap();
        assert_eq!(task.state, TaskState::Running);
    }

    #[test]
    fn test_approval_flow() {
        let mut task = Task::new("sensitive command");
        task.start().unwrap();
        task.request_approval().unwrap();
        assert_eq!(task.state, TaskState::AwaitingApproval);
        assert!(task.state.is_waiting_for_human());

        task.resume().unwrap();
        assert_eq!(task.state, TaskState::Running);
    }

    #[test]
    fn test_failure_with_error() {
        let mut task = Task::new("failing command");
        task.start().unwrap();
        task.fail("Network timed out").unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error_message, Some("Network timed out".to_string()));
        assert!(task.state.is_terminal());
    }

    #[test]
    fn test_safety_limits() {
        let mut task = Task::new("long command");
        task.max_steps = 2;
        assert!(!task.exceeds_limits());

        task.steps.push(TaskStep {
            index: 0,
            description: "Step 1".to_string(),
            tool_name: None,
            arguments_json: None,
            result_json: None,
            state: StepState::Completed,
            started_at: None,
            completed_at: None,
            error: None,
        });
        task.steps.push(TaskStep {
            index: 1,
            description: "Step 2".to_string(),
            tool_name: None,
            arguments_json: None,
            result_json: None,
            state: StepState::Completed,
            started_at: None,
            completed_at: None,
            error: None,
        });

        assert!(task.exceeds_limits());
    }
}
