//! JARVIS CLI Entry Point
//!
//! Invokes the real JARVIS core runtime from the command line.
//!
//! Usage:
//! ```bash
//! jarvis "open chrome"
//! jarvis "what time is it"
//! ```

use std::env;
use std::sync::Arc;

use jarvis_logging::init_logging;
use jarvis_orchestrator::{ExecutionOutcome, Orchestrator};
use jarvis_windows::WindowsPlatformAdapter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    // Get command from CLI arguments
    let args: Vec<String> = env::args().collect();
    let command = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "open chrome".to_string()
    };

    println!();
    println!("  ========================================");
    println!("  J.A.R.V.I.S. — Core Execution Pipeline");
    println!("  ========================================");
    println!("  Command: \"{}\"", command);
    println!();

    // Instantiate real platform adapter (Windows)
    let adapter = Arc::new(WindowsPlatformAdapter::new());

    // Instantiate orchestrator
    let orchestrator = Orchestrator::new(adapter);

    // Subscribe to events for live console telemetry
    let mut event_sub = orchestrator.event_bus().subscribe();

    // Spawn event listener
    let listener = tokio::spawn(async move {
        while let Ok(event) = event_sub.recv().await {
            match event {
                jarvis_event_bus::JarvisEvent::Task(t) => {
                    println!("  [TASK EVENT] {:?}", t);
                }
                jarvis_event_bus::JarvisEvent::Tool(t) => {
                    println!("  [TOOL EVENT] {:?}", t);
                }
                _ => {}
            }
        }
    });

    // Execute command through the complete architecture
    let outcome = orchestrator.execute_command(&command).await;

    // Small yield for event printing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    listener.abort();

    println!();
    println!("  ----------------------------------------");
    match outcome {
        ExecutionOutcome::Success {
            spoken_response,
            duration_ms,
            tool_name,
            tool_data,
            ..
        } => {
            println!("  STATUS: SUCCESS ({}ms)", duration_ms);
            println!("  TOOL:   {}", tool_name);
            println!("  DATA:   {}", tool_data);
            println!();
            println!("  JARVIS: \"{}\"", spoken_response);
        }
        ExecutionOutcome::ApprovalRequired {
            reason, tool_name, ..
        } => {
            println!("  STATUS: APPROVAL REQUIRED");
            println!("  TOOL:   {}", tool_name);
            println!("  REASON: {}", reason);
        }
        ExecutionOutcome::Denied { reason, .. } => {
            println!("  STATUS: POLICY DENIED");
            println!("  REASON: {}", reason);
        }
        ExecutionOutcome::Failed { error, .. } => {
            println!("  STATUS: FAILED");
            println!("  ERROR:  {}", error);
        }
    }
    println!("  ========================================");
    println!();

    Ok(())
}
