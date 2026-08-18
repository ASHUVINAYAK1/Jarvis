//! Voice Session State Controller & Audio Pipeline Orchestration

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};

use jarvis_ai::ModelGateway;
use jarvis_event_bus::{EventBus, JarvisEvent, VoiceEvent};

use crate::output::AudioOutput;
use crate::stt::{SpeechToText, WhisperSttEngine};
use crate::tts::{MockTtsEngine, TextToSpeech};
use crate::types::{AudioChunk, AudioFormat, SpeechError, VoiceSessionState};
use crate::vad::{VadState, VoiceActivityDetector};
use crate::wakeword::WakeWordDetector;

/// Central controller managing the state machine of a voice interaction session.
pub struct VoiceSessionController {
    state: Arc<RwLock<VoiceSessionState>>,
    event_bus: Arc<EventBus>,
    ai_gateway: Option<Arc<ModelGateway>>,
    orchestrator: Option<Arc<jarvis_orchestrator::Orchestrator>>,
    stt_engine: Arc<dyn SpeechToText>,
    tts_engine: Arc<dyn TextToSpeech>,
    audio_output: Arc<AudioOutput>,
    wake_detector: Arc<RwLock<WakeWordDetector>>,
    vad: Arc<RwLock<VoiceActivityDetector>>,
    speech_buffer: Arc<RwLock<Vec<f32>>>,
    speech_format: Arc<RwLock<AudioFormat>>,
    is_running: Arc<AtomicBool>,
    playback_lock: Arc<tokio::sync::Mutex<()>>,
}

impl VoiceSessionController {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            state: Arc::new(RwLock::new(VoiceSessionState::Idle)),
            event_bus,
            ai_gateway: None,
            orchestrator: None,
            stt_engine: Arc::new(WhisperSttEngine::default()),
            tts_engine: Arc::new(MockTtsEngine),
            audio_output: Arc::new(AudioOutput::new()),
            wake_detector: Arc::new(RwLock::new(WakeWordDetector::default_jarvis())),
            vad: Arc::new(RwLock::new(VoiceActivityDetector::new())),
            speech_buffer: Arc::new(RwLock::new(Vec::new())),
            speech_format: Arc::new(RwLock::new(AudioFormat::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            playback_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn with_ai_gateway(mut self, gateway: Arc<ModelGateway>) -> Self {
        self.ai_gateway = Some(gateway);
        self
    }

    pub fn with_orchestrator(mut self, orchestrator: Arc<jarvis_orchestrator::Orchestrator>) -> Self {
        self.orchestrator = Some(orchestrator);
        self
    }

    pub fn with_stt(mut self, stt: Arc<dyn SpeechToText>) -> Self {
        self.stt_engine = stt;
        self
    }

    pub fn with_tts(mut self, tts: Arc<dyn TextToSpeech>) -> Self {
        self.tts_engine = tts;
        self
    }

    pub fn with_audio_output(mut self, output: Arc<AudioOutput>) -> Self {
        self.audio_output = output;
        self
    }

    /// Read current session state.
    pub async fn current_state(&self) -> VoiceSessionState {
        *self.state.read().await
    }

    /// Trigger state transition and publish event to EventBus.
    #[instrument(skip(self), fields(state = %new_state))]
    async fn transition_state(&self, new_state: VoiceSessionState) {
        let mut guard = self.state.write().await;
        *guard = new_state;
        info!(new_state = %new_state, "VoiceSessionState transition");
    }

    /// Process an incoming live PCM audio chunk from capture stream.
    pub async fn process_audio_chunk(&self, chunk: AudioChunk) {
        let current = self.current_state().await;

        if current == VoiceSessionState::Idle {
            let mut wake = self.wake_detector.write().await;
            let wake_res = wake.detect(&chunk);
            if wake_res.detected {
                drop(wake);
                let _ = self.trigger_wake_word().await;
            }
        } else if current == VoiceSessionState::Listening {
            // Store audio format and accumulate PCM samples in buffer
            {
                let mut fmt = self.speech_format.write().await;
                *fmt = chunk.format;
                let mut buf = self.speech_buffer.write().await;
                buf.extend_from_slice(&chunk.samples);
            }

            let mut vad = self.vad.write().await;
            let vad_state = vad.process_chunk(&chunk);

            if vad_state == VadState::SpeechStarted {
                let _ = self
                    .event_bus
                    .publish(JarvisEvent::Voice(VoiceEvent::SpeechStarted))
                    .await;
            } else if vad_state == VadState::SpeechEnded {
                vad.reset();
                drop(vad);

                // Drain complete accumulated speech buffer
                let (collected_samples, format) = {
                    let mut buf = self.speech_buffer.write().await;
                    let samples = buf.drain(..).collect::<Vec<f32>>();
                    let fmt = *self.speech_format.read().await;
                    (samples, fmt)
                };

                let _ = self
                    .event_bus
                    .publish(JarvisEvent::Voice(VoiceEvent::SpeechEnded))
                    .await;

                if !collected_samples.is_empty() {
                    let full_utterance = AudioChunk {
                        samples: collected_samples,
                        format,
                        timestamp_ms: chunk.timestamp_ms,
                    };
                    let _ = self.process_speech_utterance(full_utterance).await;
                }
            }
        }
    }

    /// Trigger software wake word activation.
    pub async fn trigger_wake_word(&self) -> Result<(), SpeechError> {
        // Clear speech buffer when starting listening session
        {
            let mut buf = self.speech_buffer.write().await;
            buf.clear();
        }

        self.transition_state(VoiceSessionState::WakeDetected).await;
        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::WakeWordDetected {
                confidence: 0.98,
            }))
            .await;

        self.transition_state(VoiceSessionState::Listening).await;
        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::SpeechStarted))
            .await;

        Ok(())
    }

    /// Trigger user barge-in interruption.
    pub async fn interrupt(&self) {
        self.audio_output.stop();
        self.transition_state(VoiceSessionState::Interrupted).await;
        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::SynthesisInterrupted))
            .await;
        self.transition_state(VoiceSessionState::Listening).await;
    }

    /// Process a completed audio speech segment through STT → AI Gateway / Orchestrator → TTS → Speaker Output.
    #[instrument(skip(self, speech_chunk))]
    pub async fn process_speech_utterance(
        &self,
        speech_chunk: AudioChunk,
    ) -> Result<String, SpeechError> {
        let utterance_id = uuid::Uuid::new_v4().to_string();

        // 1. Transition to Transcribing
        self.transition_state(VoiceSessionState::Transcribing).await;

        let stt_res = match self.stt_engine.transcribe(&speech_chunk).await {
            Ok(res) => res,
            Err(e) => {
                info!(
                    utterance_id = %utterance_id,
                    error = %e,
                    "[STT ISOLATION] STT transcription failed or empty — zero command execution"
                );
                self.reset_to_idle().await;
                return Err(e);
            }
        };

        let command_text = stt_res.text.trim();
        if command_text.is_empty() {
            info!(
                utterance_id = %utterance_id,
                "[STT ISOLATION] STT returned empty transcript — zero command execution"
            );
            self.reset_to_idle().await;
            return Ok(String::new());
        }

        info!(
            utterance_id = %utterance_id,
            command = %command_text,
            "[STT ISOLATION] Valid speech transcribed from microphone"
        );

        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::Transcribed {
                text: command_text.to_string(),
                confidence: stt_res.confidence,
                language: stt_res.language.clone(),
            }))
            .await;

        // 2. Transition to Thinking / Reasoning
        self.transition_state(VoiceSessionState::Thinking).await;

        let response_text = if let Some(orch) = &self.orchestrator {
            info!(
                utterance_id = %utterance_id,
                command = %command_text,
                "[ORCHESTRATOR ISOLATION] Invoking Orchestrator for transcribed command"
            );
            let outcome = orch.execute_command(command_text).await;
            let (tool_name, resp) = match outcome {
                jarvis_orchestrator::ExecutionOutcome::Success { spoken_response, tool_name, .. } => (tool_name, spoken_response),
                jarvis_orchestrator::ExecutionOutcome::ApprovalRequired { reason, .. } => ("approval_required".to_string(), reason),
                jarvis_orchestrator::ExecutionOutcome::Denied { reason, .. } => ("policy_denied".to_string(), format!("Policy denied: {}", reason)),
                jarvis_orchestrator::ExecutionOutcome::Failed { error, .. } => ("execution_failed".to_string(), format!("Execution failed: {}", error)),
            };
            info!(
                utterance_id = %utterance_id,
                tool_name = %tool_name,
                spoken_response = %resp,
                "[ORCHESTRATOR ISOLATION] Orchestrator completed command execution"
            );
            resp
        } else if let Some(ai) = &self.ai_gateway {
            ai.ask(command_text)
                .await
                .unwrap_or_else(|e| format!("I apologize, sir. I encountered an error: {}", e))
        } else {
            info!(
                utterance_id = %utterance_id,
                command = %command_text,
                "[STT ISOLATION] Speech transcribed but no execution engine attached"
            );
            self.reset_to_idle().await;
            return Ok(command_text.to_string());
        };

        self.speak_text(&response_text).await?;
        Ok(response_text)
    }

    /// Reset session state back to Idle for continuous hands-free wake word monitoring.
    pub async fn reset_to_idle(&self) {
        {
            let mut buf = self.speech_buffer.write().await;
            buf.clear();
        }
        self.transition_state(VoiceSessionState::Idle).await;
    }

    /// Synthesize and play spoken response text via Piper TTS and CPAL AudioOutput.
    pub async fn speak_text(&self, text: &str) -> Result<(), SpeechError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.reset_to_idle().await;
            return Ok(());
        }

        // 1. Interrupt any currently active playback (barge-in / sequential replacement)
        self.audio_output.stop();

        // 2. Acquire playback lock to ensure strictly serialized TTS synthesis & audio playback
        let _guard = self.playback_lock.lock().await;

        info!(text = %trimmed, "[TTS RESPONSE] Synthesizing user-facing response");

        self.transition_state(VoiceSessionState::Speaking).await;
        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::SynthesisStarted {
                text: trimmed.to_string(),
            }))
            .await;

        let synth_res = self.tts_engine.synthesize(trimmed).await;
        let synth = match synth_res {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Piper TTS synthesis failed");
                self.reset_to_idle().await;
                return Err(e);
            }
        };

        let play_dur = synth.duration_ms;
        let play_res = self.audio_output.play(synth).await;

        match play_res {
            Ok(_) => {
                self.event_bus
                    .publish(JarvisEvent::Voice(VoiceEvent::SynthesisCompleted {
                        duration_ms: play_dur,
                    }))
                    .await;
            }
            Err(SpeechError::Interrupted) => {
                info!("TTS audio playback interrupted by new user response or barge-in");
            }
            Err(e) => {
                info!(error = %e, "Audio playback completed with error");
            }
        }

        self.reset_to_idle().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_voice_session_reset_to_idle() {
        let bus = Arc::new(EventBus::new(32));
        let controller = VoiceSessionController::new(bus);

        controller.trigger_wake_word().await.unwrap();
        assert_eq!(controller.current_state().await, VoiceSessionState::Listening);

        controller.reset_to_idle().await;
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_voice_session_speak_text_and_return_to_idle() {
        let bus = Arc::new(EventBus::new(32));
        let controller = VoiceSessionController::new(bus);

        controller.speak_text("Chrome is open, sir.").await.unwrap();
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_voice_session_no_fake_processing_text_generated() {
        let bus = Arc::new(EventBus::new(32));
        let mock_stt = Arc::new(crate::stt::MockSttEngine::new("open notepad"));
        let controller = VoiceSessionController::new(bus).with_stt(mock_stt);

        let chunk = AudioChunk {
            samples: vec![0.0; 160],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        // When process_speech_utterance runs without orchestrator/AI, it must NOT generate "Processing query: ..."
        let res = controller.process_speech_utterance(chunk).await.unwrap();
        assert!(!res.contains("Processing query:"));
        assert!(!res.contains("Systems nominal"));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_voice_session_sequential_speak_text_serialized() {
        let bus = Arc::new(EventBus::new(32));
        let controller = Arc::new(VoiceSessionController::new(bus));

        let c1 = controller.clone();
        let handle1 = tokio::spawn(async move {
            c1.speak_text("Chrome is open, sir.").await.unwrap();
        });

        let c2 = controller.clone();
        let handle2 = tokio::spawn(async move {
            c2.speak_text("Spotify is open, sir.").await.unwrap();
        });

        handle1.await.unwrap();
        handle2.await.unwrap();

        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_empty_stt_transcript_zero_command_execution() {
        let bus = Arc::new(EventBus::new(32));
        let mock_stt = Arc::new(crate::stt::MockSttEngine::new(""));
        let controller = VoiceSessionController::new(bus).with_stt(mock_stt);

        let chunk = AudioChunk {
            samples: vec![0.0; 160],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        let res = controller.process_speech_utterance(chunk).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "");
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    struct TestMockPlatformAdapter;

    #[async_trait::async_trait]
    impl jarvis_platform::PlatformAdapter for TestMockPlatformAdapter {
        async fn get_platform_info(&self) -> anyhow::Result<jarvis_platform::PlatformInfo> {
            unimplemented!()
        }
        async fn open_application(
            &self,
            app: &str,
            _options: Option<jarvis_platform::LaunchOptions>,
        ) -> anyhow::Result<jarvis_platform::ProcessInfo> {
            Ok(jarvis_platform::ProcessInfo {
                pid: 1234,
                name: app.to_string(),
                executable_path: None,
                command_line: None,
                running: true,
            })
        }
        async fn close_application(&self, _app: &str) -> anyhow::Result<()> { Ok(()) }
        async fn list_processes(&self) -> anyhow::Result<Vec<jarvis_platform::ProcessInfo>> { Ok(vec![]) }
        async fn is_application_running(&self, _app: &str) -> anyhow::Result<bool> { Ok(true) }
        async fn list_windows(&self) -> anyhow::Result<Vec<jarvis_platform::WindowInfo>> { Ok(vec![]) }
        async fn focus_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn minimize_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn maximize_window(&self, _handle: &str) -> anyhow::Result<()> { Ok(()) }
        async fn set_window_bounds(&self, _handle: &str, _bounds: jarvis_platform::Rect) -> anyhow::Result<()> { Ok(()) }
        async fn take_screenshot(&self) -> anyhow::Result<jarvis_platform::Screenshot> { unimplemented!() }
        async fn take_screenshot_display(&self, _display: u32) -> anyhow::Result<jarvis_platform::Screenshot> { unimplemented!() }
        async fn take_screenshot_region(&self, _rect: jarvis_platform::Rect) -> anyhow::Result<jarvis_platform::Screenshot> { unimplemented!() }
        async fn get_clipboard(&self) -> anyhow::Result<jarvis_platform::ClipboardContent> { Ok(jarvis_platform::ClipboardContent::Empty) }
        async fn set_clipboard(&self, _content: jarvis_platform::ClipboardContent) -> anyhow::Result<()> { Ok(()) }
        async fn show_notification(&self, _notification: jarvis_platform::NotificationRequest) -> anyhow::Result<()> { Ok(()) }
        async fn get_disk_space(&self) -> anyhow::Result<jarvis_platform::DiskInfo> { unimplemented!() }
        async fn get_memory_info(&self) -> anyhow::Result<jarvis_platform::MemoryInfo> { unimplemented!() }
    }

    #[tokio::test]
    async fn test_stt_isolation_orchestrator_integration_notepad_only() {
        use jarvis_orchestrator::Orchestrator;
        use jarvis_platform::PlatformAdapter;

        let bus = Arc::new(EventBus::new(32));
        let mock_stt = Arc::new(crate::stt::MockSttEngine::new("open notepad"));
        let platform: Arc<dyn PlatformAdapter> = Arc::new(TestMockPlatformAdapter);
        let orchestrator = Arc::new(Orchestrator::new(platform));

        let controller = VoiceSessionController::new(bus)
            .with_stt(mock_stt)
            .with_orchestrator(orchestrator);

        let chunk = AudioChunk {
            samples: vec![0.0; 160],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        let res = controller.process_speech_utterance(chunk).await.unwrap();
        assert!(res.to_lowercase().contains("notepad"));
        assert!(!res.to_lowercase().contains("chrome"));
        assert!(!res.to_lowercase().contains("spotify"));
        assert!(!res.to_lowercase().contains("time"));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_stt_isolation_orchestrator_integration_chrome_only() {
        use jarvis_orchestrator::Orchestrator;
        use jarvis_platform::PlatformAdapter;

        let bus = Arc::new(EventBus::new(32));
        let mock_stt = Arc::new(crate::stt::MockSttEngine::new("open chrome"));
        let platform: Arc<dyn PlatformAdapter> = Arc::new(TestMockPlatformAdapter);
        let orchestrator = Arc::new(Orchestrator::new(platform));

        let controller = VoiceSessionController::new(bus)
            .with_stt(mock_stt)
            .with_orchestrator(orchestrator);

        let chunk = AudioChunk {
            samples: vec![0.0; 160],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        let res = controller.process_speech_utterance(chunk).await.unwrap();
        assert!(res.to_lowercase().contains("chrome"));
        assert!(!res.to_lowercase().contains("notepad"));
        assert!(!res.to_lowercase().contains("spotify"));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn test_stt_isolation_orchestrator_integration_time_only() {
        use jarvis_orchestrator::Orchestrator;
        use jarvis_platform::PlatformAdapter;

        let bus = Arc::new(EventBus::new(32));
        let mock_stt = Arc::new(crate::stt::MockSttEngine::new("what time is it"));
        let platform: Arc<dyn PlatformAdapter> = Arc::new(TestMockPlatformAdapter);
        let orchestrator = Arc::new(Orchestrator::new(platform));

        let controller = VoiceSessionController::new(bus)
            .with_stt(mock_stt)
            .with_orchestrator(orchestrator);

        let chunk = AudioChunk {
            samples: vec![0.0; 160],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        let res = controller.process_speech_utterance(chunk).await.unwrap();
        assert!(res.to_lowercase().contains("currently") || res.to_lowercase().contains("time"));
        assert!(!res.to_lowercase().contains("chrome"));
        assert!(!res.to_lowercase().contains("notepad"));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }
}
