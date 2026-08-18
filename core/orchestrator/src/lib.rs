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

    /// Parse a natural language or structured command into a tool intent.
    pub fn parse_intent(&self, command: &str) -> Result<ParsedIntent> {
        let text = command.trim().to_lowercase();
        if text.is_empty() {
            return Err(anyhow!("Empty command"));
        }
        let clean_text = text.trim_end_matches(['?', '.', '!', ';']).trim();

        // 1. Explicit tool names & window list commands
        if text == "list_windows"
            || text == "list windows"
            || text == "show open windows"
            || text.contains("list windows")
            || text.contains("show windows")
            || text.contains("open windows")
            || text.contains("what windows")
        {
            return Ok(ParsedIntent {
                tool_name: "list_windows".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 2. Active window detection commands
        if text == "get_active_window"
            || text == "what window is active?"
            || text == "what window is active"
            || text == "which window is currently active?"
            || text == "which window is currently active"
            || text.contains("active window")
            || text.contains("current window")
            || text.contains("what window am i using")
            || text.contains("what window am i currently using")
        {
            return Ok(ParsedIntent {
                tool_name: "get_active_window".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 3. Focus window commands
        if text.starts_with("focus_window ")
            || text.starts_with("focus ")
            || text.starts_with("switch to ")
            || text.starts_with("bring ")
        {
            let target = text
                .trim_start_matches("focus_window ")
                .trim_start_matches("focus ")
                .trim_start_matches("switch to ")
                .trim_start_matches("bring ")
                .trim_end_matches(" to the front")
                .trim_end_matches(" to front")
                .trim();

            if !target.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "focus_window".to_string(),
                    arguments: json!({ "target": target, "application": target, "window_handle": target }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 4. Minimize window commands
        if text.starts_with("minimize_window ") || text.starts_with("minimize ") {
            let target = text
                .trim_start_matches("minimize_window ")
                .trim_start_matches("minimize ")
                .trim();

            if !target.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "minimize_window".to_string(),
                    arguments: json!({ "target": target, "application": target, "window_handle": target }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 5. Maximize window commands
        if text.starts_with("maximize_window ") || text.starts_with("maximize ") {
            let target = text
                .trim_start_matches("maximize_window ")
                .trim_start_matches("maximize ")
                .trim();

            if !target.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "maximize_window".to_string(),
                    arguments: json!({ "target": target, "application": target, "window_handle": target }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 6. Restore window commands
        if text.starts_with("restore_window ") || text.starts_with("restore ") {
            let target = text
                .trim_start_matches("restore_window ")
                .trim_start_matches("restore ")
                .trim();

            if !target.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "restore_window".to_string(),
                    arguments: json!({ "target": target, "application": target, "window_handle": target }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 7. Move window commands (e.g. "move_window chrome 100 100" or "move chrome to 100 100")
        if text.starts_with("move_window ") || text.starts_with("move ") {
            let body = text
                .trim_start_matches("move_window ")
                .trim_start_matches("move ")
                .trim();

            // Extract target, x, y
            let parts: Vec<&str> = body.split_whitespace().collect();
            if parts.len() >= 3 {
                let target = parts[0];
                let x_idx = if parts[1] == "to" { 2 } else { 1 };
                let y_idx = x_idx + 1;
                if y_idx < parts.len() {
                    let x = parts[x_idx].trim_matches(',').parse::<i32>().unwrap_or(100);
                    let y = parts[y_idx].trim_matches(',').parse::<i32>().unwrap_or(100);
                    return Ok(ParsedIntent {
                        tool_name: "move_window".to_string(),
                        arguments: json!({ "target": target, "application": target, "x": x, "y": y }),
                        raw_command: command.to_string(),
                    });
                }
            }
        }

        // 8. Resize window commands (e.g. "resize_window chrome 1280 720" or "resize chrome to 1280 by 720")
        if text.starts_with("resize_window ") || text.starts_with("resize ") {
            let body = text
                .trim_start_matches("resize_window ")
                .trim_start_matches("resize ")
                .trim();

            let tokens: Vec<&str> = body.split_whitespace().collect();
            if tokens.len() >= 3 {
                let target = tokens[0];
                let mut nums = Vec::new();
                for t in &tokens[1..] {
                    if let Ok(val) = t.trim_matches(',').parse::<u32>() {
                        nums.push(val);
                    }
                }
                if nums.len() >= 2 {
                    return Ok(ParsedIntent {
                        tool_name: "resize_window".to_string(),
                        arguments: json!({ "target": target, "application": target, "width": nums[0], "height": nums[1] }),
                        raw_command: command.to_string(),
                    });
                }
            }
        }

        // 9. Volume & Mute control
        if text.starts_with("set volume ")
            || text.starts_with("set system volume ")
            || text.starts_with("volume ")
            || text.starts_with("set_system_volume ")
        {
            let num_str = text
                .trim_start_matches("set_system_volume ")
                .trim_start_matches("set system volume ")
                .trim_start_matches("set volume ")
                .trim_start_matches("volume ")
                .trim_end_matches('%')
                .trim();

            if let Ok(level) = num_str.parse::<u32>() {
                return Ok(ParsedIntent {
                    tool_name: "set_system_volume".to_string(),
                    arguments: json!({ "level": level }),
                    raw_command: command.to_string(),
                });
            }
        }

        if text == "mute"
            || text == "unmute"
            || text == "mute volume"
            || text == "mute system"
            || text == "mute_system_volume"
            || text.starts_with("mute_system_volume ")
        {
            let is_unmute = text.contains("unmute");
            return Ok(ParsedIntent {
                tool_name: "mute_system_volume".to_string(),
                arguments: json!({ "mute": !is_unmute }),
                raw_command: command.to_string(),
            });
        }

        // 10. Lock workstation
        if text == "lock"
            || text == "lock computer"
            || text == "lock workstation"
            || text == "lock screen"
            || text == "lock_workstation"
        {
            return Ok(ParsedIntent {
                tool_name: "lock_workstation".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 11. Power management (Shutdown / Restart / Sleep)
        if text == "shutdown"
            || text == "shutdown computer"
            || text == "shutdown system"
            || text == "shutdown_system"
            || text.starts_with("shutdown_system ")
        {
            let force = text.contains("force");
            return Ok(ParsedIntent {
                tool_name: "shutdown_system".to_string(),
                arguments: json!({ "force": force }),
                raw_command: command.to_string(),
            });
        }

        if text == "restart"
            || text == "reboot"
            || text == "restart computer"
            || text == "restart system"
            || text == "restart_system"
            || text.starts_with("restart_system ")
        {
            let force = text.contains("force");
            return Ok(ParsedIntent {
                tool_name: "restart_system".to_string(),
                arguments: json!({ "force": force }),
                raw_command: command.to_string(),
            });
        }

        if text == "sleep"
            || text == "sleep computer"
            || text == "sleep system"
            || text == "sleep_system"
        {
            return Ok(ParsedIntent {
                tool_name: "sleep_system".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 12. System info query
        if text == "system info"
            || text == "system status"
            || text == "get system info"
            || text == "get_system_info"
            || text == "device info"
        {
            return Ok(ParsedIntent {
                tool_name: "get_system_info".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 13. Process Management: Close / Kill Application
        if clean_text.starts_with("close_application ")
            || clean_text.starts_with("close ")
            || clean_text.starts_with("quit ")
            || clean_text.starts_with("exit ")
        {
            let app = clean_text
                .trim_start_matches("close_application ")
                .trim_start_matches("close ")
                .trim_start_matches("quit ")
                .trim_start_matches("exit ")
                .trim_end_matches(['?', '.', '!', ';'])
                .trim();

            if !app.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "close_application".to_string(),
                    arguments: json!({ "target": app, "application": app }),
                    raw_command: command.to_string(),
                });
            }
        }

        if clean_text.starts_with("kill_process ") || clean_text.starts_with("kill ") {
            let app = clean_text
                .trim_start_matches("kill_process ")
                .trim_start_matches("kill ")
                .trim_end_matches(['?', '.', '!', ';'])
                .trim();

            if !app.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "kill_process".to_string(),
                    arguments: json!({ "target": app, "process": app }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 14. Process Management: List Processes
        if clean_text == "list_processes"
            || clean_text == "list processes"
            || clean_text == "show running processes"
            || clean_text == "show processes"
            || clean_text == "what processes are running"
            || clean_text == "running processes"
            || clean_text.contains("list processes")
            || clean_text.contains("running processes")
        {
            return Ok(ParsedIntent {
                tool_name: "list_processes".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 15. Process Management: Is Application Running
        if clean_text.starts_with("is_application_running ")
            || (clean_text.starts_with("is ")
                && (clean_text.contains(" running")
                    || clean_text.contains(" active")
                    || clean_text.contains(" open")))
        {
            let app = if clean_text.starts_with("is_application_running ") {
                clean_text.trim_start_matches("is_application_running ").trim()
            } else if clean_text.starts_with("is ") && clean_text.contains(" running") {
                let after_is = clean_text.trim_start_matches("is ").trim();
                after_is.split(" running").next().unwrap_or(after_is).trim()
            } else if clean_text.starts_with("is ") && clean_text.contains(" active") {
                let after_is = clean_text.trim_start_matches("is ").trim();
                after_is.split(" active").next().unwrap_or(after_is).trim()
            } else if clean_text.starts_with("is ") && clean_text.contains(" open") {
                let after_is = clean_text.trim_start_matches("is ").trim();
                after_is.split(" open").next().unwrap_or(after_is).trim()
            } else {
                ""
            };

            let app_clean = app.trim_end_matches(['?', '.', '!', ';']).trim();

            if !app_clean.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "is_application_running".to_string(),
                    arguments: json!({ "target": app_clean, "application": app_clean }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 16. Screenshot requests
        if clean_text == "take_screenshot"
            || clean_text == "take a screenshot"
            || clean_text == "take screenshot"
            || clean_text == "screenshot"
            || clean_text == "screenshot my screen"
            || clean_text == "capture my screen"
            || clean_text == "capture screen"
            || clean_text == "take a picture of my screen"
            || clean_text.starts_with("take a screenshot")
            || clean_text.starts_with("take screenshot")
            || clean_text.starts_with("screenshot ")
            || clean_text.starts_with("capture screen")
        {
            // Check for display variant e.g. "take a screenshot of display 2"
            if clean_text.contains("display ") {
                let parts: Vec<&str> = clean_text.split_whitespace().collect();
                for (i, word) in parts.iter().enumerate() {
                    if *word == "display" && i + 1 < parts.len() {
                        if let Ok(idx) = parts[i + 1].parse::<u32>() {
                            return Ok(ParsedIntent {
                                tool_name: "take_screenshot_display".to_string(),
                                arguments: json!({ "display_index": idx, "display": idx }),
                                raw_command: command.to_string(),
                            });
                        }
                    }
                }
            }

            // Check for region variant e.g. "take a screenshot of region 0 0 800 600"
            if clean_text.contains("region") {
                let numbers: Vec<i64> = clean_text
                    .split_whitespace()
                    .filter_map(|w| w.parse::<i64>().ok())
                    .collect();
                if numbers.len() >= 4 {
                    return Ok(ParsedIntent {
                        tool_name: "take_screenshot_region".to_string(),
                        arguments: json!({
                            "x": numbers[0],
                            "y": numbers[1],
                            "width": numbers[2],
                            "height": numbers[3]
                        }),
                        raw_command: command.to_string(),
                    });
                }
            }

            return Ok(ParsedIntent {
                tool_name: "take_screenshot".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 17. Clipboard Get / Read requests
        if clean_text == "get_clipboard"
            || clean_text == "get clipboard"
            || clean_text == "read clipboard"
            || clean_text == "read my clipboard"
            || clean_text == "show clipboard"
            || clean_text == "show my clipboard"
            || clean_text == "what is in my clipboard"
            || clean_text == "what's in my clipboard"
            || clean_text == "what is on my clipboard"
            || clean_text == "what's on my clipboard"
            || clean_text == "check clipboard"
            || clean_text == "check my clipboard"
        {
            return Ok(ParsedIntent {
                tool_name: "get_clipboard".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 18. Clipboard Set / Copy requests
        if clean_text.starts_with("copy ")
            || clean_text.starts_with("set clipboard ")
            || clean_text.starts_with("put ")
            || clean_text.starts_with("copy_to_clipboard ")
            || clean_text.starts_with("set_clipboard ")
        {
            let mut clip_text = "";
            let lower = clean_text;

            if lower.starts_with("copy this to my clipboard:") {
                clip_text = command.trim_start_matches(|c: char| c != ':').trim_start_matches(':').trim();
            } else if lower.starts_with("copy this to my clipboard") {
                clip_text = command.trim_start_matches("copy this to my clipboard").trim();
            } else if lower.starts_with("set clipboard to ") {
                clip_text = command.trim_start_matches("set clipboard to ").trim();
            } else if lower.starts_with("set clipboard ") {
                clip_text = command.trim_start_matches("set clipboard ").trim();
            } else if lower.starts_with("set_clipboard ") {
                clip_text = command.trim_start_matches("set_clipboard ").trim();
            } else if lower.starts_with("copy ") && (lower.ends_with(" to my clipboard") || lower.ends_with(" to clipboard")) {
                let start_idx = command.to_lowercase().find("copy ").map(|i| i + 5).unwrap_or(0);
                let end_idx = if command.to_lowercase().ends_with(" to my clipboard") {
                    command.len() - " to my clipboard".len()
                } else if command.to_lowercase().ends_with(" to clipboard") {
                    command.len() - " to clipboard".len()
                } else {
                    command.len()
                };
                clip_text = command[start_idx..end_idx].trim();
            } else if lower.starts_with("put ") && (lower.ends_with(" in my clipboard") || lower.ends_with(" in clipboard")) {
                let start_idx = command.to_lowercase().find("put ").map(|i| i + 4).unwrap_or(0);
                let end_idx = if command.to_lowercase().ends_with(" in my clipboard") {
                    command.len() - " in my clipboard".len()
                } else if command.to_lowercase().ends_with(" in clipboard") {
                    command.len() - " in clipboard".len()
                } else {
                    command.len()
                };
                clip_text = command[start_idx..end_idx].trim();
            } else if lower.starts_with("copy ") {
                clip_text = command.trim_start_matches("copy ").trim();
                if clip_text.to_lowercase().ends_with(" to my clipboard") {
                    clip_text = clip_text[..clip_text.len() - " to my clipboard".len()].trim();
                } else if clip_text.to_lowercase().ends_with(" to clipboard") {
                    clip_text = clip_text[..clip_text.len() - " to clipboard".len()].trim();
                }
            }

            let clean_val = clip_text.trim_matches(['"', '\'']).trim();

            if !clean_val.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "set_clipboard".to_string(),
                    arguments: json!({ "text": clean_val }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 19. Notification requests ("notify me", "show a notification", "send me a notification", etc.)
        if clean_text.starts_with("notify ")
            || clean_text.starts_with("notify me")
            || clean_text == "notify me"
            || clean_text.contains("notification")
            || clean_text.starts_with("send notification")
            || clean_text.starts_with("show notification")
            || clean_text.starts_with("show_notification")
        {
            let mut title = "JARVIS".to_string();
            let message;

            if let Some(titled_idx) = command.to_lowercase().find("titled ") {
                let after_titled = &command[titled_idx + 7..];
                if let Some(saying_idx) = after_titled.to_lowercase().find(" saying ") {
                    title = after_titled[..saying_idx].trim().trim_matches(['"', '\'']).to_string();
                    message = after_titled[saying_idx + 8..].trim().trim_matches(['"', '\'']).to_string();
                } else if let Some(that_idx) = after_titled.to_lowercase().find(" that ") {
                    title = after_titled[..that_idx].trim().trim_matches(['"', '\'']).to_string();
                    message = after_titled[that_idx + 6..].trim().trim_matches(['"', '\'']).to_string();
                } else {
                    message = after_titled.trim().trim_matches(['"', '\'']).to_string();
                }
            } else if let Some(with_title_idx) = command.to_lowercase().find("with title ") {
                let after_title = &command[with_title_idx + 11..];
                if let Some(saying_idx) = after_title.to_lowercase().find(" saying ") {
                    title = after_title[..saying_idx].trim().trim_matches(['"', '\'']).to_string();
                    message = after_title[saying_idx + 8..].trim().trim_matches(['"', '\'']).to_string();
                } else if let Some(that_idx) = after_title.to_lowercase().find(" that ") {
                    title = after_title[..that_idx].trim().trim_matches(['"', '\'']).to_string();
                    message = after_title[that_idx + 6..].trim().trim_matches(['"', '\'']).to_string();
                } else {
                    message = after_title.trim().trim_matches(['"', '\'']).to_string();
                }
            } else if let Some(saying_idx) = command.to_lowercase().find(" saying ") {
                message = command[saying_idx + 8..].trim().trim_matches(['"', '\'']).to_string();
            } else if let Some(that_idx) = command.to_lowercase().find(" that ") {
                message = command[that_idx + 6..].trim().trim_matches(['"', '\'']).to_string();
            } else if clean_text == "notify me"
                || clean_text == "send me a notification"
                || clean_text == "show a notification"
                || clean_text == "show notification"
                || clean_text == "send notification"
                || clean_text == "show_notification"
            {
                message = "Notification from JARVIS".to_string();
            } else {
                let mut rest = command;
                for prefix in &[
                    "send me a notification ",
                    "send a notification ",
                    "show me a notification ",
                    "show a notification ",
                    "show notification ",
                    "send notification ",
                    "notify me ",
                    "notify ",
                ] {
                    if rest.to_lowercase().starts_with(prefix) {
                        rest = &rest[prefix.len()..];
                        break;
                    }
                }
                message = rest.trim().trim_matches(['"', '\'']).to_string();
            }

            if title.is_empty() {
                title = "JARVIS".to_string();
            }

            if !message.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "show_notification".to_string(),
                    arguments: json!({
                        "title": title,
                        "message": message
                    }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 17. Application launch requests ("open <app>", "launch <app>", "start <app>")
        if text.starts_with("open ") || text.starts_with("launch ") || text.starts_with("start ") {
            let app = text
                .trim_start_matches("open ")
                .trim_start_matches("launch ")
                .trim_start_matches("start ")
                .trim();

            if !app.is_empty() {
                return Ok(ParsedIntent {
                    tool_name: "open_application".to_string(),
                    arguments: json!({ "application": app }),
                    raw_command: command.to_string(),
                });
            }
        }

        // 18. Time query
        if text.contains("time") || text.contains("what time") || text.contains("clock") {
            return Ok(ParsedIntent {
                tool_name: "get_time".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 19. Controlled failure for unknown intent (NO automatic fallthrough to open_application!)
        Err(anyhow!("Could not determine intent for command: '{}'", command))
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
                let spoken_response = self.generate_spoken_response(&intent, &result);

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
    fn generate_spoken_response(&self, intent: &ParsedIntent, result: &ToolResult) -> String {
        match intent.tool_name.as_str() {
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
            "close_application" | "kill_process" => {
                let app = result
                    .data
                    .get("target")
                    .or_else(|| result.data.get("application"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("the application");
                let capitalized = {
                    let mut c = app.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                };
                format!("{} has been closed, sir.", capitalized)
            }
            "is_application_running" => {
                let app = result
                    .data
                    .get("target")
                    .or_else(|| result.data.get("application"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("The application");
                let running = result
                    .data
                    .get("running")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let capitalized = {
                    let mut c = app.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                };
                if running {
                    format!("Yes, {} is currently running, sir.", capitalized)
                } else {
                    format!("No, {} is not running, sir.", capitalized)
                }
            }
            "list_processes" => {
                let count = result
                    .data
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                format!("Found {} active processes running on the system, sir.", count)
            }
            "take_screenshot" | "take_screenshot_display" | "take_screenshot_region" => {
                "Screenshot captured, sir.".to_string()
            }
            "get_clipboard" => {
                let text = result.data.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if text.is_empty() {
                    "Your clipboard is currently empty, sir.".to_string()
                } else {
                    format!("Here is what is in your clipboard: {}", text)
                }
            }
            "set_clipboard" => {
                let text = intent.arguments.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if text.is_empty() {
                    "Copied text to your clipboard, sir.".to_string()
                } else {
                    format!("Copied {} to your clipboard, sir.", text)
                }
            }
            "show_notification" => {
                "Notification displayed, sir.".to_string()
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
        pub clipboard: std::sync::Mutex<Option<String>>,
        pub notifications: std::sync::Mutex<Vec<NotificationRequest>>,
    }

    impl MockAdapter {
        pub fn new(fail: bool) -> Self {
            Self {
                fail,
                clipboard: std::sync::Mutex::new(None),
                notifications: std::sync::Mutex::new(Vec::new()),
            }
        }
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
            let guard = self.clipboard.lock().unwrap();
            match &*guard {
                Some(text) if !text.is_empty() => Ok(ClipboardContent::Text(text.clone())),
                _ => Ok(ClipboardContent::Empty),
            }
        }
        async fn set_clipboard(&self, c: ClipboardContent) -> Result<()> {
            let mut guard = self.clipboard.lock().unwrap();
            match c {
                ClipboardContent::Text(t) => *guard = Some(t),
                ClipboardContent::Empty => *guard = None,
                _ => {}
            }
            Ok(())
        }
        async fn show_notification(&self, n: NotificationRequest) -> Result<()> {
            if self.fail {
                anyhow::bail!("Failed to show notification");
            }
            self.notifications.lock().unwrap().push(n);
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
        async fn set_system_volume(&self, _level: u32) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_orchestrator_vertical_slice_open_chrome() {
        let adapter = Arc::new(MockAdapter::new(false));
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
        let adapter = Arc::new(MockAdapter::new(false));
        let mut orchestrator = Orchestrator::new(adapter);
        orchestrator.set_autonomy_level(AutonomyLevel::Level0ChatOnly);

        let outcome = orchestrator.execute_command("open chrome").await;
        assert!(matches!(outcome, ExecutionOutcome::Denied { .. }));
    }

    #[tokio::test]
    async fn test_orchestrator_with_sqlite_persistence() {
        use jarvis_task_engine::SqliteTaskRepository;

        let adapter = Arc::new(MockAdapter::new(false));
        let sqlite_repo = Arc::new(SqliteTaskRepository::open_in_memory().unwrap());
        let orchestrator = Orchestrator::new(adapter).with_repository(sqlite_repo.clone());

        let outcome = orchestrator.execute_command("what time is it").await;
        assert!(matches!(outcome, ExecutionOutcome::Success { .. }));

        // Verify task was durably saved to SQLite repository
        assert_eq!(sqlite_repo.count().await.unwrap(), 1);
        let tasks = sqlite_repo.list().await.unwrap();
        assert_eq!(tasks[0].state, jarvis_task_engine::TaskState::Completed);
    }

    #[test]
    fn test_parse_intent_window_and_app_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Window list
        let intent = orchestrator.parse_intent("list_windows").unwrap();
        assert_eq!(intent.tool_name, "list_windows");

        let intent = orchestrator.parse_intent("show open windows").unwrap();
        assert_eq!(intent.tool_name, "list_windows");

        let intent = orchestrator.parse_intent("what windows are open?").unwrap();
        assert_eq!(intent.tool_name, "list_windows");

        // Active window
        let intent = orchestrator.parse_intent("get_active_window").unwrap();
        assert_eq!(intent.tool_name, "get_active_window");

        let intent = orchestrator.parse_intent("what window is active?").unwrap();
        assert_eq!(intent.tool_name, "get_active_window");

        // Focus
        let intent = orchestrator.parse_intent("focus chrome").unwrap();
        assert_eq!(intent.tool_name, "focus_window");
        assert_eq!(intent.arguments["target"], "chrome");

        // Minimize / Maximize / Restore
        let intent = orchestrator.parse_intent("minimize spotify").unwrap();
        assert_eq!(intent.tool_name, "minimize_window");
        assert_eq!(intent.arguments["target"], "spotify");

        let intent = orchestrator.parse_intent("maximize chrome").unwrap();
        assert_eq!(intent.tool_name, "maximize_window");

        let intent = orchestrator.parse_intent("restore chrome").unwrap();
        assert_eq!(intent.tool_name, "restore_window");

        // Move / Resize
        let intent = orchestrator.parse_intent("move chrome to 100 100").unwrap();
        assert_eq!(intent.tool_name, "move_window");
        assert_eq!(intent.arguments["x"], 100);

        let intent = orchestrator.parse_intent("resize chrome to 1280 by 720").unwrap();
        assert_eq!(intent.tool_name, "resize_window");
        assert_eq!(intent.arguments["width"], 1280);

        // App launch regression
        let intent = orchestrator.parse_intent("open chrome").unwrap();
        assert_eq!(intent.tool_name, "open_application");
        assert_eq!(intent.arguments["application"], "chrome");

        let intent = orchestrator.parse_intent("open spotify").unwrap();
        assert_eq!(intent.tool_name, "open_application");
        assert_eq!(intent.arguments["application"], "spotify");
    }

    #[tokio::test]
    async fn test_unknown_command_does_not_fallback_to_open_application() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Random unknown command
        let result = orchestrator.parse_intent("xyz_random_command_123");
        assert!(result.is_err());

        let outcome = orchestrator.execute_command("xyz_random_command_123").await;
        assert!(matches!(outcome, ExecutionOutcome::Failed { .. }));
    }

    #[test]
    fn test_parse_intent_system_control_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator.parse_intent("set volume 75").unwrap();
        assert_eq!(intent.tool_name, "set_system_volume");
        assert_eq!(intent.arguments["level"], 75);

        let intent = orchestrator.parse_intent("mute").unwrap();
        assert_eq!(intent.tool_name, "mute_system_volume");
        assert_eq!(intent.arguments["mute"], true);

        let intent = orchestrator.parse_intent("unmute").unwrap();
        assert_eq!(intent.tool_name, "mute_system_volume");
        assert_eq!(intent.arguments["mute"], false);

        let intent = orchestrator.parse_intent("lock workstation").unwrap();
        assert_eq!(intent.tool_name, "lock_workstation");

        let intent = orchestrator.parse_intent("shutdown system").unwrap();
        assert_eq!(intent.tool_name, "shutdown_system");

        let intent = orchestrator.parse_intent("restart system").unwrap();
        assert_eq!(intent.tool_name, "restart_system");

        let intent = orchestrator.parse_intent("sleep system").unwrap();
        assert_eq!(intent.tool_name, "sleep_system");

        let intent = orchestrator.parse_intent("system info").unwrap();
        assert_eq!(intent.tool_name, "get_system_info");
    }

    #[tokio::test]
    async fn test_policy_enforcement_shutdown_requires_approval_by_default() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Shutdown system is Critical risk -> ApprovalRequired at default Level3Conservative
        let outcome = orchestrator.execute_command("shutdown system").await;
        assert!(matches!(outcome, ExecutionOutcome::ApprovalRequired { .. }));

        // Volume control is Low risk -> Allowed at default Level3Conservative
        let outcome = orchestrator.execute_command("set volume 50").await;
        assert!(matches!(outcome, ExecutionOutcome::Success { .. }));
    }

    #[test]
    fn test_parse_intent_process_management_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // close_application
        let intent = orchestrator.parse_intent("close chrome").unwrap();
        assert_eq!(intent.tool_name, "close_application");
        assert_eq!(intent.arguments["target"], "chrome");

        let intent = orchestrator.parse_intent("close notepad?").unwrap();
        assert_eq!(intent.tool_name, "close_application");
        assert_eq!(intent.arguments["target"], "notepad");

        // kill_process
        let intent = orchestrator.parse_intent("kill notepad").unwrap();
        assert_eq!(intent.tool_name, "kill_process");
        assert_eq!(intent.arguments["target"], "notepad");

        // list_processes
        let intent = orchestrator.parse_intent("list processes").unwrap();
        assert_eq!(intent.tool_name, "list_processes");

        let intent = orchestrator.parse_intent("list processes?").unwrap();
        assert_eq!(intent.tool_name, "list_processes");

        let intent = orchestrator.parse_intent("show running processes").unwrap();
        assert_eq!(intent.tool_name, "list_processes");

        // is_application_running
        let intent = orchestrator.parse_intent("is notepad running?").unwrap();
        assert_eq!(intent.tool_name, "is_application_running");
        assert_eq!(intent.arguments["target"], "notepad");

        let intent = orchestrator.parse_intent("is chrome running").unwrap();
        assert_eq!(intent.tool_name, "is_application_running");
        assert_eq!(intent.arguments["target"], "chrome");

        let intent = orchestrator.parse_intent("is spotify running?").unwrap();
        assert_eq!(intent.tool_name, "is_application_running");
        assert_eq!(intent.arguments["target"], "spotify");
    }

    #[tokio::test]
    async fn test_orchestrator_execute_process_management_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // close chrome -> RiskLevel::Medium -> Allowed under Level3Conservative
        let outcome = orchestrator.execute_command("close chrome").await;
        if let ExecutionOutcome::Success { spoken_response, .. } = outcome {
            assert_eq!(spoken_response, "Chrome has been closed, sir.");
        } else {
            panic!("Expected Success outcome");
        }

        // list processes -> RiskLevel::Low -> Allowed
        let outcome = orchestrator.execute_command("list processes?").await;
        if let ExecutionOutcome::Success { spoken_response, .. } = outcome {
            assert!(spoken_response.contains("active processes"));
        } else {
            panic!("Expected Success outcome");
        }

        // is notepad running? -> RiskLevel::Low -> Allowed
        let outcome = orchestrator.execute_command("is notepad running?").await;
        if let ExecutionOutcome::Success { spoken_response, .. } = outcome {
            assert!(spoken_response.contains("Notepad"));
        } else {
            panic!("Expected Success outcome");
        }
    }

    #[test]
    fn test_parse_intent_screenshot_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator.parse_intent("take a screenshot").unwrap();
        assert_eq!(intent.tool_name, "take_screenshot");

        let intent = orchestrator.parse_intent("screenshot my screen").unwrap();
        assert_eq!(intent.tool_name, "take_screenshot");

        let intent = orchestrator.parse_intent("take a screenshot of display 2").unwrap();
        assert_eq!(intent.tool_name, "take_screenshot_display");
        assert_eq!(intent.arguments["display_index"], 2);

        let intent = orchestrator.parse_intent("take a screenshot of region 0 0 800 600").unwrap();
        assert_eq!(intent.tool_name, "take_screenshot_region");
        assert_eq!(intent.arguments["width"], 800);
        assert_eq!(intent.arguments["height"], 600);
    }

    #[tokio::test]
    async fn test_orchestrator_execute_screenshot_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("take a screenshot").await;
        if let ExecutionOutcome::Success { spoken_response, tool_data, .. } = outcome {
            assert_eq!(spoken_response, "Screenshot captured, sir.");
            assert_eq!(tool_data["format"], "png");
        } else {
            panic!("Expected Success outcome");
        }
    }

    #[test]
    fn test_parse_intent_clipboard_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator.parse_intent("what is in my clipboard?").unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator.parse_intent("what's in my clipboard").unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator.parse_intent("read my clipboard").unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator.parse_intent("copy hello world to my clipboard").unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator.parse_intent("copy this to my clipboard: hello world").unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator.parse_intent("put hello world in my clipboard").unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator.parse_intent("set clipboard to hello world").unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");
    }

    #[tokio::test]
    async fn test_orchestrator_execute_clipboard_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("copy hello world to my clipboard").await;
        if let ExecutionOutcome::Success { spoken_response, .. } = outcome {
            assert!(spoken_response.contains("Copied hello world"));
        } else {
            panic!("Expected Success outcome");
        }

        let outcome = orchestrator.execute_command("what is in my clipboard").await;
        if let ExecutionOutcome::Success { spoken_response, .. } = outcome {
            assert!(spoken_response.contains("hello world"));
        } else {
            panic!("Expected Success outcome");
        }
    }

    #[test]
    fn test_parse_intent_notification_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator.parse_intent("send me a notification saying hello").unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "JARVIS");
        assert_eq!(intent.arguments["message"], "hello");

        let intent = orchestrator.parse_intent("show a notification titled JARVIS saying hello").unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "JARVIS");
        assert_eq!(intent.arguments["message"], "hello");

        let intent = orchestrator.parse_intent("show a notification titled Build saying complete").unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "Build");
        assert_eq!(intent.arguments["message"], "complete");

        let intent = orchestrator.parse_intent("notify me that the task is complete").unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["message"], "the task is complete");
    }

    #[tokio::test]
    async fn test_orchestrator_execute_notification_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("send me a notification saying hello").await;
        if let ExecutionOutcome::Success { spoken_response, tool_data, .. } = outcome {
            assert_eq!(spoken_response, "Notification displayed, sir.");
            assert_eq!(tool_data["title"], "JARVIS");
        } else {
            panic!("Expected Success outcome");
        }
    }
}
