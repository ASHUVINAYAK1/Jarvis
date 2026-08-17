//! JARVIS Desktop Tauri Backend
//!
//! Exposes Tauri IPC commands and integrates real-time local voice pipeline,
//! continuous audio capture, event bus, and JARVIS Core Orchestrator.

use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use tracing::info;

use jarvis_event_bus::{EventBus, JarvisEvent, VoiceEvent};
use jarvis_logging::init_logging;
use jarvis_orchestrator::{ExecutionOutcome, Orchestrator};
use jarvis_platform::{PlatformAdapter, PlatformInfo};
use jarvis_speech::{AudioCapture, VoiceSessionController};
use jarvis_windows::WindowsPlatformAdapter;

/// Managed state holding the JARVIS core orchestrator & voice controller
pub struct JarvisState {
    pub orchestrator: Arc<Orchestrator>,
    pub platform: Arc<dyn PlatformAdapter>,
    pub event_bus: Arc<EventBus>,
    pub voice_controller: Arc<VoiceSessionController>,
    pub audio_capture: Arc<AudioCapture>,
}

#[tauri::command]
async fn execute_command(
    command: String,
    state: State<'_, Arc<Mutex<JarvisState>>>,
) -> Result<ExecutionOutcome, String> {
    let guard = state.lock().await;
    let outcome = guard.orchestrator.execute_command(&command).await;
    Ok(outcome)
}

#[tauri::command]
async fn trigger_wake_word(state: State<'_, Arc<Mutex<JarvisState>>>) -> Result<String, String> {
    let guard = state.lock().await;
    guard
        .voice_controller
        .trigger_wake_word()
        .await
        .map_err(|e| e.to_string())?;
    Ok("WAKE_DETECTED".to_string())
}

#[tauri::command]
async fn get_platform_info(
    state: State<'_, Arc<Mutex<JarvisState>>>,
) -> Result<PlatformInfo, String> {
    let guard = state.lock().await;
    guard
        .platform
        .get_platform_info()
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    info!("Initializing JARVIS Desktop Core Subsystems...");

    let platform = Arc::new(WindowsPlatformAdapter::new());
    let orchestrator = Arc::new(Orchestrator::new(platform.clone()));
    let event_bus = Arc::new(EventBus::new(256));
    let voice_controller = Arc::new(VoiceSessionController::new(event_bus.clone()));
    let audio_capture = Arc::new(AudioCapture::default_16k_mono());

    let jarvis_state = Arc::new(Mutex::new(JarvisState {
        orchestrator,
        platform,
        event_bus: event_bus.clone(),
        voice_controller: voice_controller.clone(),
        audio_capture: audio_capture.clone(),
    }));

    // Start background microphone audio stream listening task using Tauri async runtime
    let capture_ref = audio_capture.clone();
    let voice_ref = voice_controller.clone();
    tauri::async_runtime::spawn(async move {
        info!("Starting continuous background audio capture loop...");
        if let Ok(mut rx) = capture_ref.start_capture() {
            while let Some(chunk) = rx.recv().await {
                voice_ref.process_audio_chunk(chunk).await;
            }
        }
    });

    let bus_ref = event_bus.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(jarvis_state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let bus = bus_ref.clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = bus.subscribe();
                while let Ok(event) = rx.recv().await {
                    if let JarvisEvent::Voice(v) = event {
                        match v {
                            VoiceEvent::WakeWordDetected { .. } => {
                                let _ = handle.emit("jarvis-voice-state", "WAKE_DETECTED");
                            }
                            VoiceEvent::SpeechStarted => {
                                let _ = handle.emit("jarvis-voice-state", "LISTENING");
                            }
                            VoiceEvent::Transcribed { text, .. } => {
                                let _ = handle.emit("jarvis-transcribed", text);
                            }
                            VoiceEvent::SynthesisStarted { text } => {
                                let _ = handle.emit("jarvis-speaking", text);
                            }
                            VoiceEvent::SynthesisCompleted { .. } => {
                                let _ = handle.emit("jarvis-voice-state", "SUCCESS");
                            }
                            _ => {}
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            execute_command,
            trigger_wake_word,
            get_platform_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running JARVIS desktop application");
}
