//! JARVIS Tool Runtime & Registry
//!
//! Provides the execution framework for all actions that JARVIS can perform.
//! Every tool call is schema-validated, policy-checked, executed within an isolated context,
//! and audited with duration and result metrics.
//!
//! # Architecture
//!
//! ```text
//! ToolRequest
//!     ↓
//! ToolRegistry::execute(request, ctx)
//!     ↓
//! Schema Validation
//!     ↓
//! Tool::execute(&self, request, ctx)
//!     ↓
//! PlatformAdapter (OS operations)
//!     ↓
//! ToolResult
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 7 / Vertical Slice 1 Foundation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{error, info, instrument};
use uuid::Uuid;

use jarvis_platform::{PlatformAdapter, Rect};
use jarvis_policy::RiskLevel;

// ============================================================
// Tool Data Models
// ============================================================

/// Metadata describing a JARVIS tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool identifier (e.g. "open_application")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what the tool does (used by planner / LLM)
    pub description: String,
    /// Expected parameter JSON schema
    pub parameters_schema: Value,
    /// Inherent risk level
    pub risk_level: RiskLevel,
    /// Required capability tags
    pub required_permissions: Vec<String>,
    /// Default execution timeout in seconds
    pub timeout_secs: u64,
}

/// A request to execute a specific tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Request correlation ID
    pub request_id: String,
    /// Associated task ID if part of a workflow
    pub task_id: Option<String>,
    /// Name of the tool to invoke
    pub tool_name: String,
    /// Arguments encoded as a JSON object
    pub arguments: Value,
    /// Caller identifier (e.g. "orchestrator", "user", "cli")
    pub invoked_by: String,
}

impl ToolRequest {
    pub fn new(tool_name: impl Into<String>, arguments: Value) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            task_id: None,
            tool_name: tool_name.into(),
            arguments,
            invoked_by: "system".to_string(),
        }
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }
}

/// The structured result returned from tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub request_id: String,
    pub tool_name: String,
    pub success: bool,
    pub data: Value,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

impl ToolResult {
    pub fn success(request_id: String, tool_name: String, data: Value, elapsed_ms: u64) -> Self {
        Self {
            request_id,
            tool_name,
            success: true,
            data,
            error: None,
            execution_time_ms: elapsed_ms,
        }
    }

    pub fn failure(
        request_id: String,
        tool_name: String,
        error_msg: String,
        elapsed_ms: u64,
    ) -> Self {
        Self {
            request_id,
            tool_name,
            success: false,
            data: Value::Null,
            error: Some(error_msg),
            execution_time_ms: elapsed_ms,
        }
    }
}

/// Errors occurring within the tool runtime.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool '{0}' not found in registry")]
    NotFound(String),
    #[error("Invalid arguments for tool '{tool}': {details}")]
    InvalidArguments { tool: String, details: String },
    #[error("Execution failed for tool '{tool}': {cause}")]
    ExecutionFailed { tool: String, cause: String },
    #[error("Tool '{tool}' timed out after {timeout_secs}s")]
    Timeout { tool: String, timeout_secs: u64 },
}

/// Execution context passed to tools during invocation.
pub struct ToolExecutionContext {
    pub platform_adapter: Arc<dyn PlatformAdapter>,
}

impl ToolExecutionContext {
    pub fn new(platform_adapter: Arc<dyn PlatformAdapter>) -> Self {
        Self { platform_adapter }
    }
}

// ============================================================
// Tool Trait
// ============================================================

/// Trait implemented by every executable JARVIS tool.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Return the metadata definition for this tool.
    fn definition(&self) -> &ToolDefinition;

    /// Execute the tool with given request and context.
    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError>;
}

// ============================================================
// Built-in Tools: OpenApplicationTool
// ============================================================

/// Tool to launch a desktop application.
pub struct OpenApplicationTool {
    definition: ToolDefinition,
}

impl OpenApplicationTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "open_application".to_string(),
                name: "Open Application".to_string(),
                description: "Launches a desktop application by name or executable path (e.g. chrome, notepad, vscode)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "application": {
                            "type": "string",
                            "description": "Name or path of the application to open (e.g. 'chrome', 'notepad')"
                        }
                    },
                    "required": ["application"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["process.launch".to_string()],
                timeout_secs: 15,
            },
        }
    }
}

impl Default for OpenApplicationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OpenApplicationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    #[instrument(skip(self, ctx), fields(req_id = %request.request_id, tool = "open_application"))]
    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        // Extract application argument
        let app_name = request
            .arguments
            .get("application")
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "open_application".to_string(),
                details: "Missing required argument 'application'".to_string(),
            })?;

        info!(application = %app_name, "Executing open_application via PlatformAdapter");

        // Invoke platform adapter
        match ctx.platform_adapter.open_application(app_name, None).await {
            Ok(process_info) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let data = json!({
                    "application": app_name,
                    "pid": process_info.pid,
                    "running": process_info.running,
                    "status_message": format!("{} is now open", app_name)
                });
                info!(application = %app_name, pid = process_info.pid, elapsed_ms = elapsed, "Application opened successfully");
                Ok(ToolResult::success(
                    request.request_id,
                    "open_application".to_string(),
                    data,
                    elapsed,
                ))
            }
            Err(e) => {
                let elapsed = start.elapsed().as_millis() as u64;
                error!(application = %app_name, error = %e, "Failed to open application");
                Ok(ToolResult::failure(
                    request.request_id,
                    "open_application".to_string(),
                    e.to_string(),
                    elapsed,
                ))
            }
        }
    }
}

// ============================================================
// Built-in Tools: GetTimeTool
// ============================================================

pub struct GetTimeTool {
    definition: ToolDefinition,
}

impl GetTimeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "get_time".to_string(),
                name: "Get Current Time".to_string(),
                description: "Returns the current local date, time, and timezone".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec![],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for GetTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetTimeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let now = chrono::Local::now();
        let data = json!({
            "iso": now.to_rfc3339(),
            "time": now.format("%I:%M %p").to_string(),
            "date": now.format("%A, %B %d, %Y").to_string(),
            "timezone": now.format("%Z").to_string(),
        });
        Ok(ToolResult::success(
            request.request_id,
            "get_time".to_string(),
            data,
            start.elapsed().as_millis() as u64,
        ))
    }
}

// ============================================================
// Built-in Tools: Window Management Tools
// ============================================================

/// Tool to list all open top-level windows.
pub struct ListWindowsTool {
    definition: ToolDefinition,
}

impl ListWindowsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "list_windows".to_string(),
                name: "List Windows".to_string(),
                description: "Enumerates all visible open application windows on the desktop".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["window.list".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for ListWindowsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListWindowsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        match ctx.platform_adapter.list_windows().await {
            Ok(windows) => {
                let data = json!({
                    "windows": windows,
                    "count": windows.len()
                });
                Ok(ToolResult::success(
                    request.request_id,
                    "list_windows".to_string(),
                    data,
                    start.elapsed().as_millis() as u64,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "list_windows".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to detect the active foreground window.
pub struct GetActiveWindowTool {
    definition: ToolDefinition,
}

impl GetActiveWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "get_active_window".to_string(),
                name: "Get Active Window".to_string(),
                description: "Retrieves details of the currently focused foreground desktop window".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["window.inspect".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for GetActiveWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetActiveWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        match ctx.platform_adapter.get_active_window().await {
            Ok(active) => {
                let data = json!({
                    "active_window": active
                });
                Ok(ToolResult::success(
                    request.request_id,
                    "get_active_window".to_string(),
                    data,
                    start.elapsed().as_millis() as u64,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "get_active_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to bring a window into focus foreground.
pub struct FocusWindowTool {
    definition: ToolDefinition,
}

impl FocusWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "focus_window".to_string(),
                name: "Focus Window".to_string(),
                description: "Brings a window to the foreground by handle or application name".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "window_handle": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.focus".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for FocusWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FocusWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("window_handle")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "focus_window".to_string(),
                details: "Missing required argument 'window_handle' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.focus_window(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "focus_window".to_string(),
                json!({ "target": target, "status_message": format!("Focused {}", target) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "focus_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to minimize a window.
pub struct MinimizeWindowTool {
    definition: ToolDefinition,
}

impl MinimizeWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "minimize_window".to_string(),
                name: "Minimize Window".to_string(),
                description: "Minimizes a window by handle or application name".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "window_handle": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.state".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for MinimizeWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MinimizeWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("window_handle")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "minimize_window".to_string(),
                details: "Missing required argument 'window_handle' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.minimize_window(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "minimize_window".to_string(),
                json!({ "target": target, "status_message": format!("Minimized {}", target) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "minimize_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to maximize a window.
pub struct MaximizeWindowTool {
    definition: ToolDefinition,
}

impl MaximizeWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "maximize_window".to_string(),
                name: "Maximize Window".to_string(),
                description: "Maximizes a window by handle or application name".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "window_handle": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.state".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for MaximizeWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MaximizeWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("window_handle")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "maximize_window".to_string(),
                details: "Missing required argument 'window_handle' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.maximize_window(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "maximize_window".to_string(),
                json!({ "target": target, "status_message": format!("Maximized {}", target) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "maximize_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to restore a window to normal windowed state.
pub struct RestoreWindowTool {
    definition: ToolDefinition,
}

impl RestoreWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "restore_window".to_string(),
                name: "Restore Window".to_string(),
                description: "Restores a minimized or maximized window to normal windowed state".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "window_handle": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.state".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for RestoreWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RestoreWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("window_handle")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "restore_window".to_string(),
                details: "Missing required argument 'window_handle' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.restore_window(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "restore_window".to_string(),
                json!({ "target": target, "status_message": format!("Restored {}", target) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "restore_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to resize and/or move a window.
pub struct SetWindowBoundsTool {
    definition: ToolDefinition,
}

impl SetWindowBoundsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "set_window_bounds".to_string(),
                name: "Set Window Bounds".to_string(),
                description: "Moves and resizes a window to specified coordinates (x, y, width, height)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "window_handle": { "type": "string" },
                        "application": { "type": "string" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    },
                    "required": ["x", "y", "width", "height"]
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.bounds".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for SetWindowBoundsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SetWindowBoundsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("window_handle"))
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "set_window_bounds".to_string(),
                details: "Missing required argument 'target' or 'window_handle'".to_string(),
            })?;

        let x = request.arguments.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = request.arguments.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let width = request.arguments.get("width").and_then(|v| v.as_u64()).unwrap_or(800) as u32;
        let height = request.arguments.get("height").and_then(|v| v.as_u64()).unwrap_or(600) as u32;

        let bounds = jarvis_platform::Rect { x, y, width, height };

        match ctx.platform_adapter.set_window_bounds(target, bounds).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "set_window_bounds".to_string(),
                json!({ "target": target, "x": x, "y": y, "width": width, "height": height }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "set_window_bounds".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to move a window to specified coordinates (x, y).
pub struct MoveWindowTool {
    definition: ToolDefinition,
}

impl MoveWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "move_window".to_string(),
                name: "Move Window".to_string(),
                description: "Moves a window to specified desktop coordinates (x, y)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["target", "x", "y"]
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.bounds".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for MoveWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MoveWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("window_handle"))
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "move_window".to_string(),
                details: "Missing required argument 'target'".to_string(),
            })?;

        let x = request.arguments.get("x").and_then(|v| v.as_i64()).unwrap_or(100) as i32;
        let y = request.arguments.get("y").and_then(|v| v.as_i64()).unwrap_or(100) as i32;

        let bounds = jarvis_platform::Rect { x, y, width: 800, height: 600 };

        match ctx.platform_adapter.set_window_bounds(target, bounds).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "move_window".to_string(),
                json!({ "target": target, "x": x, "y": y, "status_message": format!("Moved {} to ({}, {})", target, x, y) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "move_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to resize a window to specified dimensions (width, height).
pub struct ResizeWindowTool {
    definition: ToolDefinition,
}

impl ResizeWindowTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "resize_window".to_string(),
                name: "Resize Window".to_string(),
                description: "Resizes a window to specified dimensions (width, height)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    },
                    "required": ["target", "width", "height"]
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["window.bounds".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for ResizeWindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ResizeWindowTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("window_handle"))
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "resize_window".to_string(),
                details: "Missing required argument 'target'".to_string(),
            })?;

        let width = request.arguments.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
        let height = request.arguments.get("height").and_then(|v| v.as_u64()).unwrap_or(720) as u32;

        let bounds = jarvis_platform::Rect { x: 100, y: 100, width, height };

        match ctx.platform_adapter.set_window_bounds(target, bounds).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "resize_window".to_string(),
                json!({ "target": target, "width": width, "height": height, "status_message": format!("Resized {} to {}x{}", target, width, height) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "resize_window".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Built-in Tools: System Control Tools
// ============================================================

/// Tool to set system audio volume (0..100).
pub struct SetSystemVolumeTool {
    definition: ToolDefinition,
}

impl SetSystemVolumeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "set_system_volume".to_string(),
                name: "Set System Volume".to_string(),
                description: "Sets system master volume to specified percentage (0 to 100)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "level": { "type": "integer", "minimum": 0, "maximum": 100 }
                    },
                    "required": ["level"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["system.audio".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for SetSystemVolumeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SetSystemVolumeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let level = request
            .arguments
            .get("level")
            .or_else(|| request.arguments.get("volume"))
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as u32;

        match ctx.platform_adapter.set_system_volume(level).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "set_system_volume".to_string(),
                json!({ "level": level, "status_message": format!("System volume set to {}%", level) }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "set_system_volume".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to mute or unmute system audio.
pub struct MuteSystemVolumeTool {
    definition: ToolDefinition,
}

impl MuteSystemVolumeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "mute_system_volume".to_string(),
                name: "Mute System Volume".to_string(),
                description: "Mutes or unmutes system master audio".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "mute": { "type": "boolean" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["system.audio".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for MuteSystemVolumeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MuteSystemVolumeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let mute = request
            .arguments
            .get("mute")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        match ctx.platform_adapter.set_system_mute(mute).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "mute_system_volume".to_string(),
                json!({ "muted": mute, "status_message": if mute { "System muted" } else { "System unmuted" } }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "mute_system_volume".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to lock the desktop workstation.
pub struct LockWorkstationTool {
    definition: ToolDefinition,
}

impl LockWorkstationTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "lock_workstation".to_string(),
                name: "Lock Workstation".to_string(),
                description: "Locks the desktop user session/workstation".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["system.lock".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for LockWorkstationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LockWorkstationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        match ctx.platform_adapter.lock_workstation().await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "lock_workstation".to_string(),
                json!({ "status_message": "Workstation locked" }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "lock_workstation".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to shutdown the operating system (Critical risk).
pub struct ShutdownSystemTool {
    definition: ToolDefinition,
}

impl ShutdownSystemTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "shutdown_system".to_string(),
                name: "Shutdown System".to_string(),
                description: "Initiates system shutdown (requires user approval)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "force": { "type": "boolean" }
                    }
                }),
                risk_level: RiskLevel::Critical,
                required_permissions: vec!["system.power".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for ShutdownSystemTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShutdownSystemTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let force = request.arguments.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        match ctx.platform_adapter.shutdown_system(force).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "shutdown_system".to_string(),
                json!({ "status_message": "System shutdown initiated" }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "shutdown_system".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to restart the operating system (Critical risk).
pub struct RestartSystemTool {
    definition: ToolDefinition,
}

impl RestartSystemTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "restart_system".to_string(),
                name: "Restart System".to_string(),
                description: "Initiates system restart (requires user approval)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "force": { "type": "boolean" }
                    }
                }),
                risk_level: RiskLevel::Critical,
                required_permissions: vec!["system.power".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for RestartSystemTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RestartSystemTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let force = request.arguments.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        match ctx.platform_adapter.restart_system(force).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "restart_system".to_string(),
                json!({ "status_message": "System restart initiated" }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "restart_system".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to put system into sleep/suspend state (High risk).
pub struct SleepSystemTool {
    definition: ToolDefinition,
}

impl SleepSystemTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "sleep_system".to_string(),
                name: "Sleep System".to_string(),
                description: "Puts system into sleep/suspend state".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::High,
                required_permissions: vec!["system.power".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for SleepSystemTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SleepSystemTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        match ctx.platform_adapter.sleep_system().await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "sleep_system".to_string(),
                json!({ "status_message": "System sleep initiated" }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "sleep_system".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to query overall system status (platform info, disk, memory).
pub struct GetSystemInfoTool {
    definition: ToolDefinition,
}

impl GetSystemInfoTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "get_system_info".to_string(),
                name: "Get System Info".to_string(),
                description: "Queries OS platform info, disk space, and memory usage".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["system.info".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for GetSystemInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetSystemInfoTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let platform_info = ctx.platform_adapter.get_platform_info().await.ok();
        let disk_info = ctx.platform_adapter.get_disk_space().await.ok();
        let memory_info = ctx.platform_adapter.get_memory_info().await.ok();

        let data = json!({
            "platform": platform_info,
            "disk": disk_info,
            "memory": memory_info,
        });

        Ok(ToolResult::success(
            request.request_id,
            "get_system_info".to_string(),
            data,
            start.elapsed().as_millis() as u64,
        ))
    }
}

// ============================================================
// Built-in Tools: Process Management Tools
// ============================================================

/// Tool to close/terminate a desktop application or process.
pub struct CloseApplicationTool {
    definition: ToolDefinition,
}

impl CloseApplicationTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "close_application".to_string(),
                name: "Close Application".to_string(),
                description: "Terminates or closes a running application or process by name or PID".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["process.kill".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for CloseApplicationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CloseApplicationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .or_else(|| request.arguments.get("process"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "close_application".to_string(),
                details: "Missing required argument 'target' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.close_application(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "close_application".to_string(),
                json!({
                    "target": target,
                    "application": target,
                    "status_message": format!("Application '{}' closed successfully", target)
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "close_application".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool alias for explicit kill_process operation.
pub struct KillProcessTool {
    definition: ToolDefinition,
}

impl KillProcessTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "kill_process".to_string(),
                name: "Kill Process".to_string(),
                description: "Terminates a process by name or PID".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" },
                        "process": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Medium,
                required_permissions: vec!["process.kill".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for KillProcessTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for KillProcessTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("process"))
            .or_else(|| request.arguments.get("application"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "kill_process".to_string(),
                details: "Missing required argument 'target' or 'process'".to_string(),
            })?;

        match ctx.platform_adapter.close_application(target).await {
            Ok(_) => Ok(ToolResult::success(
                request.request_id,
                "kill_process".to_string(),
                json!({
                    "target": target,
                    "process": target,
                    "status_message": format!("Process '{}' terminated successfully", target)
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "kill_process".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to list running processes.
pub struct ListProcessesTool {
    definition: ToolDefinition,
}

impl ListProcessesTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "list_processes".to_string(),
                name: "List Processes".to_string(),
                description: "Enumerates active processes running on the system".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["process.read".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for ListProcessesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListProcessesTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        match ctx.platform_adapter.list_processes().await {
            Ok(processes) => Ok(ToolResult::success(
                request.request_id,
                "list_processes".to_string(),
                json!({
                    "processes": processes,
                    "count": processes.len()
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "list_processes".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to check if a specific application or process is currently running.
pub struct IsApplicationRunningTool {
    definition: ToolDefinition,
}

impl IsApplicationRunningTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "is_application_running".to_string(),
                name: "Is Application Running".to_string(),
                description: "Checks if a specific application or process is currently active".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" },
                        "application": { "type": "string" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["process.read".to_string()],
                timeout_secs: 5,
            },
        }
    }
}

impl Default for IsApplicationRunningTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for IsApplicationRunningTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let target = request
            .arguments
            .get("target")
            .or_else(|| request.arguments.get("application"))
            .or_else(|| request.arguments.get("app"))
            .or_else(|| request.arguments.get("process"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "is_application_running".to_string(),
                details: "Missing required argument 'target' or 'application'".to_string(),
            })?;

        match ctx.platform_adapter.is_application_running(target).await {
            Ok(running) => Ok(ToolResult::success(
                request.request_id,
                "is_application_running".to_string(),
                json!({
                    "target": target,
                    "application": target,
                    "running": running,
                    "status_message": if running {
                        format!("Application '{}' is currently running", target)
                    } else {
                        format!("Application '{}' is not running", target)
                    }
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "is_application_running".to_string(),
                e.to_string(),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Built-in Tools: Screenshot Tools
// ============================================================

/// Helper function to get or create the deterministic JARVIS screenshot storage directory.
pub fn get_jarvis_screenshots_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return std::path::PathBuf::from(profile)
                .join("Pictures")
                .join("JARVIS")
                .join("Screenshots");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join("Pictures")
            .join("JARVIS")
            .join("Screenshots");
    }
    std::env::temp_dir().join("JARVIS").join("Screenshots")
}

/// Helper function to save a screenshot artifact to disk with collision-safe timestamp filename.
pub async fn save_screenshot_artifact(
    screenshot: &jarvis_platform::Screenshot,
) -> anyhow::Result<(std::path::PathBuf, String)> {
    let dir = get_jarvis_screenshots_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create screenshot directory '{}': {}", dir.display(), e))?;

    let now = chrono::Local::now();
    let filename = format!("jarvis_{}.png", now.format("%Y-%m-%d_%H-%M-%S_%3f"));
    let file_path = dir.join(&filename);

    tokio::fs::write(&file_path, &screenshot.data)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write screenshot file '{}': {}", file_path.display(), e))?;

    info!(path = %file_path.display(), bytes = screenshot.data.len(), "Screenshot artifact persisted");

    Ok((file_path, filename))
}

/// Tool to capture a screenshot of the primary screen, display, or region.
pub struct TakeScreenshotTool {
    definition: ToolDefinition,
}

impl TakeScreenshotTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "take_screenshot".to_string(),
                name: "Take Screenshot".to_string(),
                description: "Captures a screenshot of the primary screen, specified display, or screen region".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "display": { "type": "integer" },
                        "display_index": { "type": "integer" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen.capture".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for TakeScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TakeScreenshotTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let display_idx = request
            .arguments
            .get("display_index")
            .or_else(|| request.arguments.get("display"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let region = if let (Some(x), Some(y), Some(w), Some(h)) = (
            request.arguments.get("x").and_then(|v| v.as_i64()),
            request.arguments.get("y").and_then(|v| v.as_i64()),
            request.arguments.get("width").and_then(|v| v.as_u64()),
            request.arguments.get("height").and_then(|v| v.as_u64()),
        ) {
            if w == 0 || h == 0 {
                return Ok(ToolResult::failure(
                    request.request_id,
                    "take_screenshot".to_string(),
                    "Invalid region dimensions: width and height must be greater than zero".to_string(),
                    start.elapsed().as_millis() as u64,
                ));
            }
            Some(Rect {
                x: x as i32,
                y: y as i32,
                width: w as u32,
                height: h as u32,
            })
        } else {
            None
        };

        let screenshot_res = if let Some(r) = region {
            ctx.platform_adapter.take_screenshot_region(r).await
        } else if let Some(idx) = display_idx {
            ctx.platform_adapter.take_screenshot_display(idx).await
        } else {
            ctx.platform_adapter.take_screenshot().await
        };

        let screenshot = match screenshot_res {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::failure(
                    request.request_id,
                    "take_screenshot".to_string(),
                    format!("Screen capture failed: {}", e),
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        match save_screenshot_artifact(&screenshot).await {
            Ok((file_path, filename)) => Ok(ToolResult::success(
                request.request_id,
                "take_screenshot".to_string(),
                json!({
                    "success": true,
                    "artifact_type": "screenshot",
                    "mime_type": "image/png",
                    "format": "png",
                    "path": file_path.to_string_lossy().to_string(),
                    "filename": filename,
                    "width": screenshot.width,
                    "height": screenshot.height,
                    "display_index": screenshot.display_index,
                    "bytes_len": screenshot.data.len(),
                    "status_message": format!("Screenshot saved to {}", file_path.to_string_lossy())
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "take_screenshot".to_string(),
                format!("Failed to save screenshot artifact: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool alias for capturing a specific display.
pub struct TakeScreenshotDisplayTool {
    definition: ToolDefinition,
}

impl TakeScreenshotDisplayTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "take_screenshot_display".to_string(),
                name: "Take Screenshot of Display".to_string(),
                description: "Captures a screenshot of a specific display by index".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "display": { "type": "integer" },
                        "display_index": { "type": "integer" }
                    },
                    "required": ["display_index"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen.capture".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for TakeScreenshotDisplayTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TakeScreenshotDisplayTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let display_idx = request
            .arguments
            .get("display_index")
            .or_else(|| request.arguments.get("display"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let screenshot = match ctx.platform_adapter.take_screenshot_display(display_idx).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::failure(
                    request.request_id,
                    "take_screenshot_display".to_string(),
                    format!("Display screen capture failed: {}", e),
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        match save_screenshot_artifact(&screenshot).await {
            Ok((file_path, filename)) => Ok(ToolResult::success(
                request.request_id,
                "take_screenshot_display".to_string(),
                json!({
                    "success": true,
                    "artifact_type": "screenshot",
                    "mime_type": "image/png",
                    "format": "png",
                    "path": file_path.to_string_lossy().to_string(),
                    "filename": filename,
                    "width": screenshot.width,
                    "height": screenshot.height,
                    "display_index": screenshot.display_index,
                    "bytes_len": screenshot.data.len(),
                    "status_message": format!("Screenshot of display {} saved to {}", display_idx, file_path.to_string_lossy())
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "take_screenshot_display".to_string(),
                format!("Failed to save display screenshot artifact: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool alias for capturing a specific screen region.
pub struct TakeScreenshotRegionTool {
    definition: ToolDefinition,
}

impl TakeScreenshotRegionTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "take_screenshot_region".to_string(),
                name: "Take Screenshot of Region".to_string(),
                description: "Captures a screenshot of a specific screen region (x, y, width, height)".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    },
                    "required": ["x", "y", "width", "height"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen.capture".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for TakeScreenshotRegionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TakeScreenshotRegionTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let x = request.arguments.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = request.arguments.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let w = request.arguments.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let h = request.arguments.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        if w == 0 || h == 0 {
            return Ok(ToolResult::failure(
                request.request_id,
                "take_screenshot_region".to_string(),
                "Invalid region dimensions: width and height must be greater than zero".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        let region = Rect { x, y, width: w, height: h };

        let screenshot = match ctx.platform_adapter.take_screenshot_region(region).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::failure(
                    request.request_id,
                    "take_screenshot_region".to_string(),
                    format!("Region screen capture failed: {}", e),
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        match save_screenshot_artifact(&screenshot).await {
            Ok((file_path, filename)) => Ok(ToolResult::success(
                request.request_id,
                "take_screenshot_region".to_string(),
                json!({
                    "success": true,
                    "artifact_type": "screenshot",
                    "mime_type": "image/png",
                    "format": "png",
                    "path": file_path.to_string_lossy().to_string(),
                    "filename": filename,
                    "width": screenshot.width,
                    "height": screenshot.height,
                    "display_index": screenshot.display_index,
                    "bytes_len": screenshot.data.len(),
                    "status_message": format!("Region screenshot saved to {}", file_path.to_string_lossy())
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "take_screenshot_region".to_string(),
                format!("Failed to save region screenshot artifact: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Built-in Tools: Clipboard Tools
// ============================================================

/// Tool to retrieve system clipboard text content.
pub struct GetClipboardTool {
    definition: ToolDefinition,
}

impl GetClipboardTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "get_clipboard".to_string(),
                name: "Get Clipboard".to_string(),
                description: "Retrieves text content currently stored in the system clipboard".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["clipboard.read".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for GetClipboardTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetClipboardTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    #[instrument(skip(self, ctx), fields(req_id = %request.request_id, tool = "get_clipboard"))]
    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        match ctx.platform_adapter.get_clipboard().await {
            Ok(content) => {
                let elapsed = start.elapsed().as_millis() as u64;
                match content {
                    jarvis_platform::ClipboardContent::Text(text) => {
                        let text_len = text.chars().count();
                        info!(tool = "get_clipboard", text_len = text_len, "Clipboard text read successfully");
                        Ok(ToolResult::success(
                            request.request_id,
                            "get_clipboard".to_string(),
                            json!({
                                "success": true,
                                "content_type": "text/plain",
                                "text": text,
                                "length": text_len
                            }),
                            elapsed,
                        ))
                    }
                    jarvis_platform::ClipboardContent::Empty => {
                        info!(tool = "get_clipboard", "Clipboard is empty");
                        Ok(ToolResult::success(
                            request.request_id,
                            "get_clipboard".to_string(),
                            json!({
                                "success": true,
                                "content_type": "text/plain",
                                "text": "",
                                "length": 0,
                                "empty": true
                            }),
                            elapsed,
                        ))
                    }
                    _ => Ok(ToolResult::success(
                        request.request_id,
                        "get_clipboard".to_string(),
                        json!({
                            "success": true,
                            "content_type": "other",
                            "text": "",
                            "length": 0
                        }),
                        elapsed,
                    )),
                }
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "get_clipboard".to_string(),
                format!("Failed to read clipboard: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

/// Tool to copy text content into the system clipboard.
pub struct SetClipboardTool {
    definition: ToolDefinition,
}

impl SetClipboardTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "set_clipboard".to_string(),
                name: "Set Clipboard".to_string(),
                description: "Copies specified text content into the system clipboard".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text content to copy into the clipboard"
                        }
                    },
                    "required": ["text"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["clipboard.write".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for SetClipboardTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SetClipboardTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    #[instrument(skip(self, ctx), fields(req_id = %request.request_id, tool = "set_clipboard"))]
    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let text = request
            .arguments
            .get("text")
            .or_else(|| request.arguments.get("content"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "set_clipboard".to_string(),
                details: "Missing required argument 'text'".to_string(),
            })?;

        let text_len = text.chars().count();
        info!(tool = "set_clipboard", text_len = text_len, "Setting clipboard text");

        match ctx
            .platform_adapter
            .set_clipboard(jarvis_platform::ClipboardContent::Text(text.to_string()))
            .await
        {
            Ok(()) => Ok(ToolResult::success(
                request.request_id,
                "set_clipboard".to_string(),
                json!({
                    "success": true,
                    "content_type": "text/plain",
                    "length": text_len,
                    "status_message": format!("Copied {} characters to clipboard", text_len)
                }),
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "set_clipboard".to_string(),
                format!("Failed to set clipboard: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Built-in Tools: Notification Tools
// ============================================================

/// Tool to display system desktop notifications.
pub struct ShowNotificationTool {
    definition: ToolDefinition,
}

impl ShowNotificationTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "show_notification".to_string(),
                name: "Show Notification".to_string(),
                description: "Displays a native desktop system notification".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Title of the notification (defaults to 'JARVIS')"
                        },
                        "message": {
                            "type": "string",
                            "description": "Message content of the notification"
                        },
                        "body": {
                            "type": "string",
                            "description": "Alternative message body key"
                        },
                        "priority": {
                            "type": "string",
                            "description": "Priority: low, normal, high, critical"
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Notification timeout in seconds"
                        }
                    },
                    "required": []
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["notification.display".to_string()],
                timeout_secs: 10,
            },
        }
    }
}

impl Default for ShowNotificationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShowNotificationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    #[instrument(skip(self, ctx), fields(req_id = %request.request_id, tool = "show_notification"))]
    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let message = request
            .arguments
            .get("message")
            .or_else(|| request.arguments.get("body"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "show_notification".to_string(),
                details: "Missing required argument 'message' or 'body'".to_string(),
            })?;

        let title = request
            .arguments
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("JARVIS");

        let priority = match request.arguments.get("priority").and_then(|v| v.as_str()) {
            Some("low") => jarvis_platform::NotificationPriority::Low,
            Some("high") => jarvis_platform::NotificationPriority::High,
            Some("critical") => jarvis_platform::NotificationPriority::Critical,
            _ => jarvis_platform::NotificationPriority::Normal,
        };

        let timeout_secs = request
            .arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let req = jarvis_platform::NotificationRequest {
            title: title.to_string(),
            body: message.to_string(),
            icon: None,
            timeout_secs,
            priority,
        };

        let msg_len = message.chars().count();
        info!(tool = "show_notification", title = %title, message_len = msg_len, "Displaying desktop notification");

        match ctx.platform_adapter.show_notification(req).await {
            Ok(()) => {
                let notif_id = uuid::Uuid::new_v4().to_string();
                Ok(ToolResult::success(
                    request.request_id,
                    "show_notification".to_string(),
                    json!({
                        "success": true,
                        "notification_id": notif_id,
                        "title": title,
                        "message_length": msg_len,
                        "status_message": format!("Notification '{}' displayed", title)
                    }),
                    start.elapsed().as_millis() as u64,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                "show_notification".to_string(),
                format!("Failed to display system notification: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Tool Registry
// ============================================================

/// Central registry holding all available tools in JARVIS.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create registry pre-populated with built-in standard tools.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(OpenApplicationTool::new()));
        registry.register(Box::new(GetTimeTool::new()));
        registry.register(Box::new(ListWindowsTool::new()));
        registry.register(Box::new(GetActiveWindowTool::new()));
        registry.register(Box::new(FocusWindowTool::new()));
        registry.register(Box::new(MinimizeWindowTool::new()));
        registry.register(Box::new(MaximizeWindowTool::new()));
        registry.register(Box::new(RestoreWindowTool::new()));
        registry.register(Box::new(SetWindowBoundsTool::new()));
        registry.register(Box::new(MoveWindowTool::new()));
        registry.register(Box::new(ResizeWindowTool::new()));
        registry.register(Box::new(SetSystemVolumeTool::new()));
        registry.register(Box::new(MuteSystemVolumeTool::new()));
        registry.register(Box::new(LockWorkstationTool::new()));
        registry.register(Box::new(ShutdownSystemTool::new()));
        registry.register(Box::new(RestartSystemTool::new()));
        registry.register(Box::new(SleepSystemTool::new()));
        registry.register(Box::new(GetSystemInfoTool::new()));
        registry.register(Box::new(CloseApplicationTool::new()));
        registry.register(Box::new(KillProcessTool::new()));
        registry.register(Box::new(ListProcessesTool::new()));
        registry.register(Box::new(IsApplicationRunningTool::new()));
        registry.register(Box::new(TakeScreenshotTool::new()));
        registry.register(Box::new(TakeScreenshotDisplayTool::new()));
        registry.register(Box::new(TakeScreenshotRegionTool::new()));
        registry.register(Box::new(GetClipboardTool::new()));
        registry.register(Box::new(SetClipboardTool::new()));
        registry.register(Box::new(ShowNotificationTool::new()));
        registry
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.definition().id.to_lowercase();
        self.tools.insert(name, tool);
    }

    /// Retrieve a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(&name.to_lowercase()).map(|b| b.as_ref())
    }

    /// List all registered tool definitions.
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| t.definition().clone())
            .collect()
    }

    /// Execute a tool request with context.
    #[instrument(skip(self, ctx), fields(tool = %request.tool_name))]
    pub async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let tool = self
            .get(&request.tool_name)
            .ok_or_else(|| ToolError::NotFound(request.tool_name.clone()))?;

        tool.execute(request, ctx).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use jarvis_platform::*;

    // Mock platform adapter for testing
    struct MockPlatformAdapter {
        pub fail_app: bool,
        pub clipboard: std::sync::Mutex<Option<String>>,
    }

    impl MockPlatformAdapter {
        pub fn new(fail_app: bool) -> Self {
            Self {
                fail_app,
                clipboard: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl PlatformAdapter for MockPlatformAdapter {
        async fn get_platform_info(&self) -> anyhow::Result<PlatformInfo> {
            Ok(PlatformInfo {
                os: OperatingSystem::Windows,
                os_version: "Windows 11".to_string(),
                arch: Architecture::X86_64,
                hostname: "test-host".to_string(),
                username: "test-user".to_string(),
                home_dir: std::path::PathBuf::from("C:\\Users\\test"),
                temp_dir: std::path::PathBuf::from("C:\\Temp"),
            })
        }

        async fn open_application(
            &self,
            app: &str,
            _options: Option<LaunchOptions>,
        ) -> anyhow::Result<ProcessInfo> {
            if self.fail_app {
                anyhow::bail!("Application '{}' not found", app);
            }
            Ok(ProcessInfo {
                pid: 4321,
                name: app.to_string(),
                executable_path: None,
                command_line: Some(app.to_string()),
                running: true,
            })
        }

        async fn close_application(&self, _app: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_processes(&self) -> anyhow::Result<Vec<ProcessInfo>> {
            Ok(vec![])
        }
        async fn is_application_running(&self, _app: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>> {
            Ok(vec![])
        }
        async fn focus_window(&self, _handle: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn minimize_window(&self, _handle: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn maximize_window(&self, _handle: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn set_window_bounds(&self, _handle: &str, _bounds: Rect) -> anyhow::Result<()> {
            Ok(())
        }
        async fn take_screenshot(&self) -> anyhow::Result<Screenshot> {
            Ok(Screenshot {
                data: vec![],
                format: ImageFormat::Png,
                width: 100,
                height: 100,
                display_index: 0,
            })
        }
        async fn take_screenshot_display(&self, _idx: u32) -> anyhow::Result<Screenshot> {
            self.take_screenshot().await
        }
        async fn take_screenshot_region(&self, _r: Rect) -> anyhow::Result<Screenshot> {
            self.take_screenshot().await
        }
        async fn get_clipboard(&self) -> anyhow::Result<ClipboardContent> {
            let guard = self.clipboard.lock().unwrap();
            match &*guard {
                Some(text) if !text.is_empty() => Ok(ClipboardContent::Text(text.clone())),
                _ => Ok(ClipboardContent::Empty),
            }
        }
        async fn set_clipboard(&self, c: ClipboardContent) -> anyhow::Result<()> {
            let mut guard = self.clipboard.lock().unwrap();
            match c {
                ClipboardContent::Text(t) => *guard = Some(t),
                ClipboardContent::Empty => *guard = None,
                _ => {}
            }
            Ok(())
        }
        async fn show_notification(&self, _n: NotificationRequest) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_disk_space(&self) -> anyhow::Result<DiskInfo> {
            Ok(DiskInfo {
                total_bytes: 1000,
                available_bytes: 500,
                used_bytes: 500,
            })
        }
        async fn get_memory_info(&self) -> anyhow::Result<MemoryInfo> {
            Ok(MemoryInfo {
                total_bytes: 1000,
                available_bytes: 500,
                used_bytes: 500,
            })
        }
        async fn set_system_volume(&self, _level: u32) -> anyhow::Result<()> {
            Ok(())
        }
        async fn set_system_mute(&self, _mute: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn lock_workstation(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn shutdown_system(&self, _force: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn restart_system(&self, _force: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn sleep_system(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_open_application_tool_success() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("open_application", json!({ "application": "chrome" }));
        let result = registry.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["pid"], 4321);
        assert_eq!(result.data["application"], "chrome");
    }

    #[tokio::test]
    async fn test_open_application_tool_failure() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(true));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("open_application", json!({ "application": "nonexistent" }));
        let result = registry.execute(req, &ctx).await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_get_time_tool() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("get_time", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.data["time"].is_string());
    }

    #[tokio::test]
    async fn test_system_control_tools_registration_and_execution() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        // set_system_volume
        let req = ToolRequest::new("set_system_volume", json!({ "level": 75 }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // mute_system_volume
        let req = ToolRequest::new("mute_system_volume", json!({ "mute": true }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // lock_workstation
        let req = ToolRequest::new("lock_workstation", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // get_system_info
        let req = ToolRequest::new("get_system_info", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.data["disk"]["total_bytes"].as_u64().is_some());
    }

    #[test]
    fn test_system_power_tools_risk_levels() {
        let registry = ToolRegistry::with_builtins();

        let shutdown = registry.get("shutdown_system").unwrap();
        assert_eq!(shutdown.definition().risk_level, RiskLevel::Critical);

        let restart = registry.get("restart_system").unwrap();
        assert_eq!(restart.definition().risk_level, RiskLevel::Critical);

        let sleep = registry.get("sleep_system").unwrap();
        assert_eq!(sleep.definition().risk_level, RiskLevel::High);

        let volume = registry.get("set_system_volume").unwrap();
        assert_eq!(volume.definition().risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_process_management_tools_registration_and_execution() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        // Verify registration
        assert!(registry.get("close_application").is_some());
        assert!(registry.get("kill_process").is_some());
        assert!(registry.get("list_processes").is_some());
        assert!(registry.get("is_application_running").is_some());

        // close_application execution
        let req = ToolRequest::new("close_application", json!({ "target": "chrome" }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["target"], "chrome");

        // kill_process execution
        let req = ToolRequest::new("kill_process", json!({ "target": "notepad" }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // list_processes execution
        let req = ToolRequest::new("list_processes", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // is_application_running execution
        let req = ToolRequest::new("is_application_running", json!({ "target": "spotify" }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_process_tools_risk_levels() {
        let registry = ToolRegistry::with_builtins();

        let close_app = registry.get("close_application").unwrap();
        assert_eq!(close_app.definition().risk_level, RiskLevel::Medium);

        let kill_proc = registry.get("kill_process").unwrap();
        assert_eq!(kill_proc.definition().risk_level, RiskLevel::Medium);

        let list_proc = registry.get("list_processes").unwrap();
        assert_eq!(list_proc.definition().risk_level, RiskLevel::Low);

        let is_running = registry.get("is_application_running").unwrap();
        assert_eq!(is_running.definition().risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_screenshot_tools_registration_and_execution() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        // Verify registration
        assert!(registry.get("take_screenshot").is_some());
        assert!(registry.get("take_screenshot_display").is_some());
        assert!(registry.get("take_screenshot_region").is_some());

        // take_screenshot primary
        let req = ToolRequest::new("take_screenshot", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["format"], "png");
        assert_eq!(result.data["width"], 100);

        // take_screenshot_display
        let req = ToolRequest::new("take_screenshot_display", json!({ "display_index": 1 }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        // take_screenshot_region
        let req = ToolRequest::new("take_screenshot_region", json!({ "x": 0, "y": 0, "width": 500, "height": 400 }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_clipboard_tools_registration_and_execution() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        // Verify registration
        assert!(registry.get("get_clipboard").is_some());
        assert!(registry.get("set_clipboard").is_some());

        // 1. Initial empty clipboard
        let req = ToolRequest::new("get_clipboard", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["empty"], true);

        // 2. Set simple string "hello world"
        let req = ToolRequest::new("set_clipboard", json!({ "text": "hello world" }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["length"], 11);

        // 3. Get simple string "hello world"
        let req = ToolRequest::new("get_clipboard", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["text"], "hello world");

        // 4. Multiline text
        let multiline = "line one\nline two\nline three";
        let req = ToolRequest::new("set_clipboard", json!({ "text": multiline }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        let req = ToolRequest::new("get_clipboard", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["text"], multiline);

        // 5. Unicode text
        let unicode = "JARVIS — नमस्ते — 日本語";
        let req = ToolRequest::new("set_clipboard", json!({ "text": unicode }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);

        let req = ToolRequest::new("get_clipboard", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["text"], unicode);
    }

    #[tokio::test]
    async fn test_show_notification_tool_registration_and_execution() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        // Verify registration
        assert!(registry.get("show_notification").is_some());

        let req = ToolRequest::new("show_notification", json!({
            "title": "JARVIS",
            "message": "Task completed successfully"
        }));
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["title"], "JARVIS");
        assert_eq!(result.data["message_length"], 27);
        assert!(result.data["notification_id"].is_string());
    }
}
