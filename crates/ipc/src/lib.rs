//! JARVIS Inter-Process Communication (IPC) Transport Layer
//!
//! Provides abstract, resilient, bounded, and async message transports:
//! - Windows Named Pipes (`\\.\pipe\jarvis_ipc`)
//! - In-Memory Bidirectional Channels (Testing & Fast In-Process Routing)
//! - Unix Domain Sockets (Linux readiness)
//!
//! # Framing Protocol
//!
//! Every frame on the wire uses a 4-byte Big-Endian length header:
//! ```text
//! [ 4-byte Big-Endian Payload Length ][ N-byte Serialized IpcEnvelope Payload ]
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 2, Milestone M02.02 & M02.03

use std::time::Duration;

use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::timeout;

use jarvis_protocol::{IpcEnvelope, IpcError};

/// Maximum allowable frame size (16 MB) to prevent denial of service.
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Default Named Pipe path on Windows.
pub const DEFAULT_WINDOWS_PIPE_PATH: &str = r"\\.\pipe\jarvis_core_ipc";

// ============================================================
// Framing Utilities
// ============================================================

/// Write a framed message to any AsyncWrite stream.
pub async fn write_framed<W: AsyncWrite + Unpin>(
    writer: &mut W,
    data: &[u8],
) -> Result<(), IpcError> {
    if data.len() > MAX_FRAME_SIZE {
        return Err(IpcError::InvalidRequest(format!(
            "Payload size {} exceeds maximum allowable frame size {}",
            data.len(),
            MAX_FRAME_SIZE
        )));
    }

    let mut header = [0u8; 4];
    BigEndian::write_u32(&mut header, data.len() as u32);

    writer
        .write_all(&header)
        .await
        .map_err(|e| IpcError::TransportError(format!("Failed to write frame header: {}", e)))?;

    writer
        .write_all(data)
        .await
        .map_err(|e| IpcError::TransportError(format!("Failed to write frame payload: {}", e)))?;

    writer
        .flush()
        .await
        .map_err(|e| IpcError::TransportError(format!("Failed to flush frame: {}", e)))?;

    Ok(())
}

/// Read a framed message from any AsyncRead stream.
pub async fn read_framed<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, IpcError> {
    let mut header = [0u8; 4];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|e| IpcError::TransportError(format!("Failed to read frame header: {}", e)))?;

    let length = BigEndian::read_u32(&header) as usize;

    if length > MAX_FRAME_SIZE {
        return Err(IpcError::ProtocolError(format!(
            "Incoming frame size {} exceeds maximum allowable limit {}",
            length, MAX_FRAME_SIZE
        )));
    }

    let mut buffer = vec![0u8; length];
    reader
        .read_exact(&mut buffer)
        .await
        .map_err(|e| IpcError::TransportError(format!("Failed to read frame payload: {}", e)))?;

    Ok(buffer)
}

// ============================================================
// Transport Trait
// ============================================================

/// Abstract asynchronous bidirectional message transport.
#[async_trait]
pub trait IpcTransport: Send + Sync {
    /// Send an envelope over the transport with timeout.
    async fn send_envelope(
        &mut self,
        envelope: &IpcEnvelope,
        timeout_duration: Duration,
    ) -> Result<(), IpcError>;

    /// Receive the next envelope from the transport with timeout.
    async fn receive_envelope(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<IpcEnvelope, IpcError>;

    /// Close the transport connection gracefully.
    async fn close(&mut self) -> Result<(), IpcError>;
}

// ============================================================
// In-Memory Transport (For Testing & High-Speed Local Routing)
// ============================================================

/// In-memory bidirectional transport using Tokio MPSC channels.
pub struct MemoryTransport {
    sender: mpsc::Sender<Vec<u8>>,
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl MemoryTransport {
    /// Create a connected pair of in-memory transports (A <--> B).
    pub fn create_pair(capacity: usize) -> (Self, Self) {
        let (tx_a, rx_b) = mpsc::channel(capacity);
        let (tx_b, rx_a) = mpsc::channel(capacity);

        (
            Self {
                sender: tx_a,
                receiver: rx_a,
            },
            Self {
                sender: tx_b,
                receiver: rx_b,
            },
        )
    }
}

#[async_trait]
impl IpcTransport for MemoryTransport {
    async fn send_envelope(
        &mut self,
        envelope: &IpcEnvelope,
        timeout_duration: Duration,
    ) -> Result<(), IpcError> {
        let bytes = envelope
            .to_bytes()
            .map_err(|e| IpcError::ProtocolError(e.to_string()))?;

        match timeout(timeout_duration, self.sender.send(bytes)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(IpcError::TransportError(
                "Channel receiver closed".to_string(),
            )),
            Err(_) => Err(IpcError::Timeout {
                duration_ms: timeout_duration.as_millis() as u64,
            }),
        }
    }

    async fn receive_envelope(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<IpcEnvelope, IpcError> {
        match timeout(timeout_duration, self.receiver.recv()).await {
            Ok(Some(bytes)) => {
                IpcEnvelope::from_bytes(&bytes).map_err(|e| IpcError::ProtocolError(e.to_string()))
            }
            Ok(None) => Err(IpcError::TransportError(
                "Channel sender closed".to_string(),
            )),
            Err(_) => Err(IpcError::Timeout {
                duration_ms: timeout_duration.as_millis() as u64,
            }),
        }
    }

    async fn close(&mut self) -> Result<(), IpcError> {
        self.receiver.close();
        Ok(())
    }
}

// ============================================================
// Windows Named Pipe Transport
// ============================================================

#[cfg(windows)]
pub mod windows_pipe {
    use super::*;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };

    /// Windows Named Pipe Server Transport.
    pub struct WindowsNamedPipeServerTransport {
        server: NamedPipeServer,
    }

    impl WindowsNamedPipeServerTransport {
        pub fn bind(pipe_path: &str) -> Result<Self, IpcError> {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_path)
                .map_err(|e| {
                    IpcError::TransportError(format!("Failed to create Named Pipe: {}", e))
                })?;

            Ok(Self { server })
        }

        pub async fn wait_for_client(&mut self) -> Result<(), IpcError> {
            self.server.connect().await.map_err(|e| {
                IpcError::TransportError(format!("Named Pipe connection failed: {}", e))
            })
        }
    }

    #[async_trait]
    impl IpcTransport for WindowsNamedPipeServerTransport {
        async fn send_envelope(
            &mut self,
            envelope: &IpcEnvelope,
            timeout_duration: Duration,
        ) -> Result<(), IpcError> {
            let bytes = envelope
                .to_bytes()
                .map_err(|e| IpcError::ProtocolError(e.to_string()))?;

            match timeout(timeout_duration, write_framed(&mut self.server, &bytes)).await {
                Ok(res) => res,
                Err(_) => Err(IpcError::Timeout {
                    duration_ms: timeout_duration.as_millis() as u64,
                }),
            }
        }

        async fn receive_envelope(
            &mut self,
            timeout_duration: Duration,
        ) -> Result<IpcEnvelope, IpcError> {
            match timeout(timeout_duration, read_framed(&mut self.server)).await {
                Ok(Ok(bytes)) => IpcEnvelope::from_bytes(&bytes)
                    .map_err(|e| IpcError::ProtocolError(e.to_string())),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(IpcError::Timeout {
                    duration_ms: timeout_duration.as_millis() as u64,
                }),
            }
        }

        async fn close(&mut self) -> Result<(), IpcError> {
            self.server.disconnect().map_err(|e| {
                IpcError::TransportError(format!("Named Pipe disconnect failed: {}", e))
            })
        }
    }

    /// Windows Named Pipe Client Transport.
    pub struct WindowsNamedPipeClientTransport {
        client: NamedPipeClient,
    }

    impl WindowsNamedPipeClientTransport {
        pub async fn connect(pipe_path: &str) -> Result<Self, IpcError> {
            let client = ClientOptions::new().open(pipe_path).map_err(|e| {
                IpcError::Unavailable(format!(
                    "Could not connect to Named Pipe '{}': {}",
                    pipe_path, e
                ))
            })?;

            Ok(Self { client })
        }
    }

    #[async_trait]
    impl IpcTransport for WindowsNamedPipeClientTransport {
        async fn send_envelope(
            &mut self,
            envelope: &IpcEnvelope,
            timeout_duration: Duration,
        ) -> Result<(), IpcError> {
            let bytes = envelope
                .to_bytes()
                .map_err(|e| IpcError::ProtocolError(e.to_string()))?;

            match timeout(timeout_duration, write_framed(&mut self.client, &bytes)).await {
                Ok(res) => res,
                Err(_) => Err(IpcError::Timeout {
                    duration_ms: timeout_duration.as_millis() as u64,
                }),
            }
        }

        async fn receive_envelope(
            &mut self,
            timeout_duration: Duration,
        ) -> Result<IpcEnvelope, IpcError> {
            match timeout(timeout_duration, read_framed(&mut self.client)).await {
                Ok(Ok(bytes)) => IpcEnvelope::from_bytes(&bytes)
                    .map_err(|e| IpcError::ProtocolError(e.to_string())),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(IpcError::Timeout {
                    duration_ms: timeout_duration.as_millis() as u64,
                }),
            }
        }

        async fn close(&mut self) -> Result<(), IpcError> {
            Ok(())
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use jarvis_protocol::IpcMessageType;

    #[tokio::test]
    async fn test_memory_transport_roundtrip() {
        let (mut client, mut server) = MemoryTransport::create_pair(16);

        let env = IpcEnvelope::new(
            IpcMessageType::Command,
            r#"{"text":"open chrome"}"#.to_string(),
            "req_001".to_string(),
            None,
            None,
        );

        // Client sends
        client
            .send_envelope(&env, Duration::from_secs(1))
            .await
            .unwrap();

        // Server receives
        let received = server
            .receive_envelope(Duration::from_secs(1))
            .await
            .unwrap();

        assert_eq!(received.request_id, "req_001");
        assert_eq!(received.payload_json, r#"{"text":"open chrome"}"#);
    }

    #[tokio::test]
    async fn test_memory_transport_timeout() {
        let (_client, mut server) = MemoryTransport::create_pair(16);

        // Server tries to receive with immediate 50ms timeout (nothing sent)
        let res = server.receive_envelope(Duration::from_millis(50)).await;
        assert!(matches!(res, Err(IpcError::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_framed_io_roundtrip() {
        let mut buffer = Vec::new();
        let payload = b"JARVIS IPC PAYLOAD";

        write_framed(&mut buffer, payload).await.unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let read_payload = read_framed(&mut cursor).await.unwrap();

        assert_eq!(read_payload, payload);
    }
}
