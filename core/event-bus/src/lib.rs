//! JARVIS Internal Event Bus
//!
//! An in-process publish/subscribe event system that allows JARVIS services
//! to communicate asynchronously without tight coupling.
//!
//! # Design
//!
//! Uses Tokio broadcast channels under the hood.
//! Any service can publish events; any service can subscribe.
//! Subscribers receive all events published after they subscribe.
//!
//! # Usage
//!
//! ```rust,no_run
//! use jarvis_event_bus::{EventBus, JarvisEvent, TaskEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = EventBus::new(256);
//!
//!     let mut subscriber = bus.subscribe();
//!
//!     // Publish an event
//!     bus.publish(JarvisEvent::Task(TaskEvent::Started {
//!         task_id: "task_123".to_string(),
//!     })).await;
//!
//!     // Receive the event
//!     if let Ok(event) = subscriber.recv().await {
//!         println!("Got event: {:?}", event);
//!     }
//! }
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 3, Milestone M02.05 / M03.xx

use tokio::sync::broadcast;
use tracing::instrument;

// ============================================================
// Event Types
// ============================================================

/// All events that flow through the JARVIS event bus.
///
/// New event variants should be added here as new subsystems are implemented.
#[derive(Debug, Clone)]
pub enum JarvisEvent {
    /// Task lifecycle events
    Task(TaskEvent),

    /// Tool execution events
    Tool(ToolEvent),

    /// Voice/speech events
    Voice(VoiceEvent),

    /// Service health events
    Health(HealthEvent),

    /// Security/approval events
    Security(SecurityEvent),

    /// Device mesh events
    Mesh(MeshEvent),
}

// ============================================================
// Task Events
// ============================================================

#[derive(Debug, Clone)]
pub enum TaskEvent {
    Created { task_id: String, name: String },
    Started { task_id: String },
    Paused { task_id: String, reason: String },
    Resumed { task_id: String },
    Completed { task_id: String, summary: Option<String> },
    Failed { task_id: String, error: String },
    Cancelled { task_id: String },
    StepStarted { task_id: String, step_index: u32, description: String },
    StepCompleted { task_id: String, step_index: u32 },
    StepFailed { task_id: String, step_index: u32, error: String },
}

// ============================================================
// Tool Events
// ============================================================

#[derive(Debug, Clone)]
pub enum ToolEvent {
    /// Tool execution has started.
    Started {
        request_id: String,
        task_id: Option<String>,
        tool_name: String,
    },
    /// Tool completed successfully.
    Completed {
        request_id: String,
        tool_name: String,
        duration_ms: u64,
    },
    /// Tool failed.
    Failed {
        request_id: String,
        tool_name: String,
        error: String,
    },
    /// Tool requires user approval before execution.
    ApprovalRequested {
        request_id: String,
        tool_name: String,
        action_description: String,
    },
    /// User granted approval.
    ApprovalGranted { request_id: String },
    /// User denied approval.
    ApprovalDenied { request_id: String },
}

// ============================================================
// Voice Events
// ============================================================

#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// Wake word was detected.
    WakeWordDetected { confidence: f32 },
    /// Voice activity detected (someone started speaking).
    SpeechStarted,
    /// Voice activity ended (silence detected).
    SpeechEnded,
    /// Audio transcribed to text.
    Transcribed { text: String, confidence: f32, language: String },
    /// TTS synthesis started.
    SynthesisStarted { text: String },
    /// TTS synthesis completed.
    SynthesisCompleted { duration_ms: u64 },
    /// TTS was interrupted (barge-in).
    SynthesisInterrupted,
}

// ============================================================
// Health Events
// ============================================================

#[derive(Debug, Clone)]
pub enum HealthEvent {
    ServiceStarting { service_name: String },
    ServiceReady { service_name: String },
    ServiceDegraded { service_name: String, reason: String },
    ServiceFailed { service_name: String, error: String },
    ServiceStopping { service_name: String },
    ServiceStopped { service_name: String },
}

// ============================================================
// Security Events
// ============================================================

#[derive(Debug, Clone)]
pub enum SecurityEvent {
    PolicyViolationAttempted {
        action: String,
        reason: String,
        source: String,
    },
    CredentialRequested {
        service: String,
        purpose: String,
    },
    PromptInjectionDetected {
        source: String,
        snippet: String,
    },
    AuditLogEntry {
        action: String,
        result: String,
        request_id: String,
    },
}

// ============================================================
// Mesh Events
// ============================================================

#[derive(Debug, Clone)]
pub enum MeshEvent {
    DeviceDiscovered { device_id: String, device_type: String },
    DeviceConnected { device_id: String },
    DeviceDisconnected { device_id: String },
    TaskMigrated { task_id: String, from_device: String, to_device: String },
}

// ============================================================
// Event Bus
// ============================================================

/// The JARVIS event bus.
///
/// Backed by a Tokio broadcast channel. Subscribers that fall behind
/// will receive `RecvError::Lagged` and should handle this gracefully.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<JarvisEvent>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    ///
    /// `capacity` is the maximum number of events that can be buffered.
    /// Slow subscribers will miss events if they fall behind by this amount.
    /// Recommended: 256 for most services; 1024 for monitoring/audit.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events.
    ///
    /// The subscriber will receive all events published after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<JarvisEvent> {
        self.sender.subscribe()
    }

    /// Publish an event to all current subscribers.
    ///
    /// If there are no subscribers, the event is silently dropped.
    /// Returns the number of subscribers that received the event.
    #[instrument(skip(self, event), fields(event_type = %event_type_name(&event)))]
    pub async fn publish(&self, event: JarvisEvent) -> usize {
        match self.sender.send(event) {
            Ok(n) => {
                tracing::debug!(subscribers = n, "Event published");
                n
            }
            Err(_) => {
                // No subscribers — this is normal, not an error
                0
            }
        }
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

fn event_type_name(event: &JarvisEvent) -> &'static str {
    match event {
        JarvisEvent::Task(e) => match e {
            TaskEvent::Created { .. } => "Task::Created",
            TaskEvent::Started { .. } => "Task::Started",
            TaskEvent::Paused { .. } => "Task::Paused",
            TaskEvent::Resumed { .. } => "Task::Resumed",
            TaskEvent::Completed { .. } => "Task::Completed",
            TaskEvent::Failed { .. } => "Task::Failed",
            TaskEvent::Cancelled { .. } => "Task::Cancelled",
            TaskEvent::StepStarted { .. } => "Task::StepStarted",
            TaskEvent::StepCompleted { .. } => "Task::StepCompleted",
            TaskEvent::StepFailed { .. } => "Task::StepFailed",
        },
        JarvisEvent::Tool(e) => match e {
            ToolEvent::Started { .. } => "Tool::Started",
            ToolEvent::Completed { .. } => "Tool::Completed",
            ToolEvent::Failed { .. } => "Tool::Failed",
            ToolEvent::ApprovalRequested { .. } => "Tool::ApprovalRequested",
            ToolEvent::ApprovalGranted { .. } => "Tool::ApprovalGranted",
            ToolEvent::ApprovalDenied { .. } => "Tool::ApprovalDenied",
        },
        JarvisEvent::Voice(_) => "Voice",
        JarvisEvent::Health(_) => "Health",
        JarvisEvent::Security(_) => "Security",
        JarvisEvent::Mesh(_) => "Mesh",
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_and_receive() {
        let bus = EventBus::new(64);
        let mut subscriber = bus.subscribe();

        let event = JarvisEvent::Task(TaskEvent::Started {
            task_id: "task_001".to_string(),
        });

        bus.publish(event).await;

        let received = subscriber.recv().await.unwrap();
        assert!(matches!(
            received,
            JarvisEvent::Task(TaskEvent::Started { .. })
        ));
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new(64);
        let mut sub1 = bus.subscribe();
        let mut sub2 = bus.subscribe();

        bus.publish(JarvisEvent::Health(HealthEvent::ServiceReady {
            service_name: "orchestrator".to_string(),
        })).await;

        assert!(sub1.recv().await.is_ok());
        assert!(sub2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_no_subscribers_does_not_panic() {
        let bus = EventBus::new(64);
        // No subscriber — should not panic
        let count = bus.publish(JarvisEvent::Voice(VoiceEvent::WakeWordDetected {
            confidence: 0.95,
        })).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_voice_event_transcription() {
        let bus = EventBus::new(64);
        let mut sub = bus.subscribe();

        bus.publish(JarvisEvent::Voice(VoiceEvent::Transcribed {
            text: "open Chrome".to_string(),
            confidence: 0.98,
            language: "en-US".to_string(),
        })).await;

        if let Ok(JarvisEvent::Voice(VoiceEvent::Transcribed { text, .. })) = sub.recv().await {
            assert_eq!(text, "open Chrome");
        } else {
            panic!("Expected Transcribed event");
        }
    }

    #[tokio::test]
    async fn test_tool_approval_flow() {
        let bus = EventBus::new(64);
        let mut sub = bus.subscribe();

        bus.publish(JarvisEvent::Tool(ToolEvent::ApprovalRequested {
            request_id: "req_001".to_string(),
            tool_name: "send_email".to_string(),
            action_description: "Send email to user@example.com".to_string(),
        })).await;

        if let Ok(JarvisEvent::Tool(ToolEvent::ApprovalRequested { tool_name, .. })) = sub.recv().await {
            assert_eq!(tool_name, "send_email");
        } else {
            panic!("Expected ApprovalRequested event");
        }
    }
}
