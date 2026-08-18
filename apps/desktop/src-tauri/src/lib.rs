//! JARVIS Desktop Tauri Backend
//!
//! Exposes Tauri IPC commands and integrates real-time local voice pipeline,
//! continuous audio capture, event bus, and JARVIS Core Orchestrator.

use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;
use tracing::info;

use jarvis_event_bus::{EventBus, JarvisEvent, VoiceEvent};
use jarvis_logging::init_logging;
use jarvis_orchestrator::{ExecutionOutcome, Orchestrator};
use jarvis_platform::{PlatformAdapter, PlatformInfo};
use jarvis_speech::{AudioCapture, AudioOutput, PiperConfig, PiperTtsEngine, TextToSpeech, VoiceSessionController};
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
    let command_text = command.trim();
    if command_text.is_empty() {
        return Err("Command string is empty".to_string());
    }

    let utterance_id = uuid::Uuid::new_v4().to_string();
    info!(
        utterance_id = %utterance_id,
        command = %command_text,
        "[FRONTEND IPC] Desktop IPC command received"
    );

    let guard = state.lock().await;
    let outcome = guard.orchestrator.execute_command(command_text).await;
    let voice_controller = guard.voice_controller.clone();

    if let ExecutionOutcome::Success { ref spoken_response, ref tool_name, .. } = outcome {
        info!(
            utterance_id = %utterance_id,
            tool_name = %tool_name,
            spoken_response = %spoken_response,
            "[FRONTEND IPC] Desktop IPC command executed successfully"
        );
        let spoken = spoken_response.clone();
        tokio::spawn(async move {
            let _ = voice_controller.speak_text(&spoken).await;
        });
    } else {
        let vc = voice_controller.clone();
        tokio::spawn(async move {
            vc.reset_to_idle().await;
        });
    }

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

/// Security Helper: Verifies that a given file path is canonical and strictly resides inside the JARVIS screenshots directory.
pub fn validate_screenshot_path(file_path: &str) -> Result<std::path::PathBuf, String> {
    if file_path.contains("..") || file_path.contains("../") || file_path.contains("..\\") {
        return Err("Path traversal forbidden".to_string());
    }

    let allowed_dir = jarvis_tools::get_jarvis_screenshots_dir();
    let canonical_allowed = allowed_dir
        .canonicalize()
        .unwrap_or_else(|_| allowed_dir.clone());

    let raw_path = std::path::Path::new(file_path);
    let canonical_file = raw_path
        .canonicalize()
        .map_err(|e| format!("Invalid screenshot file path '{}': {}", file_path, e))?;

    if !canonical_file.starts_with(&canonical_allowed) {
        return Err(format!(
            "Security violation: path '{}' is outside the JARVIS screenshot directory",
            file_path
        ));
    }

    Ok(canonical_file)
}

#[tauri::command]
async fn get_screenshot_base64(path: String) -> Result<String, String> {
    let canonical = validate_screenshot_path(&path)?;
    let bytes = tokio::fs::read(&canonical)
        .await
        .map_err(|e| format!("Failed to read screenshot file: {}", e))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
async fn open_screenshot(path: String) -> Result<String, String> {
    let canonical = validate_screenshot_path(&path)?;

    #[cfg(target_os = "windows")]
    {
        tokio::process::Command::new("explorer")
            .arg(&canonical)
            .spawn()
            .map_err(|e| format!("Failed to open image: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        tokio::process::Command::new("xdg-open")
            .arg(&canonical)
            .spawn()
            .map_err(|e| format!("Failed to open image: {}", e))?;
    }

    Ok("SUCCESS".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    info!("Initializing JARVIS Desktop Core Subsystems...");

    let platform = Arc::new(WindowsPlatformAdapter::new());
    let orchestrator = Arc::new(Orchestrator::new(platform.clone()));
    let event_bus = Arc::new(EventBus::new(256));

    let tts_engine: Arc<dyn TextToSpeech> = match PiperConfig::discover() {
        Ok(config) => {
            info!(
                executable = ?config.executable_path,
                model = ?config.model_path,
                "Piper TTS engine configured"
            );
            Arc::new(PiperTtsEngine::with_config(config))
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Piper TTS engine unavailable — returning structured error if synthesis is requested"
            );
            Arc::new(PiperTtsEngine::default())
        }
    };

    let audio_output = Arc::new(AudioOutput::new());
    let voice_controller = Arc::new(
        VoiceSessionController::new(event_bus.clone())
            .with_tts(tts_engine)
            .with_audio_output(audio_output)
            .with_orchestrator(orchestrator.clone()),
    );
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
            // Configure System Tray Menu & Icon
            let toggle_hud = tauri::menu::MenuItemBuilder::with_id("toggle_hud", "Show / Hide JARVIS HUD").build(app)?;
            let pause_assistant = tauri::menu::MenuItemBuilder::with_id("toggle_assistant", "Pause / Resume Assistant").build(app)?;
            let quit = tauri::menu::MenuItemBuilder::with_id("quit", "Quit JARVIS").build(app)?;

            let tray_menu = tauri::menu::MenuBuilder::new(app)
                .items(&[&toggle_hud, &pause_assistant, &quit])
                .build()?;

            let _tray = tauri::tray::TrayIconBuilder::new()
                .menu(&tray_menu)
                .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                    match event.id().as_ref() {
                        "toggle_hud" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let is_vis = window.is_visible().unwrap_or(false);
                                if is_vis {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "toggle_assistant" => {
                            info!("System Tray: Assistant toggle clicked");
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

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
            get_platform_info,
            get_screenshot_base64,
            open_screenshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running JARVIS desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_screenshot_path_security() {
        // Path traversal attempts must be rejected
        assert!(validate_screenshot_path("../etc/passwd").is_err());
        assert!(validate_screenshot_path("..\\Windows\\System32\\cmd.exe").is_err());
        assert!(validate_screenshot_path("C:\\Users\\Admin\\Pictures\\JARVIS\\Screenshots\\..\\secret.txt").is_err());
    }
}
