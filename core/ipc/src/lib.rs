//! JARVIS Core IPC Service Layer
//!
//! Exposes the Core Orchestrator over the IPC boundary using strongly typed
//! protocol envelopes and abstract transports.
//!
//! # Architecture
//!
//! ```text
//! Desktop Client / External Service
//!     ↓ (IpcTransport: Named Pipe / Memory)
//! CoreIpcServer
//!     ↓
//! Core Orchestrator (Task -> Policy -> Tool -> Platform)
//!     ↓
//! Execution Outcome
//!     ↓
//! IpcEnvelope (CommandResponse / Event / Health)
//!     ↓ (IpcTransport)
//! Desktop Client
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 2, Milestone M02.04 & M02.08

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use jarvis_ipc::IpcTransport;
use jarvis_orchestrator::{ExecutionOutcome, Orchestrator};
use jarvis_protocol::{
    HealthStatusMessage, IpcEnvelope, IpcError, IpcMessageType, ServiceHealthState,
};

/// High-level IPC client for invoking the JARVIS daemon over a transport.
pub struct CoreIpcClient<T: IpcTransport> {
    transport: T,
    timeout_duration: Duration,
}

impl<T: IpcTransport> CoreIpcClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            timeout_duration: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_duration = timeout;
        self
    }

    /// Submit a command string across the IPC boundary and wait for the execution outcome.
    #[instrument(skip(self), fields(command = %command))]
    pub async fn submit_command(&mut self, command: &str) -> Result<ExecutionOutcome, IpcError> {
        let request_id = format!("req_{}", Uuid::new_v4());
        let payload = json!({ "command": command }).to_string();

        let req_env = IpcEnvelope::new(
            IpcMessageType::Command,
            payload,
            request_id.clone(),
            None,
            None,
        );

        // Send request envelope
        self.transport
            .send_envelope(&req_env, self.timeout_duration)
            .await?;

        // Receive response envelope
        let resp_env = self
            .transport
            .receive_envelope(self.timeout_duration)
            .await?;

        if resp_env.request_id != request_id {
            return Err(IpcError::ProtocolError(format!(
                "Correlation mismatch: expected request_id '{}', received '{}'",
                request_id, resp_env.request_id
            )));
        }

        match resp_env.message_type {
            IpcMessageType::CommandResponse => {
                let outcome: ExecutionOutcome = serde_json::from_str(&resp_env.payload_json)
                    .map_err(|e| {
                        IpcError::ProtocolError(format!("Failed to parse response outcome: {}", e))
                    })?;
                Ok(outcome)
            }
            _ => Err(IpcError::ProtocolError(format!(
                "Unexpected response message type: {:?}",
                resp_env.message_type
            ))),
        }
    }

    /// Query the service health across the IPC boundary.
    pub async fn check_health(&mut self) -> Result<HealthStatusMessage, IpcError> {
        let request_id = format!("req_{}", Uuid::new_v4());
        let req_env = IpcEnvelope::new(
            IpcMessageType::HealthCheck,
            "{}".to_string(),
            request_id.clone(),
            None,
            None,
        );

        self.transport
            .send_envelope(&req_env, self.timeout_duration)
            .await?;

        let resp_env = self
            .transport
            .receive_envelope(self.timeout_duration)
            .await?;

        let health: HealthStatusMessage =
            serde_json::from_str(&resp_env.payload_json).map_err(|e| {
                IpcError::ProtocolError(format!("Failed to parse health status: {}", e))
            })?;

        Ok(health)
    }
}

/// Server that hosts the JARVIS core orchestrator over an IPC transport connection.
pub struct CoreIpcServer<T: IpcTransport> {
    orchestrator: Arc<Orchestrator>,
    transport: T,
}

impl<T: IpcTransport> CoreIpcServer<T> {
    pub fn new(orchestrator: Arc<Orchestrator>, transport: T) -> Self {
        Self {
            orchestrator,
            transport,
        }
    }

    /// Process a single incoming IPC request and dispatch to the orchestrator.
    #[instrument(skip(self))]
    pub async fn handle_next_request(&mut self) -> Result<bool, IpcError> {
        let envelope = match self
            .transport
            .receive_envelope(Duration::from_secs(60))
            .await
        {
            Ok(env) => env,
            Err(IpcError::Timeout { .. }) => return Ok(true), // Keep alive
            Err(IpcError::TransportError(_)) => return Ok(false), // Connection closed
            Err(e) => return Err(e),
        };

        match envelope.message_type {
            IpcMessageType::Command => {
                let payload: serde_json::Value = serde_json::from_str(&envelope.payload_json)
                    .map_err(|e| IpcError::InvalidRequest(e.to_string()))?;

                let command_text = payload
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                info!(command = %command_text, "CoreIpcServer handling command");

                // Execute through the real Orchestrator pipeline
                let outcome = self.orchestrator.execute_command(command_text).await;

                let response_payload = serde_json::to_string(&outcome)
                    .map_err(|e| IpcError::InternalError(e.to_string()))?;

                let resp_env = IpcEnvelope::new(
                    IpcMessageType::CommandResponse,
                    response_payload,
                    envelope.request_id,
                    envelope.trace_id,
                    envelope.task_id,
                );

                self.transport
                    .send_envelope(&resp_env, Duration::from_secs(5))
                    .await?;
            }
            IpcMessageType::HealthCheck => {
                let health = HealthStatusMessage {
                    service_name: "jarvis-core".to_string(),
                    health: ServiceHealthState::Ready,
                    message: "Core daemon operational".to_string(),
                    uptime_ms: 1000,
                    last_updated_ms: Utc::now().timestamp_millis(),
                    details: HashMap::new(),
                };

                let response_payload = serde_json::to_string(&health)
                    .map_err(|e| IpcError::InternalError(e.to_string()))?;

                let resp_env = IpcEnvelope::new(
                    IpcMessageType::HealthResponse,
                    response_payload,
                    envelope.request_id,
                    envelope.trace_id,
                    envelope.task_id,
                );

                self.transport
                    .send_envelope(&resp_env, Duration::from_secs(5))
                    .await?;
            }
            _ => {
                warn!(type_ = ?envelope.message_type, "Unhandled envelope message type");
            }
        }

        Ok(true)
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use jarvis_ipc::MemoryTransport;
    use jarvis_platform::*;

    // Mock platform adapter
    struct MockAdapter;

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
            Ok(ProcessInfo {
                pid: 9999,
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
    async fn test_end_to_end_ipc_command_execution() {
        let adapter = Arc::new(MockAdapter);
        let orchestrator = Arc::new(Orchestrator::new(adapter));

        let (client_transport, server_transport) = MemoryTransport::create_pair(16);

        let mut client = CoreIpcClient::new(client_transport);
        let mut server = CoreIpcServer::new(orchestrator, server_transport);

        // Spawn server processing in background
        let server_task = tokio::spawn(async move {
            server.handle_next_request().await.unwrap();
        });

        // Client submits "open chrome" across the IPC wire
        let outcome = client.submit_command("open chrome").await.unwrap();

        server_task.await.unwrap();

        match outcome {
            ExecutionOutcome::Success {
                spoken_response,
                tool_name,
                tool_data,
                ..
            } => {
                assert_eq!(tool_name, "open_application");
                assert_eq!(spoken_response, "Chrome is open, sir.");
                assert_eq!(tool_data["pid"], 9999);
            }
            _ => panic!("Expected Success outcome across IPC wire"),
        }
    }

    #[tokio::test]
    async fn test_ipc_health_check() {
        let adapter = Arc::new(MockAdapter);
        let orchestrator = Arc::new(Orchestrator::new(adapter));

        let (client_transport, server_transport) = MemoryTransport::create_pair(16);

        let mut client = CoreIpcClient::new(client_transport);
        let mut server = CoreIpcServer::new(orchestrator, server_transport);

        let server_task = tokio::spawn(async move {
            server.handle_next_request().await.unwrap();
        });

        let health = client.check_health().await.unwrap();

        server_task.await.unwrap();

        assert_eq!(health.service_name, "jarvis-core");
        assert_eq!(health.health, ServiceHealthState::Ready);
    }
}
