//! JARVIS Local Voice Pipeline & Audio Subsystem
//!
//! Provides real-time microphone capture, Voice Activity Detection (VAD),
//! local "JARVIS" wake-word detection, Whisper Speech-to-Text (STT),
//! Piper Text-to-Speech (TTS), audio output playback, and duplex barge-in interruption.
//!
//! IMPLEMENTATION STATUS: Phase 5, Milestones M05.01 → M05.16

pub mod capture;
pub mod device;
pub mod output;
pub mod session;
pub mod stt;
pub mod tts;
pub mod types;
pub mod vad;
pub mod wakeword;

pub use capture::AudioCapture;
pub use device::{AudioDeviceManager, DefaultAudioDeviceManager};
pub use output::AudioOutput;
pub use session::VoiceSessionController;
pub use stt::{MockSttEngine, SpeechToText, WhisperSttEngine};
pub use tts::{MockTtsEngine, PiperTtsEngine, TextToSpeech};
pub use types::*;
pub use vad::{VadState, VoiceActivityDetector};
pub use wakeword::{WakeWordDetector, WakeWordResult};

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use jarvis_event_bus::EventBus;

    use super::*;

    #[test]
    fn test_audio_chunk_rms_energy() {
        let chunk = AudioChunk {
            samples: vec![0.5, -0.5, 0.5, -0.5],
            format: AudioFormat::default(),
            timestamp_ms: 1000,
        };
        assert_eq!(chunk.duration_ms(), 0);
        assert!((chunk.rms_energy() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_vad_state_machine() {
        let mut vad = VoiceActivityDetector::new().with_threshold(0.01);
        assert_eq!(
            vad.process_chunk(&AudioChunk {
                samples: vec![0.0; 1600],
                format: AudioFormat::default(),
                timestamp_ms: 0,
            }),
            VadState::Silence
        );

        // Feed speech energy
        let speech_chunk = AudioChunk {
            samples: vec![0.1; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 100,
        };

        // Needs min speech duration (300ms) to trigger SpeechStarted
        vad.process_chunk(&speech_chunk);
        vad.process_chunk(&speech_chunk);
        let state = vad.process_chunk(&speech_chunk);
        assert_eq!(state, VadState::SpeechStarted);
    }

    #[test]
    fn test_wake_word_manual_trigger_and_cooldown() {
        let mut detector = WakeWordDetector::default_jarvis();
        assert!(detector.is_enabled());

        let res = detector.trigger_manual();
        assert!(res.detected);
        assert_eq!(res.keyword, "JARVIS");
        assert!(res.confidence > 0.9);

        // Immediate next detection should be suppressed by cooldown
        let dummy_chunk = AudioChunk {
            samples: vec![0.0; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };
        let res2 = detector.detect(&dummy_chunk);
        assert!(!res2.detected);
    }

    #[tokio::test]
    async fn test_stt_and_tts_engines() {
        let stt = MockSttEngine::new("open application chrome");
        let chunk = AudioChunk {
            samples: vec![0.0; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };
        let res = stt.transcribe(&chunk).await.unwrap();
        assert_eq!(res.text, "open application chrome");

        let tts = MockTtsEngine;
        let synth = tts.synthesize("Chrome is open, sir.").await.unwrap();
        assert_eq!(synth.text, "Chrome is open, sir.");
    }

    #[tokio::test]
    async fn test_audio_output_barge_in_interruption() {
        let output = Arc::new(AudioOutput::new());

        let synth = SynthesizedSpeech {
            audio_chunk: AudioChunk {
                samples: vec![0.0; 8000],
                format: AudioFormat::default(),
                timestamp_ms: 0,
            },
            text: "Long speaking response...".to_string(),
            duration_ms: 500,
        };

        let output_clone = output.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            output_clone.stop(); // User barge-in stop
        });

        let res = output.play(synth).await;
        assert!(matches!(res, Err(SpeechError::Interrupted)));
    }

    #[tokio::test]
    async fn test_voice_session_controller_full_flow() {
        let bus = Arc::new(EventBus::new(256));
        let mut rx = bus.subscribe();

        let controller = VoiceSessionController::new(bus.clone()).with_stt(Arc::new(MockSttEngine::default()));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);

        controller.trigger_wake_word().await.unwrap();
        assert_eq!(
            controller.current_state().await,
            VoiceSessionState::Listening
        );

        // Verify VoiceEvent::WakeWordDetected was broadcast
        assert!(rx.recv().await.is_ok());

        let chunk = AudioChunk {
            samples: vec![0.05; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };

        let response = controller.process_speech_utterance(chunk).await.unwrap();
        assert!(response.contains("Processing query"));
        assert_eq!(controller.current_state().await, VoiceSessionState::Idle);
    }

    #[test]
    fn test_wake_word_negative_silence_remains_idle() {
        let mut detector = WakeWordDetector::default_jarvis();
        let silence_chunk = AudioChunk {
            samples: vec![0.0; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };
        let res = detector.detect(&silence_chunk);
        assert!(!res.detected, "Silence must not trigger wake word");
    }

    #[test]
    fn test_wake_word_negative_unrelated_speech_remains_idle() {
        let mut detector = WakeWordDetector::default_jarvis();
        // Generate synthetic speech-like chunk without "Jarvis" phonetic structure
        let mut samples = Vec::with_capacity(1600);
        for i in 0..1600 {
            let t = i as f32 / 16000.0;
            // 440 Hz tone (flat vowel sound "Open Chrome" without sibilance transition)
            samples.push(0.02 * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }
        let chunk = AudioChunk {
            samples,
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };
        let res = detector.detect(&chunk);
        assert!(!res.detected, "Unrelated speech ('Open Chrome') without 'Jarvis' must not trigger wake word");
    }

    #[test]
    fn test_wake_word_cooldown_prevents_duplicate_triggers() {
        let mut detector = WakeWordDetector::default_jarvis();
        let manual_res = detector.trigger_manual();
        assert!(manual_res.detected);

        // Next immediate audio chunk within cooldown window must be rejected
        let chunk = AudioChunk {
            samples: vec![0.05; 1600],
            format: AudioFormat::default(),
            timestamp_ms: 0,
        };
        let res = detector.detect(&chunk);
        assert!(!res.detected, "Cooldown must prevent duplicate wake triggers");
    }
}
