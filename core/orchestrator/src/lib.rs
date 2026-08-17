//! JARVIS Core Orchestrator
//!
//! Orchestrates the full lifecycle of an action from user input to OS execution:
//! Intent → Task Creation → Policy Check → Tool Execution → Verification → State Update → Response.
//!
//! # Architecture Flow
//!
//! ```text
//! User / CLI / UI
//!     ↓
//! Orchestrator::execute_command("open chrome")
//!     ↓
//! 1. Intent Parsing: "open_application" + {"application": "chrome"}
//! 2. TaskEngine: Create Task, state = PENDING
//! 3. EventBus: Emit TaskCreated
//! 4. TaskEngine: Transition state = RUNNING
//! 5. EventBus: Emit TaskStarted
//! 6. PolicyEngine: Evaluate tool safety against AutonomyLevel
//! 7. ToolRuntime: Execute open_application tool via WindowsPlatformAdapter
//! 8. Verification: Check process launch and PID
//! 9. TaskEngine: Transition state = COMPLETED (or FAILED)
//! 10. EventBus: Emit ToolCompleted & TaskCompleted
//! 11. Response: "Chrome is open, sir."
//! ```
//!
//! IMPLEMENTATION STATUS: Vertical Slice 1

use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, instrument, warn};

use jarvis_event_bus::{EventBus, JarvisEvent, TaskEvent, ToolEvent};
use jarvis_platform::PlatformAdapter;
use jarvis_policy::{AutonomyLevel, PolicyDecision, PolicyEngine};
use jarvis_task_engine::{InMemoryTaskRepository, Task, TaskRepository};
use jarvis_tools::{ToolExecutionContext, ToolRegistry, ToolRequest, ToolResult};

/// Structured intent extracted from user command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedIntent {
    pub tool_name: String,
    pub arguments: Value,
    pub raw_command: String,
}

/// The final outcome of a command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Success {
        task_id: String,
        spoken_response: String,
        tool_name: String,
        tool_data: Value,
        duration_ms: u64,
    },
    ApprovalRequired {
        task_id: String,
        reason: String,
        tool_name: String,
    },
    Denied {
        task_id: String,
        reason: String,
    },
    Failed {
        task_id: String,
        error: String,
    },
}

/// The core orchestrator managing the pipeline.
pub struct Orchestrator {
    event_bus: EventBus,
    policy_engine: PolicyEngine,
    tool_registry: ToolRegistry,
    platform_adapter: Arc<dyn PlatformAdapter>,
    task_repository: Arc<dyn TaskRepository>,
    autonomy_level: AutonomyLevel,
}

impl Orchestrator {
    /// Create a new orchestrator with the specified platform adapter and default in-memory task repository.
    pub fn new(platform_adapter: Arc<dyn PlatformAdapter>) -> Self {
        Self {
            event_bus: EventBus::new(256),
            policy_engine: PolicyEngine::new(),
            tool_registry: ToolRegistry::with_builtins(),
            platform_adapter,
            task_repository: Arc::new(InMemoryTaskRepository::new()),
            autonomy_level: AutonomyLevel::Level3Conservative,
        }
    }

    /// Create an orchestrator with a specific task repository (e.g. SqliteTaskRepository).
    pub fn with_repository(mut self, repo: Arc<dyn TaskRepository>) -> Self {
        self.task_repository = repo;
        self
    }

    /// Set the autonomy level.
    pub fn set_autonomy_level(&mut self, level: AutonomyLevel) {
        self.autonomy_level = level;
    }

    /// Access the event bus to subscribe to execution events.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Access the task repository.
    pub fn task_repository(&self) -> &Arc<dyn TaskRepository> {
        &self.task_repository
    }

    /// Parse a natural language command into a structured tool intent.
    ///
    /// (Deterministic intent parser for early vertical slice; will connect to LLM in Phase 4/10).
    pub fn parse_intent(&self, command: &str) -> Result<ParsedIntent> {
        let text = command.trim().to_lowercase();

        // 1. "open <app>" or "launch <app>" or "start <app>"
        if text.starts_with("open ") || text.starts_with("launch ") || text.starts_with("start ") {
            let app = text
                .trim_start_matches("open ")
                .trim_start_matches("launch ")
                .trim_start_matches("start ")
                .trim();

            return Ok(ParsedIntent {
                tool_name: "open_application".to_string(),
                arguments: json!({ "application": app }),
                raw_command: command.to_string(),
            });
        }

        // 2. Time query
        if text.contains("time") || text.contains("what time") || text.contains("clock") {
            return Ok(ParsedIntent {
                tool_name: "get_time".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 3. Fallback: treat as open application if it's a single word
        if !text.contains(' ') && !text.is_empty() {
            return Ok(ParsedIntent {
                tool_name: "open_application".to_string(),
                arguments: json!({ "application": text }),
                raw_command: command.to_string(),
            });
        }

        Err(anyhow!(
            "Could not determine intent for command: '{}'",
            command
        ))
    }

    /// Execute a command through the complete architectural pipeline.
    #[instrument(skip(self), fields(command = %raw_command))]
    pub async fn execute_command(&self, raw_command: &str) -> ExecutionOutcome {
        let start_time = Instant::now();
        info!(command = %raw_command, "Orchestrator received command");

        // Step 1: Parse intent
        let intent = match self.parse_intent(raw_command) {
            Ok(i) => i,
            Err(e) => {
                error!(error = %e, "Intent parsing failed");
                return ExecutionOutcome::Failed {
                    task_id: "none".to_string(),
                    error: e.to_string(),
                };
            }
        };

        // Step 2: Create Task in TaskEngine & persist
        let mut task = Task::new(raw_command);
        let task_id_str = task.id.as_str();

        if let Err(e) = self.task_repository.save(task.clone()).await {
            error!(error = %e, "Failed to save task to repository");
        }

        // Step 3: Emit TaskCreated event
        self.event_bus
            .publish(JarvisEvent::Task(TaskEvent::Created {
                task_id: task_id_str.clone(),
                name: task.name.clone(),
            }))
            .await;

        // Step 4: Transition Task to Running & persist
        if let Err(e) = task.start() {
            error!(error = %e, "Failed to start task");
        }
        self.task_repository.save(task.clone()).await.ok();
        self.event_bus
            .publish(JarvisEvent::Task(TaskEvent::Started {
                task_id: task_id_str.clone(),
            }))
            .await;

        // Step 5: Policy Evaluation
        let tool_def = match self.tool_registry.get(&intent.tool_name) {
            Some(t) => t.definition().clone(),
            None => {
                let err_msg = format!("Tool '{}' not found", intent.tool_name);
                task.fail(&err_msg).ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Task(TaskEvent::Failed {
                        task_id: task_id_str.clone(),
                        error: err_msg.clone(),
                    }))
                    .await;
                return ExecutionOutcome::Failed {
                    task_id: task_id_str,
                    error: err_msg,
                };
            }
        };

        let decision = self.policy_engine.evaluate(
            &intent.tool_name,
            tool_def.risk_level,
            self.autonomy_level,
        );

        match decision {
            PolicyDecision::Denied { reason } => {
                warn!(reason = %reason, "Policy denied tool execution");
                task.fail(&reason).ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Task(TaskEvent::Failed {
                        task_id: task_id_str.clone(),
                        error: reason.clone(),
                    }))
                    .await;
                return ExecutionOutcome::Denied {
                    task_id: task_id_str,
                    reason,
                };
            }
            PolicyDecision::ApprovalRequired { reason, .. } => {
                info!(reason = %reason, "Policy requires user approval");
                task.request_approval().ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Tool(ToolEvent::ApprovalRequested {
                        request_id: task_id_str.clone(),
                        tool_name: intent.tool_name.clone(),
                        action_description: reason.clone(),
                    }))
                    .await;
                return ExecutionOutcome::ApprovalRequired {
                    task_id: task_id_str,
                    reason,
                    tool_name: intent.tool_name,
                };
            }
            PolicyDecision::Allowed => {
                info!(tool = %intent.tool_name, "Policy allowed execution");
            }
        }

        // Step 6: Execute Tool via ToolRuntime
        let tool_req = ToolRequest::new(&intent.tool_name, intent.arguments.clone())
            .with_task_id(&task_id_str);

        self.event_bus
            .publish(JarvisEvent::Tool(ToolEvent::Started {
                request_id: tool_req.request_id.clone(),
                task_id: Some(task_id_str.clone()),
                tool_name: intent.tool_name.clone(),
            }))
            .await;

        let ctx = ToolExecutionContext::new(self.platform_adapter.clone());
        let tool_result = self.tool_registry.execute(tool_req.clone(), &ctx).await;

        // Step 7: Process Tool Result & Complete Task
        let elapsed_total = start_time.elapsed().as_millis() as u64;

        match tool_result {
            Ok(result) if result.success => {
                let spoken_response = self.generate_spoken_response(&intent.tool_name, &result);

                self.event_bus
                    .publish(JarvisEvent::Tool(ToolEvent::Completed {
                        request_id: tool_req.request_id,
                        tool_name: intent.tool_name.clone(),
                        duration_ms: result.execution_time_ms,
                    }))
                    .await;

                task.complete(Some(spoken_response.clone())).ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Task(TaskEvent::Completed {
                        task_id: task_id_str.clone(),
                        summary: Some(spoken_response.clone()),
                    }))
                    .await;

                info!(
                    task_id = %task_id_str,
                    spoken = %spoken_response,
                    duration_ms = elapsed_total,
                    "Command executed successfully"
                );

                ExecutionOutcome::Success {
                    task_id: task_id_str,
                    spoken_response,
                    tool_name: intent.tool_name,
                    tool_data: result.data,
                    duration_ms: elapsed_total,
                }
            }
            Ok(result) => {
                let error_msg = result
                    .error
                    .unwrap_or_else(|| "Tool execution failed".to_string());
                self.event_bus
                    .publish(JarvisEvent::Tool(ToolEvent::Failed {
                        request_id: tool_req.request_id,
                        tool_name: intent.tool_name.clone(),
                        error: error_msg.clone(),
                    }))
                    .await;

                task.fail(&error_msg).ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Task(TaskEvent::Failed {
                        task_id: task_id_str.clone(),
                        error: error_msg.clone(),
                    }))
                    .await;

                ExecutionOutcome::Failed {
                    task_id: task_id_str,
                    error: error_msg,
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                task.fail(&error_msg).ok();
                self.task_repository.save(task.clone()).await.ok();
                self.event_bus
                    .publish(JarvisEvent::Task(TaskEvent::Failed {
                        task_id: task_id_str.clone(),
                        error: error_msg.clone(),
                    }))
                    .await;

                ExecutionOutcome::Failed {
                    task_id: task_id_str,
                    error: error_msg,
                }
            }
        }
    }

    /// Generate natural companion response for a successful action.
    fn generate_spoken_response(&self, tool_name: &str, result: &ToolResult) -> String {
        match tool_name {
            "open_application" => {
                let app = result
                    .data
                    .get("application")
                    .and_then(|v| v.as_str())
                    .unwrap_or("the application");
                let capitalized = {
                    let mut c = app.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                };
                format!("{} is open, sir.", capitalized)
            }
            "get_time" => {
                let time = result
                    .data
                    .get("time")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let date = result
                    .data
                    .get("date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("It is currently {}, {}.", time, date)
            }
            _ => "Action completed, sir.".to_string(),
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use jarvis_platform::*;

    struct MockAdapter {
        pub fail: bool,
    }

    #[async_trait::async_trait]
    impl PlatformAdapter for MockAdapter {
        async fn get_platform_info(&self) -> Result<PlatformInfo> {
            Ok(PlatformInfo {
                os: OperatingSystem::Windows,
                os_version: "11".to_string(),
                arch: Architecture::X86_64,
                hostname: "test".to_string(),
                username: "admin".to_string(),
                home_dir: std::path::PathBuf::from("C:\\Users\\admin"),
                temp_dir: std::path::PathBuf::from("C:\\Temp"),
            })
        }
        async fn open_application(
            &self,
            app: &str,
            _opts: Option<LaunchOptions>,
        ) -> Result<ProcessInfo> {
            if self.fail {
                anyhow::bail!("Process creation failed");
            }
            Ok(ProcessInfo {
                pid: 1234,
                name: app.to_string(),
                executable_path: None,
                command_line: Some(app.to_string()),
                running: true,
            })
        }
        async fn close_application(&self, _app: &str) -> Result<()> {
            Ok(())
        }
        async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
            Ok(vec![])
        }
        async fn is_application_running(&self, _app: &str) -> Result<bool> {
            Ok(true)
        }
        async fn list_windows(&self) -> Result<Vec<WindowInfo>> {
            Ok(vec![])
        }
        async fn focus_window(&self, _h: &str) -> Result<()> {
            Ok(())
        }
        async fn minimize_window(&self, _h: &str) -> Result<()> {
            Ok(())
        }
        async fn maximize_window(&self, _h: &str) -> Result<()> {
            Ok(())
        }
        async fn set_window_bounds(&self, _h: &str, _b: Rect) -> Result<()> {
            Ok(())
        }
        async fn take_screenshot(&self) -> Result<Screenshot> {
            Ok(Screenshot {
                data: vec![],
                format: ImageFormat::Png,
                width: 0,
                height: 0,
                display_index: 0,
            })
        }
        async fn take_screenshot_display(&self, _i: u32) -> Result<Screenshot> {
            self.take_screenshot().await
        }
        async fn take_screenshot_region(&self, _r: Rect) -> Result<Screenshot> {
            self.take_screenshot().await
        }
        async fn get_clipboard(&self) -> Result<ClipboardContent> {
            Ok(ClipboardContent::Empty)
        }
        async fn set_clipboard(&self, _c: ClipboardContent) -> Result<()> {
            Ok(())
        }
        async fn show_notification(&self, _n: NotificationRequest) -> Result<()> {
            Ok(())
        }
        async fn get_disk_space(&self) -> Result<DiskInfo> {
            Ok(DiskInfo {
                total_bytes: 1,
                available_bytes: 1,
                used_bytes: 0,
            })
        }
        async fn get_memory_info(&self) -> Result<MemoryInfo> {
            Ok(MemoryInfo {
                total_bytes: 1,
                available_bytes: 1,
                used_bytes: 0,
            })
        }
    }

    #[tokio::test]
    async fn test_orchestrator_vertical_slice_open_chrome() {
        let adapter = Arc::new(MockAdapter { fail: false });
        let orchestrator = Orchestrator::new(adapter);

        let mut sub = orchestrator.event_bus().subscribe();

        let outcome = orchestrator.execute_command("open chrome").await;

        match outcome {
            ExecutionOutcome::Success {
                spoken_response,
                tool_name,
                tool_data,
                ..
            } => {
                assert_eq!(tool_name, "open_application");
                assert_eq!(spoken_response, "Chrome is open, sir.");
                assert_eq!(tool_data["pid"], 1234);
            }
            _ => panic!("Expected Success outcome"),
        }

        // Verify event was emitted
        assert!(sub.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_policy_denied() {
        let adapter = Arc::new(MockAdapter { fail: false });
        let mut orchestrator = Orchestrator::new(adapter);
        orchestrator.set_autonomy_level(AutonomyLevel::Level0ChatOnly);

        let outcome = orchestrator.execute_command("open chrome").await;
        assert!(matches!(outcome, ExecutionOutcome::Denied { .. }));
    }

    #[tokio::test]
    async fn test_orchestrator_with_sqlite_persistence() {
        use jarvis_task_engine::SqliteTaskRepository;

        let adapter = Arc::new(MockAdapter { fail: false });
        let sqlite_repo = Arc::new(SqliteTaskRepository::open_in_memory().unwrap());
        let orchestrator = Orchestrator::new(adapter).with_repository(sqlite_repo.clone());

        let outcome = orchestrator.execute_command("what time is it").await;
        assert!(matches!(outcome, ExecutionOutcome::Success { .. }));

        // Verify task was durably saved to SQLite repository
        assert_eq!(sqlite_repo.count().await.unwrap(), 1);
        let tasks = sqlite_repo.list().await.unwrap();
        assert_eq!(tasks[0].state, jarvis_task_engine::TaskState::Completed);
    }
}
