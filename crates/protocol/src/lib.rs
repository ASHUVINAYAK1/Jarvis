//! JARVIS Protocol Definitions & Wire Envelopes
//!
//! Strongly typed protocol contracts, common envelopes, message types, and structured errors
//! matching `proto/jarvis/core/v1/core.proto` and Document 22.
//!
//! # Architecture
//!
//! ```text
//! Transport Bytes
//!     ↓
//! IpcEnvelope (Framed JSON / Protobuf)
//!     ├── RequestHeader (request_id, trace_id, task_id, deadline_ms)
//!     ├── MessageType (Command, Response, Event, Health, Cancel)
//!     └── Payload
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 2, Milestone M02.01 & M02.02

use std::collections::HashMap;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

// ============================================================
// Identifiers
// ============================================================

/// Protocol version number (Current: 1)
pub const PROTOCOL_VERSION: u32 = 1;

/// Common request header attached to all incoming service requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestHeader {
    pub request_id: String,
    pub task_id: Option<String>,
    pub trace_id: Option<String>,
    pub timestamp_ms: i64,
    pub source: String,
    pub destination: String,
    pub protocol_version: u32,
    pub deadline_ms: Option<i64>,
}

impl RequestHeader {
    pub fn new(source: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            request_id: format!("req_{}", Uuid::new_v4()),
            task_id: None,
            trace_id: Some(format!("trc_{}", Uuid::new_v4())),
            timestamp_ms: Utc::now().timestamp_millis(),
            source: source.into(),
            destination: destination.into(),
            protocol_version: PROTOCOL_VERSION,
            deadline_ms: None,
        }
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_deadline_ms(mut self, deadline_ms: i64) -> Self {
        self.deadline_ms = Some(deadline_ms);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(deadline) = self.deadline_ms {
            Utc::now().timestamp_millis() > deadline
        } else {
            false
        }
    }
}

/// Response status classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseStatus {
    Ok = 1,
    Error = 2,
    Cancelled = 3,
    Timeout = 4,
    PermissionDenied = 5,
    NotFound = 6,
    InvalidArgument = 7,
}

/// Common response header returned with all service replies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseHeader {
    pub request_id: String,
    pub status: ResponseStatus,
    pub error_message: Option<String>,
    pub duration_ms: u64,
}

impl ResponseHeader {
    pub fn ok(request_id: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            request_id: request_id.into(),
            status: ResponseStatus::Ok,
            error_message: None,
            duration_ms,
        }
    }

    pub fn error(request_id: impl Into<String>, status: ResponseStatus, err: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            request_id: request_id.into(),
            status,
            error_message: Some(err.into()),
            duration_ms,
        }
    }
}

// ============================================================
// Core Message Payloads
// ============================================================

/// Origin source of a user command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSource {
    Voice,
    Text,
    Api,
    Scheduled,
    DesktopUi,
}

/// User command message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    pub header: RequestHeader,
    pub text: String,
    pub source: CommandSource,
    pub language_code: String,
}

/// Execution response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponseMessage {
    pub header: ResponseHeader,
    pub task_id: String,
    pub spoken_response: String,
    pub tool_name: Option<String>,
    pub tool_data: serde_json::Value,
}

/// Structured tool invocation payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallMessage {
    pub header: RequestHeader,
    pub tool_name: String,
    pub arguments_json: String,
    pub invoked_by: String,
}

/// Structured tool execution response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub header: ResponseHeader,
    pub tool_name: String,
    pub result_json: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Service health state enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceHealthState {
    Starting,
    Ready,
    Degraded,
    Failed,
    Stopping,
    Stopped,
}

/// Service health status report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusMessage {
    pub service_name: String,
    pub health: ServiceHealthState,
    pub message: String,
    pub uptime_ms: u64,
    pub last_updated_ms: i64,
    pub details: HashMap<String, String>,
}

// ============================================================
// IPC Wire Envelope
// ============================================================

/// Message types crossing the IPC boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcMessageType {
    Command,
    CommandResponse,
    ToolCall,
    ToolResult,
    Event,
    Heartbeat,
    HealthCheck,
    HealthResponse,
    CancelRequest,
}

/// Common wire envelope for all serialized IPC messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub protocol_version: u32,
    pub request_id: String,
    pub trace_id: Option<String>,
    pub task_id: Option<String>,
    pub message_type: IpcMessageType,
    pub payload_json: String,
    pub timestamp_ms: i64,
}

impl IpcEnvelope {
    pub fn new(
        message_type: IpcMessageType,
        payload_json: String,
        request_id: String,
        trace_id: Option<String>,
        task_id: Option<String>,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            trace_id,
            task_id,
            message_type,
            payload_json,
            timestamp_ms: Utc::now().timestamp_millis(),
        }
    }

    /// Encode envelope to JSON bytes for transport.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Decode envelope from transport bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ============================================================
// Structured IPC Errors
// ============================================================

/// Standard structured error taxonomy for all IPC operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum IpcError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Request timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_header_creation() {
        let header = RequestHeader::new("desktop_ui", "core");
        assert_eq!(header.source, "desktop_ui");
        assert_eq!(header.destination, "core");
        assert_eq!(header.protocol_version, PROTOCOL_VERSION);
        assert!(!header.is_expired());
    }

    #[test]
    fn test_deadline_expiration() {
        let past_time = Utc::now().timestamp_millis() - 1000;
        let header = RequestHeader::new("ui", "core").with_deadline_ms(past_time);
        assert!(header.is_expired());
    }

    #[test]
    fn test_ipc_envelope_roundtrip() {
        let payload = serde_json::json!({ "command": "open chrome" }).to_string();
        let env = IpcEnvelope::new(
            IpcMessageType::Command,
            payload.clone(),
            "req_123".to_string(),
            Some("trc_456".to_string()),
            Some("task_789".to_string()),
        );

        let bytes = env.to_bytes().unwrap();
        let decoded = IpcEnvelope::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.request_id, "req_123");
        assert_eq!(decoded.message_type, IpcMessageType::Command);
        assert_eq!(decoded.payload_json, payload);
    }
}
