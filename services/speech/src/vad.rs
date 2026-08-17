//! Voice Activity Detection (VAD)

use crate::types::AudioChunk;

/// VAD State evaluating continuous audio chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadState {
    Silence,
    SpeechStarted,
    SpeechContinuing,
    SpeechEnded,
}

/// Voice Activity Detector evaluating energy thresholds and silence timeouts.
pub struct VoiceActivityDetector {
    energy_threshold: f32,
    silence_duration_ms: u64,
    min_speech_duration_ms: u64,
    max_speech_duration_ms: u64,
    current_speech_ms: u64,
    current_silence_ms: u64,
    is_speaking: bool,
}

impl VoiceActivityDetector {
    pub fn new() -> Self {
        Self {
            energy_threshold: 0.0035,
            silence_duration_ms: 700, // 700ms silence ends utterance
            min_speech_duration_ms: 300,
            max_speech_duration_ms: 15000,
            current_speech_ms: 0,
            current_silence_ms: 0,
            is_speaking: false,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.energy_threshold = threshold;
        self
    }

    pub fn reset(&mut self) {
        self.current_speech_ms = 0;
        self.current_silence_ms = 0;
        self.is_speaking = false;
    }

    /// Process a 100ms audio chunk and return the updated VAD state.
    pub fn process_chunk(&mut self, chunk: &AudioChunk) -> VadState {
        let chunk_dur = chunk.duration_ms();
        let rms = chunk.rms_energy();

        let has_energy = rms >= self.energy_threshold;

        if has_energy {
            self.current_silence_ms = 0;
            self.current_speech_ms += chunk_dur;

            if !self.is_speaking {
                if self.current_speech_ms >= self.min_speech_duration_ms {
                    self.is_speaking = true;
                    return VadState::SpeechStarted;
                }
            } else {
                if self.current_speech_ms >= self.max_speech_duration_ms {
                    self.is_speaking = false;
                    return VadState::SpeechEnded;
                }
                return VadState::SpeechContinuing;
            }
        } else if self.is_speaking {
            self.current_silence_ms += chunk_dur;
            self.current_speech_ms += chunk_dur;

            if self.current_silence_ms >= self.silence_duration_ms {
                self.is_speaking = false;
                return VadState::SpeechEnded;
            }
            return VadState::SpeechContinuing;
        }

        VadState::Silence
    }
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new()
    }
}
