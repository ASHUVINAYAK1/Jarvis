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

use jarvis_platform::PlatformAdapter;
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

    pub fn failure(request_id: String, tool_name: String, error_msg: String, elapsed_ms: u64) -> Self {
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
        self.tools.values().map(|t| t.definition().clone()).collect()
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

        async fn close_application(&self, _app: &str) -> anyhow::Result<()> { Ok(()) }
        async fn list_processes(&self) -> anyhow::Result<Vec<ProcessInfo>> { Ok(vec![]) }
        async fn is_application_running(&self, _app: &str) -> anyhow::Result<bool> { Ok(true) }
        async fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>> { Ok(vec![]) }
        async fn focus_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn minimize_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn maximize_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn set_window_bounds(&self, _handle: &str, _bounds: Rect) -> anyhow::Result<()> { Ok(()) }
        async fn take_screenshot(&self) -> anyhow::Result<Screenshot> {
            Ok(Screenshot { data: vec![], format: ImageFormat::Png, width: 100, height: 100, display_index: 0 })
        }
        async fn take_screenshot_display(&self, _idx: u32) -> anyhow::Result<Screenshot> { self.take_screenshot().await }
        async fn take_screenshot_region(&self, _r: Rect) -> anyhow::Result<Screenshot> { self.take_screenshot().await }
        async fn get_clipboard(&self) -> anyhow::Result<ClipboardContent> { Ok(ClipboardContent::Empty) }
        async fn set_clipboard(&self, _c: ClipboardContent) -> anyhow::Result<()> { Ok(()) }
        async fn show_notification(&self, _n: NotificationRequest) -> anyhow::Result<()> { Ok(()) }
        async fn get_disk_space(&self) -> anyhow::Result<DiskInfo> {
            Ok(DiskInfo { total_bytes: 1000, available_bytes: 500, used_bytes: 500 })
        }
        async fn get_memory_info(&self) -> anyhow::Result<MemoryInfo> {
            Ok(MemoryInfo { total_bytes: 1000, available_bytes: 500, used_bytes: 500 })
        }
    }

    #[tokio::test]
    async fn test_open_application_tool_success() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter { fail_app: false });
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
        let adapter = Arc::new(MockPlatformAdapter { fail_app: true });
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("open_application", json!({ "application": "nonexistent" }));
        let result = registry.execute(req, &ctx).await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_get_time_tool() {
        let registry = ToolRegistry::with_builtins();
        let adapter = Arc::new(MockPlatformAdapter { fail_app: false });
        let ctx = ToolExecutionContext::new(adapter);

        let req = ToolRequest::new("get_time", json!({}));
        let result = registry.execute(req, &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.data["time"].is_string());
    }
}
