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

use jarvis_ai::{
    build_detection_prompt, parse_elements_from_vision_response, OcrProvider, OcrRequest,
    OllamaVisionProvider, TesseractOcrProvider, VisionImage, VisionModelProvider, VisionRequest,
};
use jarvis_browser::TabTarget;
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
                description: "Enumerates all visible open application windows on the desktop"
                    .to_string(),
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
                description: "Retrieves details of the currently focused foreground desktop window"
                    .to_string(),
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
                description: "Brings a window to the foreground by handle or application name"
                    .to_string(),
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
                description: "Restores a minimized or maximized window to normal windowed state"
                    .to_string(),
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
                description:
                    "Moves and resizes a window to specified coordinates (x, y, width, height)"
                        .to_string(),
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

        let x = request
            .arguments
            .get("x")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let y = request
            .arguments
            .get("y")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let width = request
            .arguments
            .get("width")
            .and_then(|v| v.as_u64())
            .unwrap_or(800) as u32;
        let height = request
            .arguments
            .get("height")
            .and_then(|v| v.as_u64())
            .unwrap_or(600) as u32;

        let bounds = jarvis_platform::Rect {
            x,
            y,
            width,
            height,
        };

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

        let x = request
            .arguments
            .get("x")
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as i32;
        let y = request
            .arguments
            .get("y")
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as i32;

        let bounds = jarvis_platform::Rect {
            x,
            y,
            width: 800,
            height: 600,
        };

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

        let width = request
            .arguments
            .get("width")
            .and_then(|v| v.as_u64())
            .unwrap_or(1280) as u32;
        let height = request
            .arguments
            .get("height")
            .and_then(|v| v.as_u64())
            .unwrap_or(720) as u32;

        let bounds = jarvis_platform::Rect {
            x: 100,
            y: 100,
            width,
            height,
        };

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
                description: "Sets system master volume to specified percentage (0 to 100)"
                    .to_string(),
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
        let force = request
            .arguments
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

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
        let force = request
            .arguments
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

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
                description: "Terminates or closes a running application or process by name or PID"
                    .to_string(),
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
                description: "Checks if a specific application or process is currently active"
                    .to_string(),
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
    tokio::fs::create_dir_all(&dir).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to create screenshot directory '{}': {}",
            dir.display(),
            e
        )
    })?;

    let now = chrono::Local::now();
    let filename = format!("jarvis_{}.png", now.format("%Y-%m-%d_%H-%M-%S_%3f"));
    let file_path = dir.join(&filename);

    tokio::fs::write(&file_path, &screenshot.data)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to write screenshot file '{}': {}",
                file_path.display(),
                e
            )
        })?;

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
                    "Invalid region dimensions: width and height must be greater than zero"
                        .to_string(),
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

        let screenshot = match ctx
            .platform_adapter
            .take_screenshot_display(display_idx)
            .await
        {
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
                description:
                    "Captures a screenshot of a specific screen region (x, y, width, height)"
                        .to_string(),
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
        let x = request
            .arguments
            .get("x")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let y = request
            .arguments
            .get("y")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let w = request
            .arguments
            .get("width")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let h = request
            .arguments
            .get("height")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        if w == 0 || h == 0 {
            return Ok(ToolResult::failure(
                request.request_id,
                "take_screenshot_region".to_string(),
                "Invalid region dimensions: width and height must be greater than zero".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        let region = Rect {
            x,
            y,
            width: w,
            height: h,
        };

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
                description: "Retrieves text content currently stored in the system clipboard"
                    .to_string(),
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
                        info!(
                            tool = "get_clipboard",
                            text_len = text_len,
                            "Clipboard text read successfully"
                        );
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
        info!(
            tool = "set_clipboard",
            text_len = text_len,
            "Setting clipboard text"
        );

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
// Vision Tools: DescribeScreenTool
// ============================================================

/// Tool to capture the current desktop screen and generate a visual description using a local vision model.
pub struct DescribeScreenTool {
    definition: ToolDefinition,
    vision_provider: Arc<dyn VisionModelProvider>,
}

impl DescribeScreenTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "describe_screen".to_string(),
                name: "Describe Screen".to_string(),
                description: "Captures the current desktop screenshot and returns a visual description using the local vision model.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Optional custom prompt or question describing what visual information to analyze"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ai_vision".to_string()],
                timeout_secs: 60,
            },
            vision_provider: Arc::new(OllamaVisionProvider::default()),
        }
    }

    pub fn with_provider(vision_provider: Arc<dyn VisionModelProvider>) -> Self {
        Self {
            definition: ToolDefinition {
                id: "describe_screen".to_string(),
                name: "Describe Screen".to_string(),
                description: "Captures the current desktop screenshot and returns a visual description using the local vision model.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Optional custom prompt or question describing what visual information to analyze"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ai_vision".to_string()],
                timeout_secs: 60,
            },
            vision_provider,
        }
    }
}

impl Default for DescribeScreenTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DescribeScreenTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        // 1. Capture screen via platform adapter
        let screenshot = ctx.platform_adapter.take_screenshot().await.map_err(|e| {
            ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to capture desktop screenshot: {}", e),
            }
        })?;

        if screenshot.data.is_empty() {
            return Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                "Captured screenshot is empty (0 bytes)".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 2. Extract prompt (default to general visual query if not provided)
        let prompt_str = request
            .arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Describe what is visible on the screen.");

        // 3. Build VisionImage and validate
        let image = VisionImage::from_png_bytes(screenshot.data)
            .with_dimensions(screenshot.width, screenshot.height);

        if let Err(val_err) = image.validate(10 * 1024 * 1024, Some((4096, 4096))) {
            return Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                format!("Vision image validation failed: {}", val_err),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 4. Send request through vision_provider
        let vision_req = VisionRequest::new(image, prompt_str);
        match self.vision_provider.analyze_image(&vision_req).await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_millis() as u64;
                Ok(ToolResult::success(
                    request.request_id,
                    self.definition.id.clone(),
                    json!({
                        "description": resp.description,
                        "model_id": resp.model_id,
                        "provider": format!("{:?}", resp.provider_type),
                        "latency_ms": resp.latency_ms
                    }),
                    elapsed,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                format!("Vision model inference failed: {}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// OCR Tools: ReadScreenTextTool
// ============================================================

/// Tool to capture the current desktop screen and extract visible text using local Tesseract OCR.
pub struct ReadScreenTextTool {
    definition: ToolDefinition,
    ocr_provider: Arc<dyn OcrProvider>,
}

impl ReadScreenTextTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "read_screen_text".to_string(),
                name: "Read Screen Text".to_string(),
                description: "Captures the current desktop screenshot and extracts visible text using local Tesseract OCR.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "language": {
                            "type": "string",
                            "description": "Optional OCR language code (default: 'eng')"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ocr".to_string()],
                timeout_secs: 60,
            },
            ocr_provider: Arc::new(TesseractOcrProvider::new()),
        }
    }

    pub fn with_provider(ocr_provider: Arc<dyn OcrProvider>) -> Self {
        Self {
            definition: ToolDefinition {
                id: "read_screen_text".to_string(),
                name: "Read Screen Text".to_string(),
                description: "Captures the current desktop screenshot and extracts visible text using local Tesseract OCR.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "language": {
                            "type": "string",
                            "description": "Optional OCR language code (default: 'eng')"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ocr".to_string()],
                timeout_secs: 60,
            },
            ocr_provider,
        }
    }
}

impl Default for ReadScreenTextTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadScreenTextTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        // 1. Capture screen via platform adapter
        let screenshot = ctx.platform_adapter.take_screenshot().await.map_err(|e| {
            ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to capture desktop screenshot for OCR: {}", e),
            }
        })?;

        if screenshot.data.is_empty() {
            return Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                "Captured screenshot is empty (0 bytes)".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 2. Extract optional language hint
        let lang = request
            .arguments
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 3. Construct OcrRequest
        let mut ocr_req = OcrRequest::new(screenshot.data);
        if let Some(l) = lang {
            ocr_req = ocr_req.with_language(l);
        }

        // 4. Perform text extraction via OCR provider
        match self.ocr_provider.extract_text(&ocr_req).await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_millis() as u64;
                Ok(ToolResult::success(
                    request.request_id,
                    self.definition.id.clone(),
                    json!({
                        "text": resp.text,
                        "has_text": resp.has_text,
                        "char_count": resp.char_count,
                        "confidence": resp.confidence,
                        "provider": format!("{:?}", resp.provider_type),
                        "latency_ms": resp.latency_ms
                    }),
                    elapsed,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                format!("{}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// OCR Tools: ReadScreenTool (canonical "read_screen" tool name per M08.03 spec)
// ============================================================

/// Tool to capture the current desktop screen and extract all visible text using local Tesseract OCR.
/// This is the canonical `read_screen` tool registered under the M08.03 milestone spec.
/// Functionally equivalent to `ReadScreenTextTool`, registered under the id `"read_screen"`.
pub struct ReadScreenTool {
    definition: ToolDefinition,
    ocr_provider: Arc<dyn OcrProvider>,
}

impl ReadScreenTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "read_screen".to_string(),
                name: "Read Screen".to_string(),
                description: "Captures the current desktop screenshot and extracts all visible text using local Tesseract OCR. Use for commands like 'read my screen', 'what does my screen say', 'read the text on my screen'.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "language": {
                            "type": "string",
                            "description": "Optional OCR language code (default: 'eng')"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ocr".to_string()],
                timeout_secs: 60,
            },
            ocr_provider: Arc::new(TesseractOcrProvider::new()),
        }
    }

    /// Inject an alternative OCR provider (e.g. MockOcrProvider for unit tests).
    pub fn with_provider(ocr_provider: Arc<dyn OcrProvider>) -> Self {
        Self {
            definition: ToolDefinition {
                id: "read_screen".to_string(),
                name: "Read Screen".to_string(),
                description: "Captures the current desktop screenshot and extracts all visible text using local Tesseract OCR. Use for commands like 'read my screen', 'what does my screen say', 'read the text on my screen'.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "language": {
                            "type": "string",
                            "description": "Optional OCR language code (default: 'eng')"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "ocr".to_string()],
                timeout_secs: 60,
            },
            ocr_provider,
        }
    }
}

impl Default for ReadScreenTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadScreenTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        // 1. Capture screen via platform adapter (reuse existing screenshot infrastructure)
        let screenshot = ctx.platform_adapter.take_screenshot().await.map_err(|e| {
            ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to capture desktop screenshot for OCR: {}", e),
            }
        })?;

        if screenshot.data.is_empty() {
            return Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                "Captured screenshot is empty (0 bytes)".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 2. Extract optional language hint
        let lang = request
            .arguments
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 3. Construct OcrRequest (PNG bytes from PlatformAdapter::take_screenshot)
        let mut ocr_req = OcrRequest::new(screenshot.data);
        if let Some(l) = lang {
            ocr_req = ocr_req.with_language(l);
        }

        // 4. Perform text extraction via OCR provider (TesseractOcrProvider in production)
        match self.ocr_provider.extract_text(&ocr_req).await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_millis() as u64;
                info!(
                    tool = "read_screen",
                    has_text = resp.has_text,
                    char_count = resp.char_count,
                    latency_ms = resp.latency_ms,
                    "OCR text extraction completed"
                );
                Ok(ToolResult::success(
                    request.request_id,
                    self.definition.id.clone(),
                    json!({
                        "text": resp.text,
                        "has_text": resp.has_text,
                        "char_count": resp.char_count,
                        "confidence": resp.confidence,
                        "provider": format!("{:?}", resp.provider_type),
                        "latency_ms": resp.latency_ms
                    }),
                    elapsed,
                ))
            }
            Err(e) => Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                format!("{}", e),
                start.elapsed().as_millis() as u64,
            )),
        }
    }
}

// ============================================================
// Screen Element Detection Tool (M08.04)
// ============================================================

/// Tool that captures the desktop and uses the existing VisionModelProvider
/// to detect visible UI elements with bounding boxes.
///
/// IMPORTANT: This tool detects and describes elements only.
/// It does NOT move the mouse, click, type, or perform any UI interaction.
/// Autonomous interaction belongs to later roadmap phases.
pub struct DetectScreenElementsTool {
    definition: ToolDefinition,
    vision_provider: Arc<dyn VisionModelProvider>,
}

impl DetectScreenElementsTool {
    /// Create with the default local Ollama vision provider.
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "detect_screen_elements".to_string(),
                name: "Detect Screen Elements".to_string(),
                description: "Captures the desktop screenshot and uses the local vision model to detect visible UI elements (buttons, icons, inputs, etc.) with approximate bounding boxes and coordinates. Use for queries like 'find the Chrome icon', 'what buttons are visible', 'where is the search box'. Does NOT perform any interaction.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Optional natural-language element query (e.g. 'find the Chrome icon', 'what buttons are visible')"
                        },
                        "min_confidence": {
                            "type": "number",
                            "description": "Minimum confidence threshold [0.0, 1.0] for returned elements (default: 0.5)"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "vision".to_string()],
                timeout_secs: 120,
            },
            vision_provider: Arc::new(OllamaVisionProvider::default()),
        }
    }

    /// Create with an injected VisionModelProvider (for unit tests via MockVisionProvider).
    pub fn with_vision_provider(vision_provider: Arc<dyn VisionModelProvider>) -> Self {
        Self {
            definition: ToolDefinition {
                id: "detect_screen_elements".to_string(),
                name: "Detect Screen Elements".to_string(),
                description: "Captures the desktop screenshot and uses the local vision model to detect visible UI elements (buttons, icons, inputs, etc.) with approximate bounding boxes and coordinates. Use for queries like 'find the Chrome icon', 'what buttons are visible', 'where is the search box'. Does NOT perform any interaction.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Optional natural-language element query (e.g. 'find the Chrome icon', 'what buttons are visible')"
                        },
                        "min_confidence": {
                            "type": "number",
                            "description": "Minimum confidence threshold [0.0, 1.0] for returned elements (default: 0.5)"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["screen_capture".to_string(), "vision".to_string()],
                timeout_secs: 120,
            },
            vision_provider,
        }
    }
}

impl Default for DetectScreenElementsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DetectScreenElementsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        // 1. Capture desktop screenshot via existing platform infrastructure
        let screenshot = ctx.platform_adapter.take_screenshot().await.map_err(|e| {
            ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!(
                    "Failed to capture desktop screenshot for element detection: {}",
                    e
                ),
            }
        })?;

        if screenshot.data.is_empty() {
            return Ok(ToolResult::failure(
                request.request_id,
                self.definition.id.clone(),
                "Captured screenshot is empty (0 bytes)".to_string(),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 2. Extract optional query and confidence threshold
        let query = request
            .arguments
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let min_confidence = request
            .arguments
            .get("min_confidence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5_f32)
            .clamp(0.0, 1.0);

        // 3. Build the structured detection prompt (reuse build_detection_prompt)
        let prompt = build_detection_prompt(query.as_deref());

        // 4. Construct VisionImage and VisionRequest using existing types
        let vision_image = VisionImage::from_png_bytes(screenshot.data)
            .with_dimensions(screenshot.width, screenshot.height);
        let vision_request = VisionRequest::new(vision_image, &prompt);

        // 5. Invoke vision model via VisionModelProvider (OllamaVisionProvider / MockVisionProvider)
        let vision_response = match self.vision_provider.analyze_image(&vision_request).await {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(ToolResult::failure(
                    request.request_id,
                    self.definition.id.clone(),
                    format!("Vision model analysis failed: {}", e),
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        let elapsed = start.elapsed().as_millis() as u64;

        // 6. Parse structured elements from model response
        let detection = parse_elements_from_vision_response(
            &vision_response.description,
            query.clone(),
            elapsed,
        );

        // 7. Apply confidence filter
        let filtered_elements: Vec<serde_json::Value> = detection
            .elements
            .iter()
            .filter(|e| e.confidence >= min_confidence)
            .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
            .collect();

        let element_count = filtered_elements.len();

        info!(
            tool = "detect_screen_elements",
            element_count,
            is_limited = detection.is_limited(),
            latency_ms = elapsed,
            "Screen element detection completed"
        );

        // 8. Build result data
        let mut data = serde_json::json!({
            "elements": filtered_elements,
            "element_count": element_count,
            "query": query,
            "latency_ms": elapsed
        });

        if let Some(limitation) = &detection.detection_limitation {
            data["detection_limitation"] = serde_json::Value::String(limitation.clone());
        }

        if let Some(raw) = &detection.raw_description {
            data["raw_description"] = serde_json::Value::String(raw.clone());
        }

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

// ============================================================
// UI Automation Inspection Tool (M08.04)
// ============================================================

/// Tool that inspects the active foreground application window using native
/// Windows UI Automation (accessibility tree) to locate UI elements and exact bounding boxes.
pub struct InspectUiTreeTool {
    definition: ToolDefinition,
}

impl InspectUiTreeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "inspect_ui_tree".to_string(),
                name: "Inspect UI Tree".to_string(),
                description: "Inspects the active foreground application window using Windows UI Automation to discover accessible UI elements (buttons, text inputs, links, menus, tabs, etc.) with exact OS bounding rectangles and metadata. Use for queries like 'inspect the UI', 'find the Soft Reset button', 'where is the search box'. Does NOT perform any mouse or click interaction.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Optional search filter matching element name, control type, or automation ID (e.g. 'Soft Reset', 'button', 'text input')"
                        },
                        "max_depth": {
                            "type": "integer",
                            "description": "Maximum tree traversal depth (default: 8)"
                        },
                        "max_elements": {
                            "type": "integer",
                            "description": "Maximum elements to return (default: 100)"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["accessibility".to_string(), "ui_automation".to_string()],
                timeout_secs: 30,
            },
        }
    }
}

impl Default for InspectUiTreeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for InspectUiTreeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let query = request
            .arguments
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let max_depth = request
            .arguments
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(8);

        let max_elements = request
            .arguments
            .get("max_elements")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);

        let uia_result = ctx
            .platform_adapter
            .inspect_ui_tree(query.as_deref(), max_depth, max_elements)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Windows UI Automation inspection failed: {}", e),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        let element_count = uia_result.elements.len();

        info!(
            tool = "inspect_ui_tree",
            window_title = %uia_result.window_title,
            element_count,
            total_scanned = uia_result.total_elements_scanned,
            latency_ms = elapsed,
            "UI Automation inspection completed"
        );

        let elements_json: Vec<serde_json::Value> = uia_result
            .elements
            .iter()
            .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
            .collect();

        let data = json!({
            "window": {
                "title": uia_result.window_title,
                "process_name": uia_result.process_name
            },
            "elements": elements_json,
            "element_count": element_count,
            "total_elements_scanned": uia_result.total_elements_scanned,
            "is_truncated": uia_result.is_truncated,
            "query": query,
            "source": uia_result.source,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

// ============================================================
// Browser Session Management Tools (M09.01)
// ============================================================

use jarvis_browser::{
    BrowserNavigationRequest, BrowserProvider, BrowserType, PlatformBrowserProvider,
};

/// Tool to inspect browser detection, window, and session state.
pub struct GetBrowserStatusTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl GetBrowserStatusTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_status".to_string(),
                name: "Browser Status".to_string(),
                description: "Detects whether a web browser (e.g. Chrome, Edge, Firefox, Brave) is running, inspects process/window count, foreground status, active page title, and session metadata.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": {
                            "type": "string",
                            "description": "Browser to check (default: 'Chrome')"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["system_info".to_string(), "browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for GetBrowserStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetBrowserStatusTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");

        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let status = provider
            .detect_browser(browser_type.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to detect browser status: {}", e),
            })?;

        let session = provider
            .get_session_state(browser_type)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to get browser session state: {}", e),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;

        let data = json!({
            "browser": status.browser_name,
            "running": status.running,
            "process_name": status.process_name,
            "process_id": status.process_id,
            "window_count": status.window_count,
            "foreground": status.foreground,
            "window_title": status.active_window_title,
            "current_url": session.current_url,
            "current_page_title": session.current_page_title,
            "limitations": session.limitations,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to launch a web browser session or reuse an existing session.
pub struct OpenBrowserTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl OpenBrowserTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "open_browser".to_string(),
                name: "Open Browser".to_string(),
                description: "Launches a web browser (e.g. Chrome) or reuses an existing running browser session without launching duplicate processes, optionally navigating to an initial URL.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": {
                            "type": "string",
                            "description": "Browser name to open (default: 'Chrome')"
                        },
                        "url": {
                            "type": "string",
                            "description": "Optional initial URL to navigate to upon opening"
                        }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["process_management".to_string(), "browser".to_string()],
                timeout_secs: 30,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for OpenBrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OpenBrowserTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");

        let url_opt = request
            .arguments
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let status = provider
            .launch_browser(browser_type.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: format!("Failed to open browser: {}", e),
            })?;

        let mut nav_result = None;
        if let Some(target_url) = url_opt {
            let nav_req = BrowserNavigationRequest {
                url: target_url.to_string(),
                browser: browser_type.clone(),
                new_tab: false,
            };
            let res = provider
                .navigate(nav_req)
                .await
                .map_err(|e| ToolError::ExecutionFailed {
                    tool: self.definition.id.clone(),
                    cause: format!("Browser launched but initial navigation failed: {}", e),
                })?;
            nav_result = Some(res);
        }

        let elapsed = start.elapsed().as_millis() as u64;

        let mut data = json!({
            "browser": status.browser_name,
            "running": status.running,
            "process_name": status.process_name,
            "process_id": status.process_id,
            "window_count": status.window_count,
            "foreground": status.foreground,
            "window_title": status.active_window_title,
            "latency_ms": elapsed
        });

        if let Some(nav) = nav_result {
            data["navigation"] = json!({
                "success": nav.success,
                "url": nav.url,
                "message": nav.message
            });
        }

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to navigate an active browser session to a validated URL.
pub struct NavigateBrowserTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl NavigateBrowserTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_navigate".to_string(),
                name: "Navigate Browser".to_string(),
                description: "Navigates an active browser session to a specified URL (e.g. 'https://www.google.com', 'https://www.linkedin.com').".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to navigate to (e.g. 'https://www.google.com', 'linkedin.com')"
                        },
                        "browser": {
                            "type": "string",
                            "description": "Browser to navigate (default: 'Chrome')"
                        },
                        "new_tab": {
                            "type": "boolean",
                            "description": "Whether to open in a new tab"
                        }
                    },
                    "required": ["url"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["process_management".to_string(), "browser".to_string()],
                timeout_secs: 30,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for NavigateBrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for NavigateBrowserTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();

        let raw_url = request
            .arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required argument 'url'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");

        let new_tab = request
            .arguments
            .get("new_tab")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let nav_req = BrowserNavigationRequest {
            url: raw_url.to_string(),
            browser: browser_type,
            new_tab,
        };

        let nav_result =
            provider
                .navigate(nav_req)
                .await
                .map_err(|e| ToolError::ExecutionFailed {
                    tool: self.definition.id.clone(),
                    cause: format!("Browser navigation failed: {}", e),
                })?;

        let elapsed = start.elapsed().as_millis() as u64;

        let data = json!({
            "success": nav_result.success,
            "url": nav_result.url,
            "browser": nav_result.browser,
            "message": nav_result.message,
            "window_title": nav_result.window_title,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to navigate backward in browser history.
pub struct BrowserBackTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserBackTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_back".to_string(),
                name: "Browser Back".to_string(),
                description: "Navigates backward in the active browser session history."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserBackTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserBackTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .back(browser_type)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool to navigate forward in browser history.
pub struct BrowserForwardTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserForwardTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_forward".to_string(),
                name: "Browser Forward".to_string(),
                description: "Navigates forward in the active browser session history.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserForwardTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserForwardTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .forward(browser_type)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool to reload the current page.
pub struct BrowserReloadTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserReloadTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_reload".to_string(),
                name: "Browser Reload".to_string(),
                description: "Reloads the active browser page.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserReloadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserReloadTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .reload(browser_type)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool to inspect current page URL and title.
pub struct BrowserCurrentPageTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserCurrentPageTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_current_page".to_string(),
                name: "Browser Current Page".to_string(),
                description: "Gets the current URL and page title of the active browser session."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserCurrentPageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserCurrentPageTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let session = provider
            .get_session_state(browser_type.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data = json!({
            "browser": session.browser,
            "running": session.running,
            "current_url": session.current_url,
            "current_page_title": session.current_page_title,
            "active_window": session.active_window,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to list open browser tabs.
pub struct BrowserListTabsTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserListTabsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_list_tabs".to_string(),
                name: "Browser List Tabs".to_string(),
                description: "Lists all open tabs in the active browser session.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserListTabsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserListTabsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let tabs = provider
            .list_tabs(browser_type.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data = json!({
            "browser": browser_type.name(),
            "tab_count": tabs.len(),
            "tabs": tabs,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to open a new browser tab.
pub struct BrowserNewTabTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserNewTabTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_new_tab".to_string(),
                name: "Browser New Tab".to_string(),
                description: "Opens a new browser tab, optionally navigating to an initial URL."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Optional initial URL" },
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 20,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserNewTabTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserNewTabTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let url_opt = request
            .arguments
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let browser_type = BrowserType::from_str(browser_str);
        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let tab = provider
            .new_tab(browser_type.clone(), url_opt)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data = json!({
            "browser": browser_type.name(),
            "tab": tab,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to switch to a specific browser tab by index or title.
pub struct BrowserSwitchTabTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserSwitchTabTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_switch_tab".to_string(),
                name: "Browser Switch Tab".to_string(),
                description: "Switches to an open browser tab by 1-based index or title substring."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "tab_index": { "type": "integer", "description": "1-based tab index (e.g. 1, 2, 3)" },
                        "title": { "type": "string", "description": "Tab title search query" },
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserSwitchTabTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserSwitchTabTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let target = if let Some(idx) = request.arguments.get("tab_index").and_then(|v| v.as_u64())
        {
            TabTarget::Index(idx as usize)
        } else if let Some(title) = request.arguments.get("title").and_then(|v| v.as_str()) {
            TabTarget::Title(title.to_string())
        } else {
            return Err(ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Must specify either 'tab_index' or 'title'".to_string(),
            });
        };

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let tab = provider
            .switch_tab(browser_type.clone(), target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data = json!({
            "browser": browser_type.name(),
            "tab": tab,
            "latency_ms": elapsed
        });

        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            data,
            elapsed,
        ))
    }
}

/// Tool to close the current or specified browser tab.
pub struct BrowserCloseTabTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserCloseTabTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_close_tab".to_string(),
                name: "Browser Close Tab".to_string(),
                description: "Closes the current or specified browser tab.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "tab_index": { "type": "integer", "description": "Optional 1-based tab index" },
                        "browser": { "type": "string", "description": "Browser name (default: 'Chrome')" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(provider);
        tool
    }
}

impl Default for BrowserCloseTabTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserCloseTabTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let target = request
            .arguments
            .get("tab_index")
            .and_then(|v| v.as_u64())
            .map(|idx| TabTarget::Index(idx as usize));

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .close_tab(browser_type, target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

// ============================================================
// M09.03 Browser DOM Element Finding & Interaction Tools
// ============================================================

/// Tool for finding DOM elements inside active browser page.
pub struct BrowserFindElementTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserFindElementTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_find_element".to_string(),
                name: "Find Browser DOM Element".to_string(),
                description: "Finds DOM elements in the active browser page by query, visible text, tag, or CSS selector.".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Element text, tag, or query string" },
                        "browser": { "type": "string", "description": "Browser name (Chrome, Edge, Firefox, etc.)" }
                    },
                    "required": ["query"]
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(browser_provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(browser_provider);
        tool
    }
}

impl Default for BrowserFindElementTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserFindElementTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(
        &self,
        request: ToolRequest,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let query = request
            .arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required string parameter 'query'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .find_element(browser_type, query)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool for clicking a matched DOM element inside active browser page.
pub struct BrowserClickElementTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserClickElementTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_click_element".to_string(),
                name: "Click Browser DOM Element".to_string(),
                description: "Clicks a matched DOM element safely inside active browser page."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Element query, text, or target" },
                        "target": { "type": "string", "description": "Element query, text, or target" },
                        "browser": { "type": "string", "description": "Browser name" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(browser_provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(browser_provider);
        tool
    }
}

impl Default for BrowserClickElementTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserClickElementTool {
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
            .get("query")
            .or_else(|| request.arguments.get("target"))
            .or_else(|| request.arguments.get("element"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required parameter 'query' or 'target'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .click_element(browser_type, target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool for focusing a matched DOM element inside active browser page.
pub struct BrowserFocusElementTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserFocusElementTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_focus_element".to_string(),
                name: "Focus Browser DOM Element".to_string(),
                description:
                    "Focuses a matched input or element safely inside active browser page."
                        .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Element query, text, or target" },
                        "target": { "type": "string", "description": "Element query, text, or target" },
                        "browser": { "type": "string", "description": "Browser name" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(browser_provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(browser_provider);
        tool
    }
}

impl Default for BrowserFocusElementTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserFocusElementTool {
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
            .get("query")
            .or_else(|| request.arguments.get("target"))
            .or_else(|| request.arguments.get("element"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required parameter 'query' or 'target'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .focus_element(browser_type, target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool for extracting text content from a DOM element inside active browser page.
pub struct BrowserGetElementTextTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserGetElementTextTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_get_element_text".to_string(),
                name: "Get Browser Element Text".to_string(),
                description:
                    "Retrieves text content from a matched DOM element inside active browser page."
                        .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Element query, text, or target" },
                        "target": { "type": "string", "description": "Element query, text, or target" },
                        "browser": { "type": "string", "description": "Browser name" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(browser_provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(browser_provider);
        tool
    }
}

impl Default for BrowserGetElementTextTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserGetElementTextTool {
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
            .get("query")
            .or_else(|| request.arguments.get("target"))
            .or_else(|| request.arguments.get("element"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required parameter 'query' or 'target'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .get_element_text(browser_type, target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
    }
}

/// Tool for retrieving attributes of a DOM element inside active browser page.
pub struct BrowserGetElementAttributesTool {
    definition: ToolDefinition,
    browser_provider: Option<Arc<dyn BrowserProvider>>,
}

impl BrowserGetElementAttributesTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                id: "browser_get_element_attributes".to_string(),
                name: "Get Browser Element Attributes".to_string(),
                description: "Retrieves HTML attributes and properties of a matched DOM element."
                    .to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Element query, text, or target" },
                        "target": { "type": "string", "description": "Element query, text, or target" },
                        "browser": { "type": "string", "description": "Browser name" }
                    }
                }),
                risk_level: RiskLevel::Low,
                required_permissions: vec!["browser".to_string()],
                timeout_secs: 15,
            },
            browser_provider: None,
        }
    }

    pub fn with_browser_provider(browser_provider: Arc<dyn BrowserProvider>) -> Self {
        let mut tool = Self::new();
        tool.browser_provider = Some(browser_provider);
        tool
    }
}

impl Default for BrowserGetElementAttributesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserGetElementAttributesTool {
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
            .get("query")
            .or_else(|| request.arguments.get("target"))
            .or_else(|| request.arguments.get("element"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: self.definition.id.clone(),
                details: "Missing required parameter 'query' or 'target'".to_string(),
            })?;

        let browser_str = request
            .arguments
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("Chrome");
        let browser_type = BrowserType::from_str(browser_str);

        let provider: Arc<dyn BrowserProvider> =
            self.browser_provider.clone().unwrap_or_else(|| {
                Arc::new(PlatformBrowserProvider::new(ctx.platform_adapter.clone()))
            });

        let res = provider
            .get_element_attributes(browser_type, target)
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.definition.id.clone(),
                cause: e.to_string(),
            })?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            request.request_id,
            self.definition.id.clone(),
            json!(res),
            elapsed,
        ))
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
        registry.register(Box::new(DescribeScreenTool::new()));
        registry.register(Box::new(ReadScreenTextTool::new()));
        registry.register(Box::new(ReadScreenTool::new()));
        registry.register(Box::new(DetectScreenElementsTool::new()));
        registry.register(Box::new(InspectUiTreeTool::new()));
        registry.register(Box::new(GetBrowserStatusTool::new()));
        registry.register(Box::new(OpenBrowserTool::new()));
        registry.register(Box::new(NavigateBrowserTool::new()));
        registry.register(Box::new(BrowserBackTool::new()));
        registry.register(Box::new(BrowserForwardTool::new()));
        registry.register(Box::new(BrowserReloadTool::new()));
        registry.register(Box::new(BrowserCurrentPageTool::new()));
        registry.register(Box::new(BrowserListTabsTool::new()));
        registry.register(Box::new(BrowserNewTabTool::new()));
        registry.register(Box::new(BrowserSwitchTabTool::new()));
        registry.register(Box::new(BrowserCloseTabTool::new()));
        registry.register(Box::new(BrowserFindElementTool::new()));
        registry.register(Box::new(BrowserClickElementTool::new()));
        registry.register(Box::new(BrowserFocusElementTool::new()));
        registry.register(Box::new(BrowserGetElementTextTool::new()));
        registry.register(Box::new(BrowserGetElementAttributesTool::new()));
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
    use jarvis_ai::{MockOcrProvider, MockVisionProvider};
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
                data: vec![1, 2, 3, 4],
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
        let req = ToolRequest::new(
            "take_screenshot_region",
            json!({ "x": 0, "y": 0, "width": 500, "height": 400 }),
        );
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

        let req = ToolRequest::new(
            "show_notification",
            json!({
                "title": "JARVIS",
                "message": "Task completed successfully"
            }),
        );
        let result = registry.execute(req, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["title"], "JARVIS");
        assert_eq!(result.data["message_length"], 27);
        assert!(result.data["notification_id"].is_string());
    }

    #[tokio::test]
    async fn test_describe_screen_tool_success_with_mock_provider() {
        let registry = ToolRegistry::with_builtins();
        assert!(registry.get("describe_screen").is_some());

        let mock_vision = Arc::new(
            MockVisionProvider::new().with_canned_description("Desktop contains Chrome and VSCode"),
        );
        let tool = DescribeScreenTool::with_provider(mock_vision);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("describe_screen", json!({ "prompt": "What do you see?" }));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(
            result.data["description"],
            "Desktop contains Chrome and VSCode"
        );
        assert_eq!(result.data["model_id"], "mock-vision-model");
    }

    #[tokio::test]
    async fn test_describe_screen_tool_default_prompt() {
        let mock_vision =
            Arc::new(MockVisionProvider::new().with_canned_description("Clean desktop overview"));
        let tool = DescribeScreenTool::with_provider(mock_vision);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("describe_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["description"], "Clean desktop overview");
    }

    #[tokio::test]
    async fn test_describe_screen_tool_failure_propagation() {
        let failing_vision = Arc::new(MockVisionProvider::new().with_failing(true));
        let tool = DescribeScreenTool::with_provider(failing_vision);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("describe_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(!result.success);
        assert!(result
            .error
            .unwrap()
            .contains("Mock vision provider is configured to fail"));
    }

    #[tokio::test]
    async fn test_read_screen_text_tool_success_with_mock_ocr() {
        let registry = ToolRegistry::with_builtins();
        assert!(registry.get("read_screen_text").is_some());

        let mock_ocr = Arc::new(MockOcrProvider::new().with_canned_text("JARVIS OCR TEST 12345"));
        let tool = ReadScreenTextTool::with_provider(mock_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen_text", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["text"], "JARVIS OCR TEST 12345");
        assert_eq!(result.data["has_text"], true);
        assert_eq!(result.data["char_count"], 21);
    }

    #[tokio::test]
    async fn test_read_screen_text_tool_empty_ocr_result() {
        let mock_ocr = Arc::new(MockOcrProvider::new().with_canned_text("   "));
        let tool = ReadScreenTextTool::with_provider(mock_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen_text", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["text"], "");
        assert_eq!(result.data["has_text"], false);
        assert_eq!(result.data["char_count"], 0);
    }

    #[tokio::test]
    async fn test_read_screen_text_tool_failure_propagation() {
        let failing_ocr = Arc::new(MockOcrProvider::new().with_failing(true));
        let tool = ReadScreenTextTool::with_provider(failing_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen_text", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(!result.success);
        assert!(result
            .error
            .unwrap()
            .contains("Mock OCR provider configured to fail"));
    }

    // ===================================================
    // ReadScreenTool (canonical "read_screen") unit tests
    // ===================================================

    #[test]
    fn test_read_screen_tool_registration() {
        let registry = ToolRegistry::with_builtins();
        assert!(
            registry.get("read_screen").is_some(),
            "read_screen must be registered in ToolRegistry::with_builtins()"
        );
    }

    #[test]
    fn test_read_screen_tool_schema_validation() {
        let tool = ReadScreenTool::new();
        let def = tool.definition();
        assert_eq!(def.id, "read_screen");
        assert_eq!(def.name, "Read Screen");
        assert_eq!(def.risk_level, RiskLevel::Low);
        assert!(def
            .required_permissions
            .contains(&"screen_capture".to_string()));
        assert!(def.required_permissions.contains(&"ocr".to_string()));
        let schema = &def.parameters_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["language"].is_object());
    }

    #[tokio::test]
    async fn test_read_screen_tool_success_with_mock_ocr() {
        let mock_ocr = Arc::new(
            MockOcrProvider::new()
                .with_canned_text("JARVIS OCR TEST\nSYSTEM STATUS: ONLINE\nCPU: 42 PERCENT"),
        );
        let tool = ReadScreenTool::with_provider(mock_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.data["text"]
            .as_str()
            .unwrap()
            .contains("JARVIS OCR TEST"));
        assert_eq!(result.data["has_text"], true);
        assert!(result.data["char_count"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_read_screen_tool_full_acceptance_text() {
        let canned = "JARVIS OCR TEST\nSYSTEM STATUS: ONLINE\nCPU: 42 PERCENT\nMEMORY: 61 PERCENT\nTEST NUMBER: 12345";
        let mock_ocr = Arc::new(MockOcrProvider::new().with_canned_text(canned));
        let tool = ReadScreenTool::with_provider(mock_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        let text = result.data["text"].as_str().unwrap();
        assert!(text.contains("JARVIS OCR TEST"));
        assert!(text.contains("SYSTEM STATUS: ONLINE"));
        assert!(text.contains("CPU: 42 PERCENT"));
        assert!(text.contains("MEMORY: 61 PERCENT"));
        assert!(text.contains("TEST NUMBER: 12345"));
    }

    #[tokio::test]
    async fn test_read_screen_tool_empty_ocr_result() {
        let mock_ocr = Arc::new(MockOcrProvider::new().with_canned_text("   "));
        let tool = ReadScreenTool::with_provider(mock_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["text"], "");
        assert_eq!(result.data["has_text"], false);
        assert_eq!(result.data["char_count"], 0);
    }

    #[tokio::test]
    async fn test_read_screen_tool_failure_propagation() {
        let failing_ocr = Arc::new(MockOcrProvider::new().with_failing(true));
        let tool = ReadScreenTool::with_provider(failing_ocr);
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("read_screen", json!({}));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(!result.success);
        assert!(result
            .error
            .unwrap()
            .contains("Mock OCR provider configured to fail"));
    }

    #[test]
    fn test_read_screen_tool_id_and_read_screen_text_are_distinct() {
        let registry = ToolRegistry::with_builtins();
        // Both tools must be registered independently
        assert!(registry.get("read_screen").is_some());
        assert!(registry.get("read_screen_text").is_some());
        // They must have distinct IDs
        let rs = registry.get("read_screen").unwrap();
        let rst = registry.get("read_screen_text").unwrap();
        assert_eq!(rs.definition().id, "read_screen");
        assert_eq!(rst.definition().id, "read_screen_text");
    }

    #[test]
    fn test_detect_screen_elements_tool_registered() {
        let registry = ToolRegistry::with_builtins();
        let tool = registry.get("detect_screen_elements");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().definition().id, "detect_screen_elements");
    }

    #[tokio::test]
    async fn test_detect_screen_elements_tool_execute_with_mock_vision() {
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let mock_vision = Arc::new(MockVisionProvider::new().with_canned_description(
            "{\"elements\": [{\"type\": \"button\", \"label\": \"Submit\", \"x\": 100, \"y\": 200, \"width\": 80, \"height\": 30, \"center_x\": 140, \"center_y\": 215, \"confidence\": 0.9}]}"
        ));
        let tool = DetectScreenElementsTool::with_vision_provider(mock_vision);
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new(
            "detect_screen_elements",
            json!({ "query": "find the Submit button" }),
        );
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        let data = result.data;
        assert_eq!(data["element_count"], 1);
        assert_eq!(data["query"], "find the Submit button");
    }

    #[test]
    fn test_inspect_ui_tree_tool_registered() {
        let registry = ToolRegistry::with_builtins();
        let tool = registry.get("inspect_ui_tree");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().definition().id, "inspect_ui_tree");
    }

    #[tokio::test]
    async fn test_inspect_ui_tree_tool_execute() {
        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let tool = InspectUiTreeTool::new();
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("inspect_ui_tree", json!({ "query": "Soft Reset" }));
        let result = tool.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        let data = result.data;
        assert_eq!(data["query"], "Soft Reset");
        assert_eq!(data["source"], "WindowsUIAutomation");
    }

    #[tokio::test]
    async fn test_m09_02_browser_tools_registration_and_execution() {
        use jarvis_browser::MockBrowserProvider;

        let registry = ToolRegistry::with_builtins();
        let expected_tools = [
            "browser_back",
            "browser_forward",
            "browser_reload",
            "browser_current_page",
            "browser_list_tabs",
            "browser_new_tab",
            "browser_switch_tab",
            "browser_close_tab",
        ];

        for id in &expected_tools {
            assert!(
                registry.get(id).is_some(),
                "Tool {} should be registered in ToolRegistry::with_builtins()",
                id
            );
        }

        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);
        let mock_provider =
            Arc::new(MockBrowserProvider::new().with_running(true, 9999, "Google Chrome"));

        let back_tool = BrowserBackTool::with_browser_provider(mock_provider.clone());
        let res_back = back_tool
            .execute(ToolRequest::new("browser_back", json!({})), &ctx)
            .await
            .unwrap();
        assert!(res_back.success);

        let new_tab_tool = BrowserNewTabTool::with_browser_provider(mock_provider.clone());
        let res_tab = new_tab_tool
            .execute(
                ToolRequest::new("browser_new_tab", json!({ "url": "wikipedia.org" })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_tab.success);

        let list_tabs_tool = BrowserListTabsTool::with_browser_provider(mock_provider.clone());
        let res_list = list_tabs_tool
            .execute(ToolRequest::new("browser_list_tabs", json!({})), &ctx)
            .await
            .unwrap();
        assert!(res_list.success);
        assert_eq!(res_list.data["tab_count"], 2);

        let switch_tab_tool = BrowserSwitchTabTool::with_browser_provider(mock_provider.clone());
        let res_switch = switch_tab_tool
            .execute(
                ToolRequest::new("browser_switch_tab", json!({ "tab_index": 1 })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_switch.success);

        let close_tab_tool = BrowserCloseTabTool::with_browser_provider(mock_provider.clone());
        let res_close = close_tab_tool
            .execute(ToolRequest::new("browser_close_tab", json!({})), &ctx)
            .await
            .unwrap();
        assert!(res_close.success);
    }

    #[tokio::test]
    async fn test_m09_03_browser_dom_tools_registration_and_execution() {
        use jarvis_browser::MockBrowserProvider;

        let registry = ToolRegistry::with_builtins();
        let expected_tools = [
            "browser_find_element",
            "browser_click_element",
            "browser_focus_element",
            "browser_get_element_text",
            "browser_get_element_attributes",
        ];

        for id in &expected_tools {
            assert!(
                registry.get(id).is_some(),
                "Tool {} should be registered in ToolRegistry::with_builtins()",
                id
            );
        }

        let adapter = Arc::new(MockPlatformAdapter::new(false));
        let ctx = ToolExecutionContext::new(adapter);
        let mock_provider =
            Arc::new(MockBrowserProvider::new().with_running(true, 9999, "Google Chrome"));

        let find_tool = BrowserFindElementTool::with_browser_provider(mock_provider.clone());
        let res_find = find_tool
            .execute(
                ToolRequest::new("browser_find_element", json!({ "query": "search box" })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_find.success);
        assert_eq!(res_find.data["match_count"], 1);

        let click_tool = BrowserClickElementTool::with_browser_provider(mock_provider.clone());
        let res_click = click_tool
            .execute(
                ToolRequest::new("browser_click_element", json!({ "query": "Sign In" })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_click.success);
        assert_eq!(res_click.data["action"], "click");

        let focus_tool = BrowserFocusElementTool::with_browser_provider(mock_provider.clone());
        let res_focus = focus_tool
            .execute(
                ToolRequest::new("browser_focus_element", json!({ "query": "search box" })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_focus.success);
        assert_eq!(res_focus.data["action"], "focus");

        let text_tool = BrowserGetElementTextTool::with_browser_provider(mock_provider.clone());
        let res_text = text_tool
            .execute(
                ToolRequest::new("browser_get_element_text", json!({ "query": "Sign In" })),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_text.success);
        assert_eq!(res_text.data["text"], "Sign In");

        let attr_tool =
            BrowserGetElementAttributesTool::with_browser_provider(mock_provider.clone());
        let res_attr = attr_tool
            .execute(
                ToolRequest::new(
                    "browser_get_element_attributes",
                    json!({ "query": "search box" }),
                ),
                &ctx,
            )
            .await
            .unwrap();
        assert!(res_attr.success);
        assert_eq!(res_attr.data["action"], "get_attributes");
    }
}
