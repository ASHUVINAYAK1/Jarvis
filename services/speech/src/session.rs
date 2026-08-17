use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use jarvis_ai::ModelGateway;
use jarvis_event_bus::{EventBus, JarvisEvent, VoiceEvent};
use tokio::sync::RwLock;
use tracing::{info, instrument};

use crate::output::AudioOutput;
use crate::stt::{SpeechToText, WhisperSttEngine};
use crate::tts::{MockTtsEngine, TextToSpeech};
use crate::types::{AudioChunk, SpeechError, VoiceSessionState};
use crate::vad::{VadState, VoiceActivityDetector};
use crate::wakeword::WakeWordDetector;

/// Controller managing the voice interaction pipeline.
pub struct VoiceSessionController {
    state: Arc<RwLock<VoiceSessionState>>,
    event_bus: Arc<EventBus>,
    ai_gateway: Option<Arc<ModelGateway>>,
    stt_engine: Arc<dyn SpeechToText>,
    tts_engine: Arc<dyn TextToSpeech>,
    audio_output: Arc<AudioOutput>,
    #[allow(dead_code)]
    wake_detector: Arc<RwLock<WakeWordDetector>>,
    #[allow(dead_code)]
    vad: Arc<RwLock<VoiceActivityDetector>>,
    #[allow(dead_code)]
    is_running: Arc<AtomicBool>,
}

impl VoiceSessionController {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            state: Arc::new(RwLock::new(VoiceSessionState::Idle)),
            event_bus,
            ai_gateway: None,
            stt_engine: Arc::new(WhisperSttEngine::default()),
            tts_engine: Arc::new(MockTtsEngine),
            audio_output: Arc::new(AudioOutput::new()),
            wake_detector: Arc::new(RwLock::new(WakeWordDetector::default_jarvis())),
            vad: Arc::new(RwLock::new(VoiceActivityDetector::new())),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_ai_gateway(mut self, gateway: Arc<ModelGateway>) -> Self {
        self.ai_gateway = Some(gateway);
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

    /// Read current session state.
    pub async fn current_state(&self) -> VoiceSessionState {
        *self.state.read().await
    }

    /// Trigger state transition and publish event to EventBus.
    #[instrument(skip(self), fields(state = %new_state))]
    async fn transition_state(&self, new_state: VoiceSessionState) {
        let mut guard = self.state.write().await;
        *guard = new_state;
        info!(state = %new_state, "VoiceSessionState transition");
    }

    /// Process an incoming real-time audio chunk through continuous wake-word and VAD evaluation.
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
                let _ = self
                    .event_bus
                    .publish(JarvisEvent::Voice(VoiceEvent::SpeechEnded))
                    .await;
                let _ = self.process_speech_utterance(chunk).await;
            }
        }
    }

    /// Trigger software wake word activation.
    pub async fn trigger_wake_word(&self) -> Result<(), SpeechError> {
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

    /// Process a completed audio speech segment through STT → AI Gateway → TTS → Speaker Output.
    #[instrument(skip(self, speech_chunk))]
    pub async fn process_speech_utterance(
        &self,
        speech_chunk: AudioChunk,
    ) -> Result<String, SpeechError> {
        // 1. Transition to Transcribing
        self.transition_state(VoiceSessionState::Transcribing).await;

        let stt_res = self.stt_engine.transcribe(&speech_chunk).await?;
        info!(text = %stt_res.text, "Speech transcribed");

        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::Transcribed {
                text: stt_res.text.clone(),
                confidence: stt_res.confidence,
                language: stt_res.language.clone(),
            }))
            .await;

        // 2. Transition to Thinking / Reasoning
        self.transition_state(VoiceSessionState::Thinking).await;

        let response_text = if let Some(ai) = &self.ai_gateway {
            ai.ask(&stt_res.text)
                .await
                .unwrap_or_else(|e| format!("I apologize, sir. I encountered an error: {}", e))
        } else {
            format!(
                "Processing query: '{}'. Systems nominal, sir.",
                stt_res.text
            )
        };

        // 3. Transition to Speaking / TTS
        self.transition_state(VoiceSessionState::Speaking).await;

        self.event_bus
            .publish(JarvisEvent::Voice(VoiceEvent::SynthesisStarted {
                text: response_text.clone(),
            }))
            .await;

        let synth = self.tts_engine.synthesize(&response_text).await?;
        let play_dur = synth.duration_ms;

        // Play to speaker (supports barge-in interruption)
        match self.audio_output.play(synth).await {
            Ok(_) => {
                self.event_bus
                    .publish(JarvisEvent::Voice(VoiceEvent::SynthesisCompleted {
                        duration_ms: play_dur,
                    }))
                    .await;
                self.transition_state(VoiceSessionState::Idle).await;
                Ok(response_text)
            }
            Err(SpeechError::Interrupted) => {
                self.interrupt().await;
                Err(SpeechError::Interrupted)
            }
            Err(e) => {
                self.transition_state(VoiceSessionState::Error).await;
                Err(e)
            }
        }
    }
}
