//! Domain types for the JARVIS Voice Subsystem.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Voice session states matching the holographic HUD state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoiceSessionState {
    Idle,
    WakeDetected,
    Listening,
    Transcribing,
    Thinking,
    Speaking,
    Interrupted,
    Error,
}

impl std::fmt::Display for VoiceSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Canonical audio format for internal speech processing (16kHz Mono 32-bit Float PCM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
        }
    }
}

/// A contiguous buffer of audio samples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub format: AudioFormat,
    pub timestamp_ms: u64,
}

impl AudioChunk {
    pub fn duration_ms(&self) -> u64 {
        if self.format.sample_rate == 0 {
            return 0;
        }
        ((self.samples.len() as u64) * 1000) / (self.format.sample_rate as u64)
    }

    pub fn rms_energy(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = self.samples.iter().map(|s| s * s).sum();
        (sum_sq / self.samples.len() as f32).sqrt()
    }
}

/// Audio device metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_input: bool,
    pub supported_sample_rates: Vec<u32>,
}

/// Result of local Speech-to-Text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub language: String,
    pub duration_ms: u64,
    pub is_final: bool,
}

/// Synthesized speech audio ready for speaker output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizedSpeech {
    pub audio_chunk: AudioChunk,
    pub text: String,
    pub duration_ms: u64,
}

/// Voice Subsystem Error Taxonomy.
#[derive(Debug, Error, Clone)]
pub enum SpeechError {
    #[error("Audio device '{0}' is unavailable")]
    DeviceUnavailable(String),

    #[error("Microphone permission denied: {0}")]
    PermissionDenied(String),

    #[error("Audio capture failed: {0}")]
    CaptureFailure(String),

    #[error("Wake word detection error: {0}")]
    WakeWordFailure(String),

    #[error("Speech-to-Text transcription error: {0}")]
    SttFailure(String),

    #[error("Text-to-Speech synthesis error: {0}")]
    TtsFailure(String),

    #[error("Audio output playback error: {0}")]
    OutputFailure(String),

    #[error("Voice session was interrupted by user barge-in")]
    Interrupted,
}
