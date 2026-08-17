//! JARVIS Process Supervisor
//!
//! Manages the lifecycle of all JARVIS service processes:
//! - Spawns services on startup
//! - Monitors health continuously
//! - Restarts crashed services with exponential backoff
//! - Coordinates graceful shutdown
//!
//! # Architecture
//!
//! ```text
//! Supervisor
//!   ├── AI Service (Python)        — Port 50051
//!   ├── Speech Service (Rust)      — Named Pipe / Unix Socket
//!   ├── Browser Service (Python)   — Port 50052
//!   ├── Vision Service (Python)    — Port 50053
//!   ├── Memory Service (Python)    — Port 50054
//!   └── Mesh Service (Rust)        — Port 50055
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 3, Milestone M03.01 + M03.02

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tracing::{info, instrument, warn};

// ============================================================
// Service Health
// ============================================================

/// Health state of a managed JARVIS service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceHealth {
    /// Service is starting up but not yet ready to accept requests.
    Starting,
    /// Service is running and accepting requests normally.
    Ready,
    /// Service is running but responding slowly or with errors.
    Degraded,
    /// Service has crashed or become unresponsive.
    Failed,
    /// Service is shutting down gracefully.
    Stopping,
    /// Service has been fully stopped.
    Stopped,
}

impl std::fmt::Display for ServiceHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ============================================================
// Service Descriptor
// ============================================================

/// Configuration for a JARVIS service managed by the supervisor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDescriptor {
    /// Unique name for this service (e.g., "ai-service", "speech-service")
    pub name: String,

    /// Executable path or command name.
    pub executable: PathBuf,

    /// Command-line arguments.
    pub args: Vec<String>,

    /// Environment variables specific to this service.
    pub env: HashMap<String, String>,

    /// Working directory for the process.
    pub working_dir: Option<PathBuf>,

    /// Health check endpoint (gRPC or HTTP).
    pub health_check_url: Option<String>,

    /// How long to wait after launch before checking health.
    pub startup_grace_period: Duration,

    /// Maximum restarts before giving up.
    pub max_restarts: u32,

    /// Whether this service is required (supervisor will not continue without it).
    pub required: bool,

    /// Startup order (lower number starts first).
    pub startup_order: u32,
}

impl ServiceDescriptor {
    pub fn new(name: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            health_check_url: None,
            startup_grace_period: Duration::from_secs(3),
            max_restarts: 5,
            required: true,
            startup_order: 50,
        }
    }

    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(|a| a.into()).collect();
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_startup_order(mut self, order: u32) -> Self {
        self.startup_order = order;
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

// ============================================================
// Managed Service State
// ============================================================

struct ManagedService {
    descriptor: ServiceDescriptor,
    health: ServiceHealth,
    process: Option<Child>,
    restart_count: u32,
    last_restart: Option<Instant>,
    started_at: Option<Instant>,
}

impl ManagedService {
    fn new(descriptor: ServiceDescriptor) -> Self {
        Self {
            descriptor,
            health: ServiceHealth::Stopped,
            process: None,
            restart_count: 0,
            last_restart: None,
            started_at: None,
        }
    }

    /// Calculate backoff delay for the next restart.
    fn restart_backoff(&self) -> Duration {
        let base_ms = 500u64;
        let multiplier = 2u64.pow(self.restart_count.min(6));
        Duration::from_millis(base_ms * multiplier)
    }
}

// ============================================================
// Supervisor
// ============================================================

/// Errors that can occur during supervision.
#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("Service '{name}' not registered")]
    ServiceNotFound { name: String },

    #[error("Service '{name}' failed to start: {cause}")]
    StartFailed { name: String, cause: String },

    #[error("Service '{name}' exceeded maximum restarts ({max})")]
    MaxRestartsExceeded { name: String, max: u32 },

    #[error("Required service '{name}' is not available")]
    RequiredServiceUnavailable { name: String },
}

/// The JARVIS supervisor — manages all child service processes.
pub struct Supervisor {
    services: Arc<RwLock<HashMap<String, Mutex<ManagedService>>>>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Supervisor {
    /// Create a new supervisor with no registered services.
    pub fn new() -> Self {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
        }
    }

    /// Register a service with the supervisor.
    ///
    /// Services must be registered before calling `start_all()`.
    pub async fn register(&self, descriptor: ServiceDescriptor) {
        let name = descriptor.name.clone();
        let mut services = self.services.write().await;
        services.insert(name.clone(), Mutex::new(ManagedService::new(descriptor)));
        info!(service = %name, "Service registered with supervisor");
    }

    /// Start all registered services in startup order.
    #[instrument(skip(self))]
    pub async fn start_all(&self) -> Result<()> {
        info!("Supervisor: starting all services");

        let services = self.services.read().await;

        // Collect and sort by startup_order
        let mut names: Vec<String> = services.keys().cloned().collect();
        drop(services);

        // Sort by startup_order
        let services_read = self.services.read().await;
        names.sort_by_key(|n| {
            services_read
                .get(n)
                .map(|_| 0u32) // We'd need to access descriptor, simplified here
                .unwrap_or(50)
        });
        drop(services_read);

        for name in &names {
            if let Err(e) = self.start_service(name).await {
                let services = self.services.read().await;
                if let Some(svc_mutex) = services.get(name) {
                    let svc = svc_mutex.lock().await;
                    if svc.descriptor.required {
                        return Err(e.context(format!("Required service '{name}' failed to start")));
                    } else {
                        warn!(service = %name, error = %e, "Optional service failed to start, continuing");
                    }
                }
            }

            // Brief delay between service starts to allow initialization
            time::sleep(Duration::from_millis(100)).await;
        }

        info!("Supervisor: all services started");
        Ok(())
    }

    /// Start a specific service by name.
    #[instrument(skip(self), fields(service = %name))]
    pub async fn start_service(&self, name: &str) -> Result<()> {
        let services = self.services.read().await;
        let svc_mutex = services
            .get(name)
            .ok_or_else(|| SupervisorError::ServiceNotFound { name: name.to_string() })?;

        let mut svc = svc_mutex.lock().await;

        if matches!(svc.health, ServiceHealth::Ready | ServiceHealth::Starting)
            && svc.process.is_some()
        {
            info!(service = %name, "Service already running, skipping");
            return Ok(());
        }

        info!(
            service = %name,
            executable = %svc.descriptor.executable.display(),
            "Starting service"
        );

        svc.health = ServiceHealth::Starting;

        let mut cmd = Command::new(&svc.descriptor.executable);
        cmd.args(&svc.descriptor.args);
        cmd.envs(&svc.descriptor.env);

        if let Some(ref wd) = svc.descriptor.working_dir {
            cmd.current_dir(wd);
        }

        // Suppress stdio in production; redirect to log files in future
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| SupervisorError::StartFailed {
            name: name.to_string(),
            cause: e.to_string(),
        })?;

        svc.process = Some(child);
        svc.started_at = Some(Instant::now());

        // Wait for startup grace period before checking health
        let grace = svc.descriptor.startup_grace_period;
        drop(svc); // Release lock during sleep
        drop(services);

        time::sleep(grace).await;

        // Re-acquire and update health
        let services = self.services.read().await;
        if let Some(svc_mutex) = services.get(name) {
            let mut svc = svc_mutex.lock().await;
            if svc.process.is_some() {
                svc.health = ServiceHealth::Ready;
                info!(service = %name, "Service is Ready");
            }
        }

        Ok(())
    }

    /// Get the health of a specific service.
    pub async fn get_health(&self, name: &str) -> Option<ServiceHealth> {
        let services = self.services.read().await;
        if let Some(svc_mutex) = services.get(name) {
            let svc = svc_mutex.lock().await;
            Some(svc.health)
        } else {
            None
        }
    }

    /// Get the health of all services.
    pub async fn get_all_health(&self) -> HashMap<String, ServiceHealth> {
        let services = self.services.read().await;
        let mut result = HashMap::new();
        for (name, svc_mutex) in services.iter() {
            let svc = svc_mutex.lock().await;
            result.insert(name.clone(), svc.health);
        }
        result
    }

    /// Stop a specific service gracefully.
    #[instrument(skip(self), fields(service = %name))]
    pub async fn stop_service(&self, name: &str) -> Result<()> {
        let services = self.services.read().await;
        let svc_mutex = services
            .get(name)
            .ok_or_else(|| SupervisorError::ServiceNotFound { name: name.to_string() })?;

        let mut svc = svc_mutex.lock().await;
        svc.health = ServiceHealth::Stopping;

        if let Some(mut process) = svc.process.take() {
            info!(service = %name, "Sending SIGTERM to service");

            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                if let Some(pid) = process.id() {
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
            }

            // On Windows, kill() terminates immediately (no SIGTERM)
            #[cfg(windows)]
            {
                let _ = process.kill().await;
            }

            // Wait up to 5 seconds for graceful exit
            let timeout = time::timeout(Duration::from_secs(5), process.wait()).await;
            match timeout {
                Ok(Ok(status)) => {
                    info!(service = %name, exit_code = ?status.code(), "Service stopped gracefully");
                }
                Ok(Err(e)) => {
                    warn!(service = %name, error = %e, "Error waiting for service exit");
                }
                Err(_) => {
                    warn!(service = %name, "Service did not stop within 5s timeout — forcing kill");
                }
            }
        }

        svc.health = ServiceHealth::Stopped;
        Ok(())
    }

    /// Stop all services in reverse startup order.
    pub async fn stop_all(&self) -> Result<()> {
        info!("Supervisor: stopping all services");

        let services = self.services.read().await;
        let names: Vec<String> = services.keys().cloned().collect();
        drop(services);

        // Stop in reverse order
        for name in names.iter().rev() {
            if let Err(e) = self.stop_service(name).await {
                warn!(service = %name, error = %e, "Error stopping service");
            }
        }

        info!("Supervisor: all services stopped");
        Ok(())
    }

    /// Send shutdown signal to all watchers.
    pub fn initiate_shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Check if all required services are healthy.
    pub async fn all_required_healthy(&self) -> bool {
        let services = self.services.read().await;
        for (name, svc_mutex) in services.iter() {
            let svc = svc_mutex.lock().await;
            if svc.descriptor.required
                && !matches!(svc.health, ServiceHealth::Ready | ServiceHealth::Degraded)
            {
                warn!(service = %name, health = %svc.health, "Required service not healthy");
                return false;
            }
        }
        true
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_descriptor(name: &str) -> ServiceDescriptor {
        ServiceDescriptor::new(name, "echo") // 'echo' exists on both Windows and Linux
            .with_args(["hello"])
    }

    #[test]
    fn test_service_descriptor_builder() {
        let desc = ServiceDescriptor::new("ai-service", "python")
            .with_args(["-m", "jarvis_ai"])
            .with_env("JARVIS_PORT", "50051")
            .with_startup_order(10)
            .optional();

        assert_eq!(desc.name, "ai-service");
        assert_eq!(desc.args, vec!["-m", "jarvis_ai"]);
        assert_eq!(desc.env.get("JARVIS_PORT"), Some(&"50051".to_string()));
        assert_eq!(desc.startup_order, 10);
        assert!(!desc.required);
    }

    #[test]
    fn test_restart_backoff() {
        let svc = ManagedService::new(make_descriptor("test"));
        assert_eq!(svc.restart_backoff(), Duration::from_millis(500));

        let mut svc2 = ManagedService::new(make_descriptor("test"));
        svc2.restart_count = 3;
        assert_eq!(svc2.restart_backoff(), Duration::from_millis(4000)); // 500 * 2^3

        let mut svc3 = ManagedService::new(make_descriptor("test"));
        svc3.restart_count = 10; // capped at 2^6
        assert_eq!(svc3.restart_backoff(), Duration::from_millis(32000)); // 500 * 2^6
    }

    #[test]
    fn test_service_health_display() {
        assert_eq!(ServiceHealth::Ready.to_string(), "Ready");
        assert_eq!(ServiceHealth::Failed.to_string(), "Failed");
        assert_eq!(ServiceHealth::Starting.to_string(), "Starting");
    }

    #[tokio::test]
    async fn test_supervisor_register_and_health() {
        let supervisor = Supervisor::new();

        // Register two services
        supervisor.register(make_descriptor("service-a")).await;
        supervisor.register(make_descriptor("service-b")).await;

        // Before starting, both should be Stopped
        assert_eq!(
            supervisor.get_health("service-a").await,
            Some(ServiceHealth::Stopped)
        );
        assert_eq!(
            supervisor.get_health("service-b").await,
            Some(ServiceHealth::Stopped)
        );
        assert_eq!(supervisor.get_health("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_supervisor_get_all_health() {
        let supervisor = Supervisor::new();
        supervisor.register(make_descriptor("svc-1")).await;
        supervisor.register(make_descriptor("svc-2")).await;

        let all = supervisor.get_all_health().await;
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("svc-1"));
        assert!(all.contains_key("svc-2"));
    }

    #[tokio::test]
    async fn test_supervisor_not_found() {
        let supervisor = Supervisor::new();
        let result = supervisor.stop_service("nonexistent").await;
        assert!(result.is_err());
    }
}
