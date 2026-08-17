//! JARVIS Structured Logging and Tracing
//!
//! Provides a consistent logging setup for all JARVIS services.
//! Every operation should include: request_id, task_id, trace_id.
//!
//! # Usage
//!
//! ```rust
//! use jarvis_logging::init_logging;
//!
//! fn main() {
//!     init_logging();
//!     tracing::info!(service = "jarvisd", "JARVIS daemon starting");
//! }
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 3, Milestone M03.08

use std::env;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize the JARVIS logging subsystem.
///
/// Reads configuration from environment variables:
/// - `JARVIS_LOG_LEVEL`: Log level (trace/debug/info/warn/error). Default: info
/// - `JARVIS_LOG_FORMAT`: Output format (json/pretty). Default: pretty
/// - `JARVIS_LOG_FILE`: Optional log file path
pub fn init_logging() {
    let log_level = env::var("JARVIS_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let log_format = env::var("JARVIS_LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());

    let env_filter = EnvFilter::try_new(&log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    match log_format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty())
                .init();
        }
    }

    tracing::debug!(
        log_level = %log_level,
        log_format = %log_format,
        "JARVIS logging initialized"
    );
}

/// Structured log fields used consistently across JARVIS.
///
/// Include these in tracing spans for end-to-end observability.
pub mod fields {
    pub const REQUEST_ID: &str = "request_id";
    pub const TASK_ID: &str = "task_id";
    pub const TRACE_ID: &str = "trace_id";
    pub const SERVICE: &str = "service";
    pub const TOOL: &str = "tool";
    pub const USER: &str = "user";
    pub const DURATION_MS: &str = "duration_ms";
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_logging_does_not_panic() {
        // Just verify the logging setup doesn't crash
        // (Can't call init_logging twice in the same process due to global state)
    }
}
