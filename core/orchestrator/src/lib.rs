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

    /// Centralized normalization of user commands for robust intent pattern matching.
    /// Handles STT transcription variations (capitalization, commas, colons, trailing periods)
    /// while carefully preserving URLs, CSS selectors (#id, .class), and element queries.
    pub fn normalize_command_for_intent_matching(command: &str) -> String {
        let s = command.trim();
        if s.is_empty() {
            return String::new();
        }

        let lower = s.to_lowercase();

        // If it's a URL navigation command ("go to ...", "navigate to ...", "http://...", "https://...")
        // preserve the URL structure (colons, slashes, query params) and only strip trailing sentence punctuation.
        if lower.starts_with("go to ")
            || lower.starts_with("navigate to ")
            || lower.starts_with("browser_navigate ")
            || lower.starts_with("browser navigate ")
            || lower.contains("http://")
            || lower.contains("https://")
        {
            let trimmed = lower.trim_end_matches(['.', '!', '?', ';']).trim();
            return trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        }

        // For general intent matching: replace punctuation (, : ; ! ? " ' ( )) with spaces
        // Note: preserve '#', '_', '-', '[', ']', '=' and '.' when inside selectors or numbers
        let mut result = String::with_capacity(lower.len());
        let chars: Vec<char> = lower.chars().collect();
        let len = chars.len();

        for i in 0..len {
            let c = chars[i];
            match c {
                '\'' => {
                    let prev_is_alnum = i > 0 && chars[i - 1].is_alphanumeric();
                    let next_is_alnum = i + 1 < len && chars[i + 1].is_alphanumeric();
                    if prev_is_alnum && next_is_alnum {
                        result.push('\'');
                    } else {
                        result.push(' ');
                    }
                }
                ',' | ':' | ';' | '!' | '?' | '"' | '(' | ')' => {
                    result.push(' ');
                }
                '.' => {
                    let next_is_alnum = i + 1 < len && chars[i + 1].is_alphanumeric();
                    if next_is_alnum {
                        result.push('.');
                    } else {
                        result.push(' ');
                    }
                }
                _ => {
                    result.push(c);
                }
            }
        }

        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Parse a natural language or structured command into a tool intent.
    pub fn parse_intent(&self, command: &str) -> Result<ParsedIntent> {
        let raw_trimmed = command.trim();
        if raw_trimmed.is_empty() {
            return Err(anyhow!("Empty command"));
        }
        let clean_text = Self::normalize_command_for_intent_matching(command);
        let text = clean_text.clone();

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
        if (text == "get_active_window"
            || text == "what window is active?"
            || text == "what window is active"
            || text == "which window is currently active?"
            || text == "which window is currently active"
            || text.contains("active window")
            || text.contains("current window")
            || text.contains("what window am i using")
            || text.contains("what window am i currently using"))
            && !text.starts_with("inspect")
        {
            return Ok(ParsedIntent {
                tool_name: "get_active_window".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 3. Focus window commands
        if text.starts_with("focus_window ")
            || (text.starts_with("focus ")
                && !text.contains("field")
                && !text.contains("box")
                && !text.contains("input")
                && !text.contains("element")
                && !text.contains("button"))
            || (text.starts_with("switch to ") && !text.contains("tab"))
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

        // 12.5 Browser Status & Navigation requests (M09.01)
        {
            let lower = command.to_lowercase();

            // Browser status requests ("is chrome open", "browser status", "check chrome")
            // 12.5 Browser Navigation & Tab Control requests
            let is_browser_status = clean_text == "browser_status"
                || clean_text == "browser status"
                || clean_text == "check browser status"
                || clean_text == "is chrome open"
                || clean_text == "is chrome open?"
                || clean_text == "is chrome running"
                || clean_text == "is chrome running?"
                || clean_text == "is google chrome open"
                || clean_text == "is google chrome open?"
                || clean_text == "check chrome"
                || clean_text == "is browser open"
                || clean_text == "is browser running";

            if is_browser_status {
                let browser = if clean_text.contains("edge") {
                    "Edge"
                } else if clean_text.contains("firefox") {
                    "Firefox"
                } else if clean_text.contains("brave") {
                    "Brave"
                } else {
                    "Chrome"
                };

                return Ok(ParsedIntent {
                    tool_name: "browser_status".to_string(),
                    arguments: json!({ "browser": browser }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Back ("go back", "back", "browser back", "page back")
            if clean_text == "go back"
                || clean_text == "back"
                || clean_text == "browser back"
                || clean_text == "page back"
                || clean_text == "navigate back"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_back".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Forward ("go forward", "forward", "browser forward", "page forward")
            if clean_text == "go forward"
                || clean_text == "forward"
                || clean_text == "browser forward"
                || clean_text == "page forward"
                || clean_text == "navigate forward"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_forward".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Reload ("reload the page", "refresh the page", "reload chrome", "refresh chrome", "reload page", "refresh page")
            if clean_text == "reload the page"
                || clean_text == "refresh the page"
                || clean_text == "reload page"
                || clean_text == "refresh page"
                || clean_text == "reload chrome"
                || clean_text == "refresh chrome"
                || clean_text == "reload"
                || clean_text == "refresh"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_reload".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Current Page ("what page am i on", "what is the current url", "what is the page title", "current page", "current url")
            if clean_text == "what page am i on"
                || clean_text == "what page am i on?"
                || clean_text == "what is the current url"
                || clean_text == "what is the current url?"
                || clean_text == "what is the page title"
                || clean_text == "what is the page title?"
                || clean_text == "current page"
                || clean_text == "current url"
                || clean_text == "where am i"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_current_page".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser List Tabs ("show my tabs", "list my tabs", "what tabs are open", "list tabs", "show tabs")
            if clean_text == "show my tabs"
                || clean_text == "list my tabs"
                || clean_text == "what tabs are open"
                || clean_text == "what tabs are open?"
                || clean_text == "list tabs"
                || clean_text == "show tabs"
                || clean_text == "my tabs"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_list_tabs".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser New Tab ("open a new tab", "create a new tab", "new tab", "open new tab")
            if clean_text == "open a new tab"
                || clean_text == "create a new tab"
                || clean_text == "new tab"
                || clean_text == "open new tab"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_new_tab".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Switch Tab ("switch to tab 1", "switch to tab 2", "switch to tab google", etc.)
            if clean_text.starts_with("switch to tab ")
                || clean_text.starts_with("switch to the ")
                || clean_text.starts_with("switch tab ")
                || clean_text.starts_with("select tab ")
            {
                let rest = if clean_text.starts_with("switch to tab ") {
                    clean_text.trim_start_matches("switch to tab ")
                } else if clean_text.starts_with("switch to the ") {
                    clean_text
                        .trim_start_matches("switch to the ")
                        .trim_end_matches(" tab")
                } else if clean_text.starts_with("switch tab ") {
                    clean_text.trim_start_matches("switch tab ")
                } else {
                    clean_text.trim_start_matches("select tab ")
                }
                .trim();

                // Convert word numbers (one, two, 1, 2)
                let tab_idx = match rest {
                    "1" | "one" | "first" => Some(1),
                    "2" | "two" | "second" => Some(2),
                    "3" | "three" | "third" => Some(3),
                    "4" | "four" | "fourth" => Some(4),
                    "5" | "five" | "fifth" => Some(5),
                    "6" | "six" | "sixth" => Some(6),
                    "7" | "seven" | "seventh" => Some(7),
                    "8" | "eight" | "eighth" => Some(8),
                    "9" | "nine" | "ninth" => Some(9),
                    _ => rest.parse::<u64>().ok(),
                };

                let args = if let Some(idx) = tab_idx {
                    json!({ "tab_index": idx, "browser": "Chrome" })
                } else {
                    json!({ "title": rest, "browser": "Chrome" })
                };

                return Ok(ParsedIntent {
                    tool_name: "browser_switch_tab".to_string(),
                    arguments: args,
                    raw_command: command.to_string(),
                });
            }

            // Browser Close Tab ("close this tab", "close current tab", "close the current tab", "close tab")
            if clean_text == "close this tab"
                || clean_text == "close current tab"
                || clean_text == "close the current tab"
                || clean_text == "close tab"
            {
                return Ok(ParsedIntent {
                    tool_name: "browser_close_tab".to_string(),
                    arguments: json!({ "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Click Element ("click the search button", "click the login button", "click Sign In button", "click element search_input")
            if clean_text.starts_with("click the ")
                || clean_text.starts_with("click button ")
                || clean_text.starts_with("click element ")
                || (clean_text.starts_with("click ") && !clean_text.starts_with("click_window"))
            {
                let target = clean_text
                    .trim_start_matches("click the button that says ")
                    .trim_start_matches("click the button ")
                    .trim_start_matches("click button ")
                    .trim_start_matches("click element ")
                    .trim_start_matches("click the ")
                    .trim_start_matches("click ")
                    .trim();

                if !target.is_empty() {
                    return Ok(ParsedIntent {
                        tool_name: "browser_click_element".to_string(),
                        arguments: json!({ "query": target, "target": target, "browser": "Chrome" }),
                        raw_command: command.to_string(),
                    });
                }
            }

            // Browser Focus Element ("focus the search box", "focus the username field", "focus search box")
            if (clean_text.starts_with("focus ") || clean_text.starts_with("focus the "))
                && !clean_text.starts_with("focus_window ")
                && !clean_text.starts_with("focus window ")
            {
                let target = clean_text
                    .trim_start_matches("focus the input ")
                    .trim_start_matches("focus input ")
                    .trim_start_matches("focus element ")
                    .trim_start_matches("focus the ")
                    .trim_start_matches("focus ")
                    .trim();

                if !target.is_empty() {
                    return Ok(ParsedIntent {
                        tool_name: "browser_focus_element".to_string(),
                        arguments: json!({ "query": target, "target": target, "browser": "Chrome" }),
                        raw_command: command.to_string(),
                    });
                }
            }

            // Browser Get Element Text ("read text from element", "read the text from the page element", "what does this button say", "get text of element")
            if clean_text == "what does this button say"
                || clean_text.starts_with("read text from ")
                || clean_text.starts_with("read the text from ")
                || clean_text.starts_with("get text of ")
                || clean_text.starts_with("get text from ")
            {
                let target = if clean_text == "what does this button say" {
                    "button"
                } else {
                    clean_text
                        .trim_start_matches("read the text from the page element ")
                        .trim_start_matches("read text from element ")
                        .trim_start_matches("read text from ")
                        .trim_start_matches("read the text from ")
                        .trim_start_matches("get text of element ")
                        .trim_start_matches("get text of ")
                        .trim_start_matches("get text from ")
                        .trim()
                };

                return Ok(ParsedIntent {
                    tool_name: "browser_get_element_text".to_string(),
                    arguments: json!({ "query": target, "target": target, "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Get Element Attributes ("get attributes of element", "get element attributes", "get properties of")
            if clean_text.starts_with("get attributes of ")
                || clean_text.starts_with("get element attributes ")
                || clean_text.starts_with("get properties of ")
            {
                let target = clean_text
                    .trim_start_matches("get attributes of element ")
                    .trim_start_matches("get element attributes ")
                    .trim_start_matches("get attributes of ")
                    .trim_start_matches("get properties of ")
                    .trim();
                return Ok(ParsedIntent {
                    tool_name: "browser_get_element_attributes".to_string(),
                    arguments: json!({ "query": target, "target": target, "browser": "Chrome" }),
                    raw_command: command.to_string(),
                });
            }

            // Browser Find Element ("browser_find_element", "find DOM element search box", "find page element login", "find browser element ...")
            if clean_text.starts_with("browser_find_element")
                || clean_text.starts_with("find dom element ")
                || clean_text.starts_with("find page element ")
                || clean_text.starts_with("find browser element ")
                || clean_text.starts_with("find in browser ")
            {
                let target = clean_text
                    .trim_start_matches("browser_find_element")
                    .trim_start_matches("find dom element ")
                    .trim_start_matches("find page element ")
                    .trim_start_matches("find browser element ")
                    .trim_start_matches("find in browser ")
                    .trim();

                if !target.is_empty() {
                    return Ok(ParsedIntent {
                        tool_name: "browser_find_element".to_string(),
                        arguments: json!({ "query": target, "browser": "Chrome" }),
                        raw_command: command.to_string(),
                    });
                }
            }

            // Browser navigation requests ("go to <URL>", "navigate to <URL>")
            if lower.starts_with("go to ")
                || lower.starts_with("navigate to ")
                || lower.starts_with("browser_navigate ")
                || lower.starts_with("browser navigate ")
            {
                let raw_url = lower
                    .strip_prefix("go to ")
                    .or_else(|| lower.strip_prefix("navigate to "))
                    .or_else(|| lower.strip_prefix("browser_navigate "))
                    .or_else(|| lower.strip_prefix("browser navigate "))
                    .unwrap_or_default();

                let target_url = raw_url.trim().trim_end_matches(['.', '!', '?']).trim();

                if !target_url.is_empty() {
                    return Ok(ParsedIntent {
                        tool_name: "browser_navigate".to_string(),
                        arguments: json!({ "url": target_url, "browser": "Chrome" }),
                        raw_command: command.to_string(),
                    });
                }
            }

            // Open browser requests ("open chrome", "launch chrome", "start chrome", "open browser")
            let is_open_browser = clean_text == "open_browser"
                || clean_text == "open browser"
                || clean_text == "launch browser"
                || clean_text == "open chrome"
                || clean_text == "launch chrome"
                || clean_text == "start chrome"
                || clean_text == "open google chrome"
                || clean_text == "launch google chrome"
                || clean_text == "start google chrome"
                || clean_text == "open edge"
                || clean_text == "open firefox"
                || clean_text == "open brave";

            if is_open_browser {
                let browser = if clean_text.contains("edge") {
                    "Edge"
                } else if clean_text.contains("firefox") {
                    "Firefox"
                } else if clean_text.contains("brave") {
                    "Brave"
                } else {
                    "Chrome"
                };

                return Ok(ParsedIntent {
                    tool_name: "open_browser".to_string(),
                    arguments: json!({ "browser": browser }),
                    raw_command: command.to_string(),
                });
            }
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
                clean_text
                    .trim_start_matches("is_application_running ")
                    .trim()
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
                    .filter_map(|w: &str| w.parse::<i64>().ok())
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
            let lower = &clean_text;

            if lower.starts_with("copy this to my clipboard") {
                clip_text = command
                    .trim_start_matches("copy this to my clipboard")
                    .trim()
                    .trim_start_matches(':')
                    .trim();
            } else if lower.starts_with("set clipboard to ") {
                clip_text = command
                    .trim_start_matches("set clipboard to ")
                    .trim()
                    .trim_start_matches(':')
                    .trim();
            } else if lower.starts_with("set clipboard ") {
                clip_text = command
                    .trim_start_matches("set clipboard ")
                    .trim()
                    .trim_start_matches(':')
                    .trim();
            } else if lower.starts_with("set_clipboard ") {
                clip_text = command
                    .trim_start_matches("set_clipboard ")
                    .trim()
                    .trim_start_matches(':')
                    .trim();
            } else if lower.starts_with("copy ")
                && (lower.ends_with(" to my clipboard") || lower.ends_with(" to clipboard"))
            {
                let start_idx = command
                    .to_lowercase()
                    .find("copy ")
                    .map(|i| i + 5)
                    .unwrap_or(0);
                let end_idx = if command.to_lowercase().ends_with(" to my clipboard") {
                    command.len() - " to my clipboard".len()
                } else if command.to_lowercase().ends_with(" to clipboard") {
                    command.len() - " to clipboard".len()
                } else {
                    command.len()
                };
                clip_text = command[start_idx..end_idx].trim();
            } else if lower.starts_with("put ")
                && (lower.ends_with(" in my clipboard") || lower.ends_with(" in clipboard"))
            {
                let start_idx = command
                    .to_lowercase()
                    .find("put ")
                    .map(|i| i + 4)
                    .unwrap_or(0);
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
                    title = after_titled[..saying_idx]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                    message = after_titled[saying_idx + 8..]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                } else if let Some(that_idx) = after_titled.to_lowercase().find(" that ") {
                    title = after_titled[..that_idx]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                    message = after_titled[that_idx + 6..]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                } else {
                    message = after_titled.trim().trim_matches(['"', '\'']).to_string();
                }
            } else if let Some(with_title_idx) = command.to_lowercase().find("with title ") {
                let after_title = &command[with_title_idx + 11..];
                if let Some(saying_idx) = after_title.to_lowercase().find(" saying ") {
                    title = after_title[..saying_idx]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                    message = after_title[saying_idx + 8..]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                } else if let Some(that_idx) = after_title.to_lowercase().find(" that ") {
                    title = after_title[..that_idx]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                    message = after_title[that_idx + 6..]
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                } else {
                    message = after_title.trim().trim_matches(['"', '\'']).to_string();
                }
            } else if let Some(saying_idx) = command.to_lowercase().find(" saying ") {
                message = command[saying_idx + 8..]
                    .trim()
                    .trim_matches(['"', '\''])
                    .to_string();
            } else if let Some(that_idx) = command.to_lowercase().find(" that ") {
                message = command[that_idx + 6..]
                    .trim()
                    .trim_matches(['"', '\''])
                    .to_string();
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

        // 18a. Vision Screen Element Detection — detect_screen_elements
        {
            let lower = command.to_lowercase();
            let is_detect = (clean_text == "detect_screen_elements"
                || clean_text == "detect screen elements"
                || clean_text == "find elements on screen"
                || clean_text == "find elements on my screen"
                || clean_text == "what elements are on my screen"
                || clean_text == "what elements are visible"
                || clean_text == "what buttons are on my screen"
                || clean_text == "what buttons are visible"
                || clean_text == "what can i click"
                || clean_text == "what can i interact with"
                || lower.starts_with("find the ")
                || lower.starts_with("find a ")
                || lower.starts_with("find an ")
                || lower.starts_with("locate the ")
                || lower.starts_with("locate a ")
                || lower.starts_with("where is the ")
                || lower.starts_with("where is "))
                && !lower.contains("soft reset")
                && !lower.starts_with("inspect");

            if is_detect {
                let query_text = command
                    .trim_start_matches(|c: char| !c.is_alphabetic())
                    .to_string();

                let element_query = {
                    let lower_q = query_text.to_lowercase();
                    let stripped = if let Some(rest) = lower_q.strip_prefix("find the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find a ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find an ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("locate the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("locate a ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("where is the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("where is ") {
                        rest.to_string()
                    } else {
                        String::new()
                    };
                    let cleaned = stripped
                        .trim_end_matches(['?', '.', '!'])
                        .trim()
                        .to_string();
                    if cleaned.is_empty() {
                        None
                    } else {
                        Some(cleaned)
                    }
                };

                return Ok(ParsedIntent {
                    tool_name: "detect_screen_elements".to_string(),
                    arguments: match element_query {
                        Some(q) => json!({ "query": q }),
                        None => json!({}),
                    },
                    raw_command: command.to_string(),
                });
            }
        }

        // 18b. Windows UI Automation Inspection — inspect_ui_tree (M08.04)
        {
            let lower = command.to_lowercase();
            let is_uia = clean_text == "inspect_ui_tree"
                || clean_text == "inspect ui tree"
                || clean_text == "inspect the ui"
                || clean_text == "inspect ui"
                || clean_text == "inspect active window"
                || clean_text == "inspect the current window"
                || clean_text == "inspect current window"
                || clean_text == "inspect active application"
                || clean_text == "inspect window elements"
                || lower.starts_with("inspect ")
                || lower.contains("soft reset");

            if is_uia {
                let query_text = command
                    .trim_start_matches(|c: char| !c.is_alphabetic())
                    .to_string();

                let element_query = {
                    let lower_q = query_text.to_lowercase();
                    let stripped = if let Some(rest) = lower_q.strip_prefix("inspect the ui for ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("inspect ui for ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("inspect ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find a ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find an ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("find ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("locate the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("locate a ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("locate ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("where is the ") {
                        rest.to_string()
                    } else if let Some(rest) = lower_q.strip_prefix("where is ") {
                        rest.to_string()
                    } else {
                        String::new()
                    };
                    let cleaned = stripped
                        .trim_end_matches(['?', '.', '!'])
                        .trim()
                        .to_string();
                    if cleaned.is_empty()
                        || cleaned == "the ui"
                        || cleaned == "ui"
                        || cleaned == "tree"
                        || cleaned == "ui tree"
                    {
                        None
                    } else {
                        Some(cleaned)
                    }
                };

                return Ok(ParsedIntent {
                    tool_name: "inspect_ui_tree".to_string(),
                    arguments: match element_query {
                        Some(q) => json!({ "query": q }),
                        None => json!({}),
                    },
                    raw_command: command.to_string(),
                });
            }
        }

        // 19. Screen description & vision analysis requests
        if clean_text == "describe_screen"
            || clean_text == "describe my screen"
            || clean_text == "describe the screen"
            || clean_text == "describe screen"
            || clean_text == "what is on my screen"
            || clean_text == "what's on my screen"
            || clean_text == "what is visible on my screen"
            || clean_text == "what's visible on my screen"
            || clean_text == "what do you see"
            || clean_text == "what do you see on my screen"
            || clean_text == "look at my screen"
            || clean_text == "analyze my screen"
            || clean_text == "analyze screen"
            || clean_text == "describe my current screen"
            || clean_text.starts_with("describe my screen")
            || clean_text.starts_with("describe the screen")
            || clean_text.starts_with("what is on my screen")
            || clean_text.starts_with("what's on my screen")
            || clean_text.starts_with("what is visible on my screen")
            || clean_text.starts_with("what's visible on my screen")
            || clean_text.starts_with("what application is visible")
            || clean_text.starts_with("what application is open")
        {
            let prompt = if clean_text == "describe my screen"
                || clean_text == "describe the screen"
                || clean_text == "describe screen"
                || clean_text == "what is on my screen"
                || clean_text == "what's on my screen"
                || clean_text == "what do you see"
                || clean_text == "look at my screen"
                || clean_text == "analyze my screen"
                || clean_text == "describe_screen"
            {
                "Describe what is visible on the screen.".to_string()
            } else {
                command.trim().to_string()
            };

            return Ok(ParsedIntent {
                tool_name: "describe_screen".to_string(),
                arguments: json!({ "prompt": prompt }),
                raw_command: command.to_string(),
            });
        }

        // 19. OCR / Text extraction requests — route to canonical "read_screen" tool
        // These MUST route to read_screen, NOT describe_screen.
        // Keep read_screen_text as backward-compatible alias.
        let lower_cmd = command.to_lowercase();
        let is_ocr = clean_text == "read_screen"
            || clean_text == "read_screen_text"
            || clean_text == "read my screen"
            || clean_text == "read the screen"
            || clean_text == "read screen"
            || clean_text == "read screen text"
            || clean_text == "read the text on my screen"
            || clean_text == "read text on my screen"
            || clean_text == "read what's on my screen"
            || clean_text == "read what is on my screen"
            || clean_text == "read whats on my screen"
            || clean_text == "read everything on the screen"
            || clean_text == "what text is visible on my screen"
            || clean_text == "what text is on my screen"
            || clean_text == "what text is visible on screen"
            || clean_text == "what text is on screen"
            || clean_text == "what does the screen say"
            || clean_text == "what does this screen say"
            || clean_text == "what does my screen say"
            || clean_text == "extract the text from my screen"
            || clean_text == "extract text from screen"
            || clean_text == "can you read my screen"
            || clean_text == "tell me what is written on my screen"
            || clean_text == "tell me what text is on my screen"
            || clean_text == "what is written on my screen"
            || lower_cmd.contains("read the text on my screen")
            || lower_cmd.contains("read text on my screen")
            || lower_cmd.contains("what text is visible")
            || lower_cmd.contains("what text is on my screen")
            || lower_cmd.contains("extract the text from my screen")
            || lower_cmd.contains("what does the screen say")
            || lower_cmd.contains("what does this screen say")
            || lower_cmd.contains("what does my screen say")
            || lower_cmd.contains("written on my screen")
            || lower_cmd.contains("read everything on the screen")
            || (lower_cmd.contains("read")
                && lower_cmd.contains("screen")
                && !lower_cmd.contains("describe"));

        if is_ocr {
            return Ok(ParsedIntent {
                tool_name: "read_screen".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 20. Time query
        if text.contains("time") || text.contains("what time") || text.contains("clock") {
            return Ok(ParsedIntent {
                tool_name: "get_time".to_string(),
                arguments: json!({}),
                raw_command: command.to_string(),
            });
        }

        // 21. Controlled failure for unknown intent (NO automatic fallthrough to open_application!)
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
                format!(
                    "Found {} active processes running on the system, sir.",
                    count
                )
            }
            "take_screenshot" | "take_screenshot_display" | "take_screenshot_region" => {
                "Screenshot captured, sir.".to_string()
            }
            "get_clipboard" => {
                let text = result
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if text.is_empty() {
                    "Your clipboard is currently empty, sir.".to_string()
                } else {
                    format!("Here is what is in your clipboard: {}", text)
                }
            }
            "set_clipboard" => {
                let text = intent
                    .arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if text.is_empty() {
                    "Copied text to your clipboard, sir.".to_string()
                } else {
                    format!("Copied {} to your clipboard, sir.", text)
                }
            }
            "show_notification" => "Notification displayed, sir.".to_string(),
            "describe_screen" => {
                let desc = result
                    .data
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Screen description unavailable.");
                desc.to_string()
            }
            "detect_screen_elements" => {
                let limitation = result
                    .data
                    .get("detection_limitation")
                    .and_then(|v| v.as_str());

                let element_count = result
                    .data
                    .get("element_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if let Some(lim) = limitation {
                    format!(
                        "I could not reliably determine pixel coordinates for the screen elements. {}",
                        lim
                    )
                } else if element_count == 0 {
                    "I did not detect any elements on the current screen.".to_string()
                } else {
                    // Build a label list from the elements array
                    let labels: Vec<String> = result
                        .data
                        .get("elements")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|e| {
                                    e.get("label")
                                        .and_then(|l| l.as_str())
                                        .filter(|s| !s.is_empty())
                                        .map(|s| s.to_string())
                                        .or_else(|| {
                                            e.get("type")
                                                .and_then(|t| t.as_str())
                                                .map(|t| t.to_string())
                                        })
                                })
                                .take(5)
                                .collect()
                        })
                        .unwrap_or_default();

                    if labels.is_empty() {
                        format!(
                            "I detected {} UI element{} on the current screen.",
                            element_count,
                            if element_count == 1 { "" } else { "s" }
                        )
                    } else {
                        format!(
                            "I detected {} UI element{} on the screen: {}.",
                            element_count,
                            if element_count == 1 { "" } else { "s" },
                            labels.join(", ")
                        )
                    }
                }
            }
            "inspect_ui_tree" => {
                let win_title = result
                    .data
                    .get("window")
                    .and_then(|w| w.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Active Window");

                let element_count = result
                    .data
                    .get("element_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if element_count == 0 {
                    format!(
                        "I inspected the UI tree of '{}' but found no matching elements, sir.",
                        win_title
                    )
                } else {
                    let labels: Vec<String> = result
                        .data
                        .get("elements")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .map(|e| {
                                    let name = e.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                    let ctype = e
                                        .get("control_type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("Element");
                                    if !name.is_empty() {
                                        format!("{} ({})", name, ctype)
                                    } else {
                                        ctype.to_string()
                                    }
                                })
                                .take(5)
                                .collect()
                        })
                        .unwrap_or_default();

                    if labels.is_empty() {
                        format!("I inspected '{}' via Windows UI Automation and found {} UI element{}, sir.",
                            win_title, element_count, if element_count == 1 { "" } else { "s" })
                    } else {
                        format!(
                            "I inspected '{}' via Windows UI Automation and found {} UI element{}: {}.",
                            win_title,
                            element_count,
                            if element_count == 1 { "" } else { "s" },
                            labels.join(", ")
                        )
                    }
                }
            }
            "browser_status" => {
                let browser = result
                    .data
                    .get("browser")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Chrome");
                let running = result
                    .data
                    .get("running")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let title = result.data.get("window_title").and_then(|v| v.as_str());

                if running {
                    if let Some(t) = title {
                        format!(
                            "{} is currently running with active window '{}', sir.",
                            browser, t
                        )
                    } else {
                        format!("Yes, {} is currently running, sir.", browser)
                    }
                } else {
                    format!("No, {} is not running, sir.", browser)
                }
            }
            "open_browser" => {
                let browser = result
                    .data
                    .get("browser")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Chrome");
                let running = result
                    .data
                    .get("running")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if running {
                    format!("{} is now open and active, sir.", browser)
                } else {
                    format!("Attempted to open {}, sir.", browser)
                }
            }
            "browser_navigate" => {
                let url = result
                    .data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("the requested URL");
                format!("Navigated to {}, sir.", url)
            }
            "browser_back" => "Going back, sir.".to_string(),
            "browser_forward" => "Going forward, sir.".to_string(),
            "browser_reload" => "I've refreshed the page, sir.".to_string(),
            "browser_current_page" => {
                let url = result.data.get("current_url").and_then(|v| v.as_str());
                let title = result
                    .data
                    .get("current_page_title")
                    .and_then(|v| v.as_str());
                match (title, url) {
                    (Some(t), Some(u)) => format!("You are currently on '{}' at {}, sir.", t, u),
                    (Some(t), None) => format!("You are currently on page '{}', sir.", t),
                    (None, Some(u)) => format!("You are currently at {}, sir.", u),
                    (None, None) => "Unable to determine current page URL, sir.".to_string(),
                }
            }
            "browser_list_tabs" => {
                let count = result
                    .data
                    .get("tab_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if count == 0 {
                    "No open browser tabs were found, sir.".to_string()
                } else if count == 1 {
                    "You have 1 open tab, sir.".to_string()
                } else {
                    format!("You have {} tabs open, sir.", count)
                }
            }
            "browser_new_tab" => "Opened a new tab, sir.".to_string(),
            "browser_switch_tab" => {
                if let Some(tab_obj) = result.data.get("tab") {
                    let title = tab_obj
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tab");
                    let id = tab_obj.get("tab_id").and_then(|v| v.as_u64()).unwrap_or(1);
                    format!("Switched to tab {}, '{}', sir.", id, title)
                } else {
                    "Switched tab, sir.".to_string()
                }
            }
            "browser_close_tab" => "Closed the tab, sir.".to_string(),
            "browser_find_element" => {
                let ambiguous = result
                    .data
                    .get("ambiguous")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let match_count = result
                    .data
                    .get("match_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if ambiguous {
                    format!(
                        "Found {} matching elements, sir. Please specify which one to select.",
                        match_count
                    )
                } else if match_count == 0 {
                    "No matching page element was found, sir.".to_string()
                } else {
                    "Found 1 matching element on the page, sir.".to_string()
                }
            }
            "browser_click_element" => "Clicked the element, sir.".to_string(),
            "browser_focus_element" => "Focused the element, sir.".to_string(),
            "browser_get_element_text" => {
                let text = result
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    format!("The element text is: '{}', sir.", text)
                } else {
                    "The element has no text content, sir.".to_string()
                }
            }
            "browser_get_element_attributes" => "Retrieved element attributes, sir.".to_string(),
            "read_screen" => {
                let has_text = result
                    .data
                    .get("has_text")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let text = result
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if has_text && !text.trim().is_empty() {
                    format!("The text on your screen is: {}.", text.trim())
                } else {
                    "The screen does not appear to contain readable text.".to_string()
                }
            }
            "read_screen_text" => {
                let has_text = result
                    .data
                    .get("has_text")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let text = result
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if has_text && !text.trim().is_empty() {
                    format!("The text on your screen is: {}.", text.trim())
                } else {
                    "The screen does not appear to contain readable text.".to_string()
                }
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

        let outcome = orchestrator.execute_command("open notepad").await;

        match outcome {
            ExecutionOutcome::Success {
                spoken_response,
                tool_name,
                tool_data,
                ..
            } => {
                assert_eq!(tool_name, "open_application");
                assert_eq!(spoken_response, "Notepad is open, sir.");
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

        let intent = orchestrator
            .parse_intent("resize chrome to 1280 by 720")
            .unwrap();
        assert_eq!(intent.tool_name, "resize_window");
        assert_eq!(intent.arguments["width"], 1280);

        // App launch regression
        let intent = orchestrator.parse_intent("open notepad").unwrap();
        assert_eq!(intent.tool_name, "open_application");
        assert_eq!(intent.arguments["application"], "notepad");

        let intent = orchestrator.parse_intent("open chrome").unwrap();
        assert_eq!(intent.tool_name, "open_browser");

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

        let intent = orchestrator.parse_intent("is calculator running?").unwrap();
        assert_eq!(intent.tool_name, "is_application_running");
        assert_eq!(intent.arguments["target"], "calculator");

        let intent = orchestrator.parse_intent("is chrome running").unwrap();
        assert_eq!(intent.tool_name, "browser_status");

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
        if let ExecutionOutcome::Success {
            spoken_response, ..
        } = outcome
        {
            assert_eq!(spoken_response, "Chrome has been closed, sir.");
        } else {
            panic!("Expected Success outcome");
        }

        // list processes -> RiskLevel::Low -> Allowed
        let outcome = orchestrator.execute_command("list processes?").await;
        if let ExecutionOutcome::Success {
            spoken_response, ..
        } = outcome
        {
            assert!(spoken_response.contains("active processes"));
        } else {
            panic!("Expected Success outcome");
        }

        // is notepad running? -> RiskLevel::Low -> Allowed
        let outcome = orchestrator.execute_command("is notepad running?").await;
        if let ExecutionOutcome::Success {
            spoken_response, ..
        } = outcome
        {
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

        let intent = orchestrator
            .parse_intent("take a screenshot of display 2")
            .unwrap();
        assert_eq!(intent.tool_name, "take_screenshot_display");
        assert_eq!(intent.arguments["display_index"], 2);

        let intent = orchestrator
            .parse_intent("take a screenshot of region 0 0 800 600")
            .unwrap();
        assert_eq!(intent.tool_name, "take_screenshot_region");
        assert_eq!(intent.arguments["width"], 800);
        assert_eq!(intent.arguments["height"], 600);
    }

    #[tokio::test]
    async fn test_orchestrator_execute_screenshot_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("take a screenshot").await;
        if let ExecutionOutcome::Success {
            spoken_response,
            tool_data,
            ..
        } = outcome
        {
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

        let intent = orchestrator
            .parse_intent("what is in my clipboard?")
            .unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator.parse_intent("what's in my clipboard").unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator.parse_intent("read my clipboard").unwrap();
        assert_eq!(intent.tool_name, "get_clipboard");

        let intent = orchestrator
            .parse_intent("copy hello world to my clipboard")
            .unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator
            .parse_intent("copy this to my clipboard: hello world")
            .unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator
            .parse_intent("put hello world in my clipboard")
            .unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");

        let intent = orchestrator
            .parse_intent("set clipboard to hello world")
            .unwrap();
        assert_eq!(intent.tool_name, "set_clipboard");
        assert_eq!(intent.arguments["text"], "hello world");
    }

    #[tokio::test]
    async fn test_orchestrator_execute_clipboard_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator
            .execute_command("copy hello world to my clipboard")
            .await;
        if let ExecutionOutcome::Success {
            spoken_response, ..
        } = outcome
        {
            assert!(spoken_response.contains("Copied hello world"));
        } else {
            panic!("Expected Success outcome");
        }

        let outcome = orchestrator
            .execute_command("what is in my clipboard")
            .await;
        if let ExecutionOutcome::Success {
            spoken_response, ..
        } = outcome
        {
            assert!(spoken_response.contains("hello world"));
        } else {
            panic!("Expected Success outcome");
        }
    }

    #[test]
    fn test_parse_intent_notification_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator
            .parse_intent("send me a notification saying hello")
            .unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "JARVIS");
        assert_eq!(intent.arguments["message"], "hello");

        let intent = orchestrator
            .parse_intent("show a notification titled JARVIS saying hello")
            .unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "JARVIS");
        assert_eq!(intent.arguments["message"], "hello");

        let intent = orchestrator
            .parse_intent("show a notification titled Build saying complete")
            .unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["title"], "Build");
        assert_eq!(intent.arguments["message"], "complete");

        let intent = orchestrator
            .parse_intent("notify me that the task is complete")
            .unwrap();
        assert_eq!(intent.tool_name, "show_notification");
        assert_eq!(intent.arguments["message"], "the task is complete");
    }

    #[tokio::test]
    async fn test_orchestrator_execute_notification_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator
            .execute_command("send me a notification saying hello")
            .await;
        if let ExecutionOutcome::Success {
            spoken_response,
            tool_data,
            ..
        } = outcome
        {
            assert_eq!(spoken_response, "Notification displayed, sir.");
            assert_eq!(tool_data["title"], "JARVIS");
        } else {
            panic!("Expected Success outcome");
        }
    }

    #[test]
    fn test_parse_intent_describe_screen_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let commands = vec![
            "describe my screen",
            "describe the screen",
            "what is on my screen?",
            "what is visible on my screen",
            "what do you see",
            "look at my screen",
            "analyze my screen",
        ];

        for cmd in commands {
            let intent = orchestrator.parse_intent(cmd).unwrap();
            assert_eq!(intent.tool_name, "describe_screen");
        }

        let custom_intent = orchestrator
            .parse_intent("what application is visible on my screen?")
            .unwrap();
        assert_eq!(custom_intent.tool_name, "describe_screen");
        assert!(custom_intent.arguments["prompt"]
            .as_str()
            .unwrap()
            .contains("application"));
    }

    #[test]
    fn test_parse_intent_read_screen_text_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Spec-required phrases that must route to read_screen
        let ocr_commands = vec![
            "read my screen",
            "read the screen",
            "read the text on my screen",
            "what text is on my screen",
            "what text is visible on my screen",
            "what does the screen say",
            "what does my screen say",
            "what does this screen say",
            "extract the text from my screen",
            "read everything on the screen",
            "can you read my screen",
            "tell me what is written on my screen",
            "Can you tell me what text is visible on my screen?",
            "Could you read what's on my screen?",
            "Please read the text on my screen.",
        ];

        for cmd in ocr_commands {
            let intent = orchestrator.parse_intent(cmd).unwrap();
            assert_eq!(
                intent.tool_name, "read_screen",
                "Command failed to parse as read_screen: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_visual_vs_ocr_separation() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Visual requests -> describe_screen
        let visual_cmd = orchestrator
            .parse_intent("what do you see on my screen")
            .unwrap();
        assert_eq!(visual_cmd.tool_name, "describe_screen");

        let visual_cmd2 = orchestrator.parse_intent("describe my screen").unwrap();
        assert_eq!(visual_cmd2.tool_name, "describe_screen");

        // Text requests -> read_screen (canonical)
        let ocr_cmd = orchestrator
            .parse_intent("what text is visible on my screen")
            .unwrap();
        assert_eq!(ocr_cmd.tool_name, "read_screen");

        let ocr_cmd2 = orchestrator.parse_intent("read my screen").unwrap();
        assert_eq!(ocr_cmd2.tool_name, "read_screen");

        let ocr_cmd3 = orchestrator
            .parse_intent("what does my screen say")
            .unwrap();
        assert_eq!(ocr_cmd3.tool_name, "read_screen");

        let ocr_cmd4 = orchestrator
            .parse_intent("read everything on the screen")
            .unwrap();
        assert_eq!(ocr_cmd4.tool_name, "read_screen");
    }

    // ============================================================
    // detect_screen_elements orchestrator tests (M08.04)
    // ============================================================

    #[test]
    fn test_parse_intent_detect_screen_elements_exact_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let exact_commands = vec![
            "detect_screen_elements",
            "detect screen elements",
            "what buttons are on my screen",
            "what buttons are visible",
            "what can i click",
            "what elements are on my screen",
            "what elements are visible",
            "find elements on screen",
            "find elements on my screen",
        ];

        for cmd in exact_commands {
            let intent = orchestrator.parse_intent(cmd).unwrap();
            assert_eq!(
                intent.tool_name, "detect_screen_elements",
                "Expected detect_screen_elements for: {:?}",
                cmd
            );
        }
    }

    #[test]
    fn test_parse_intent_detect_screen_elements_find_prefix_extracts_query() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator.parse_intent("find the Chrome icon").unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
        assert_eq!(intent.arguments["query"].as_str(), Some("chrome icon"));

        let intent = orchestrator.parse_intent("find the search box").unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
        assert_eq!(intent.arguments["query"].as_str(), Some("search box"));

        let intent = orchestrator.parse_intent("where is Chrome").unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
        // query should contain "chrome"
        assert!(intent.arguments["query"]
            .as_str()
            .unwrap_or("")
            .contains("chrome"));

        let intent = orchestrator.parse_intent("where is the taskbar").unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
        assert_eq!(intent.arguments["query"].as_str(), Some("taskbar"));

        let intent = orchestrator
            .parse_intent("locate the close button")
            .unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
        assert_eq!(intent.arguments["query"].as_str(), Some("close button"));
    }

    #[test]
    fn test_parse_intent_detect_screen_elements_no_query_when_generic() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // Generic element queries have no specific query argument
        let intent = orchestrator
            .parse_intent("what buttons are on my screen")
            .unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");

        let intent = orchestrator.parse_intent("what can i click").unwrap();
        assert_eq!(intent.tool_name, "detect_screen_elements");
    }

    #[test]
    fn test_visual_vs_ocr_vs_detect_three_way_separation() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // 1. Visual description -> describe_screen
        let v = orchestrator.parse_intent("describe my screen").unwrap();
        assert_eq!(v.tool_name, "describe_screen");

        let v2 = orchestrator
            .parse_intent("what do you see on my screen")
            .unwrap();
        assert_eq!(v2.tool_name, "describe_screen");

        // 2. Text extraction -> read_screen
        let r = orchestrator.parse_intent("read my screen").unwrap();
        assert_eq!(r.tool_name, "read_screen");

        let r2 = orchestrator
            .parse_intent("what text is on my screen")
            .unwrap();
        assert_eq!(r2.tool_name, "read_screen");

        // 3. Element detection -> detect_screen_elements
        let d = orchestrator.parse_intent("find the Chrome icon").unwrap();
        assert_eq!(d.tool_name, "detect_screen_elements");

        let d2 = orchestrator
            .parse_intent("what buttons are on my screen")
            .unwrap();
        assert_eq!(d2.tool_name, "detect_screen_elements");

        let d3 = orchestrator
            .parse_intent("where is the search box")
            .unwrap();
        assert_eq!(d3.tool_name, "detect_screen_elements");
    }

    #[test]
    fn test_parse_intent_inspect_ui_tree_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let inspect_commands = vec![
            "inspect_ui_tree",
            "inspect ui tree",
            "inspect the ui",
            "inspect ui",
            "inspect active window",
            "inspect the current window",
        ];

        for cmd in inspect_commands {
            let intent = orchestrator.parse_intent(cmd).unwrap();
            assert_eq!(
                intent.tool_name, "inspect_ui_tree",
                "Expected inspect_ui_tree for: {:?}",
                cmd
            );
        }
    }

    #[test]
    fn test_parse_intent_inspect_ui_tree_extracts_query() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let intent = orchestrator
            .parse_intent("find the Soft Reset button")
            .unwrap();
        assert_eq!(intent.tool_name, "inspect_ui_tree");
        assert_eq!(
            intent.arguments["query"].as_str(),
            Some("soft reset button")
        );

        let intent = orchestrator.parse_intent("inspect search box").unwrap();
        assert_eq!(intent.tool_name, "inspect_ui_tree");
        assert_eq!(intent.arguments["query"].as_str(), Some("search box"));

        let intent = orchestrator
            .parse_intent("inspect the UI for buttons")
            .unwrap();
        assert_eq!(intent.tool_name, "inspect_ui_tree");
        assert_eq!(intent.arguments["query"].as_str(), Some("buttons"));
    }

    #[tokio::test]
    async fn test_orchestrator_inspect_ui_tree_executes() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("inspect the UI").await;
        if let ExecutionOutcome::Success { tool_name, .. } = outcome {
            assert_eq!(tool_name, "inspect_ui_tree");
        } else {
            panic!("Expected Success outcome for inspect the UI");
        }
    }

    #[test]
    fn test_parse_intent_browser_commands() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let status_cmd = orchestrator.parse_intent("is Chrome open").unwrap();
        assert_eq!(status_cmd.tool_name, "browser_status");

        let status_cmd2 = orchestrator.parse_intent("browser status").unwrap();
        assert_eq!(status_cmd2.tool_name, "browser_status");

        let open_cmd = orchestrator.parse_intent("open Chrome").unwrap();
        assert_eq!(open_cmd.tool_name, "open_browser");

        let open_cmd2 = orchestrator.parse_intent("launch chrome").unwrap();
        assert_eq!(open_cmd2.tool_name, "open_browser");

        let nav_cmd = orchestrator
            .parse_intent("go to https://www.google.com")
            .unwrap();
        assert_eq!(nav_cmd.tool_name, "browser_navigate");
        assert_eq!(
            nav_cmd.arguments["url"].as_str(),
            Some("https://www.google.com")
        );

        let nav_cmd2 = orchestrator
            .parse_intent("navigate to linkedin.com")
            .unwrap();
        assert_eq!(nav_cmd2.tool_name, "browser_navigate");
        assert_eq!(nav_cmd2.arguments["url"].as_str(), Some("linkedin.com"));

        let back_cmd = orchestrator.parse_intent("go back").unwrap();
        assert_eq!(back_cmd.tool_name, "browser_back");

        let fwd_cmd = orchestrator.parse_intent("go forward").unwrap();
        assert_eq!(fwd_cmd.tool_name, "browser_forward");

        let reload_cmd = orchestrator.parse_intent("reload the page").unwrap();
        assert_eq!(reload_cmd.tool_name, "browser_reload");

        let curr_cmd = orchestrator.parse_intent("what page am i on").unwrap();
        assert_eq!(curr_cmd.tool_name, "browser_current_page");

        let tabs_cmd = orchestrator.parse_intent("show my tabs").unwrap();
        assert_eq!(tabs_cmd.tool_name, "browser_list_tabs");

        let new_tab_cmd = orchestrator.parse_intent("open a new tab").unwrap();
        assert_eq!(new_tab_cmd.tool_name, "browser_new_tab");

        let switch_tab_cmd = orchestrator.parse_intent("switch to tab 2").unwrap();
        assert_eq!(switch_tab_cmd.tool_name, "browser_switch_tab");
        assert_eq!(switch_tab_cmd.arguments["tab_index"], 2);

        let close_tab_cmd = orchestrator.parse_intent("close this tab").unwrap();
        assert_eq!(close_tab_cmd.tool_name, "browser_close_tab");

        let find_cmd = orchestrator
            .parse_intent("find page element search box")
            .unwrap();
        assert_eq!(find_cmd.tool_name, "browser_find_element");
        assert_eq!(find_cmd.arguments["query"].as_str(), Some("search box"));

        let click_cmd = orchestrator.parse_intent("click the login button").unwrap();
        assert_eq!(click_cmd.tool_name, "browser_click_element");
        assert_eq!(click_cmd.arguments["target"].as_str(), Some("login button"));

        let focus_cmd = orchestrator
            .parse_intent("focus the username field")
            .unwrap();
        assert_eq!(focus_cmd.tool_name, "browser_focus_element");
        assert_eq!(
            focus_cmd.arguments["target"].as_str(),
            Some("username field")
        );

        let text_cmd = orchestrator
            .parse_intent("read text from element Submit")
            .unwrap();
        assert_eq!(text_cmd.tool_name, "browser_get_element_text");
        assert_eq!(text_cmd.arguments["target"].as_str(), Some("submit"));

        let attr_cmd = orchestrator
            .parse_intent("get attributes of element Submit")
            .unwrap();
        assert_eq!(attr_cmd.tool_name, "browser_get_element_attributes");
        assert_eq!(attr_cmd.arguments["target"].as_str(), Some("submit"));
    }

    #[test]
    fn test_stt_punctuation_intent_variations() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        // STT Command 1: "Find DOM element Submit"
        let cmd1 = orchestrator
            .parse_intent("Find DOM element Submit")
            .unwrap();
        assert_eq!(cmd1.tool_name, "browser_find_element");
        assert_eq!(cmd1.arguments["query"].as_str(), Some("submit"));

        // STT Command 2: "Find DOM element, Submit"
        let cmd2 = orchestrator
            .parse_intent("Find DOM element, Submit")
            .unwrap();
        assert_eq!(cmd2.tool_name, "browser_find_element");
        assert_eq!(cmd2.arguments["query"].as_str(), Some("submit"));

        // STT Command 3: "Find Dom element, submit." (Original STT reported failure)
        let cmd3 = orchestrator
            .parse_intent("Find Dom element, submit.")
            .unwrap();
        assert_eq!(cmd3.tool_name, "browser_find_element");
        assert_eq!(cmd3.arguments["query"].as_str(), Some("submit"));

        // STT Command 4: "Find DOM element Submit."
        let cmd4 = orchestrator
            .parse_intent("Find DOM element Submit.")
            .unwrap();
        assert_eq!(cmd4.tool_name, "browser_find_element");
        assert_eq!(cmd4.arguments["query"].as_str(), Some("submit"));

        // STT Command 5: "Find DOM element: Submit!"
        let cmd5 = orchestrator
            .parse_intent("Find DOM element: Submit!")
            .unwrap();
        assert_eq!(cmd5.tool_name, "browser_find_element");
        assert_eq!(cmd5.arguments["query"].as_str(), Some("submit"));

        // STT Command 6: "click Submit."
        let cmd6 = orchestrator.parse_intent("click Submit.").unwrap();
        assert_eq!(cmd6.tool_name, "browser_click_element");
        assert_eq!(cmd6.arguments["target"].as_str(), Some("submit"));

        // STT Command 7: "focus Name input."
        let cmd7 = orchestrator.parse_intent("focus Name input.").unwrap();
        assert_eq!(cmd7.tool_name, "browser_focus_element");
        assert_eq!(cmd7.arguments["target"].as_str(), Some("name input"));

        // STT Command 8: "read text from element, JARVIS DOM TEST."
        let cmd8 = orchestrator
            .parse_intent("read text from element, JARVIS DOM TEST.")
            .unwrap();
        assert_eq!(cmd8.tool_name, "browser_get_element_text");
        assert_eq!(cmd8.arguments["target"].as_str(), Some("jarvis dom test"));

        // STT Command 9: "get attributes of element, Submit."
        let cmd9 = orchestrator
            .parse_intent("get attributes of element, Submit.")
            .unwrap();
        assert_eq!(cmd9.tool_name, "browser_get_element_attributes");
        assert_eq!(cmd9.arguments["target"].as_str(), Some("submit"));

        // URL preservation test: "go to https://example.com/path?a=1&b=2."
        let cmd10 = orchestrator
            .parse_intent("go to https://example.com/path?a=1&b=2.")
            .unwrap();
        assert_eq!(cmd10.tool_name, "browser_navigate");
        assert_eq!(
            cmd10.arguments["url"].as_str(),
            Some("https://example.com/path?a=1&b=2")
        );

        // CSS Selector test: "find DOM element #name-input."
        let cmd11 = orchestrator
            .parse_intent("find DOM element #name-input.")
            .unwrap();
        assert_eq!(cmd11.tool_name, "browser_find_element");
        assert_eq!(cmd11.arguments["query"].as_str(), Some("#name-input"));
    }

    #[tokio::test]
    async fn test_orchestrator_browser_tools_execute() {
        let adapter = Arc::new(MockAdapter::new(false));
        let orchestrator = Orchestrator::new(adapter);

        let outcome = orchestrator.execute_command("is Chrome open").await;
        if let ExecutionOutcome::Success { tool_name, .. } = outcome {
            assert_eq!(tool_name, "browser_status");
        } else {
            panic!("Expected Success outcome for is Chrome open");
        }
    }
}
