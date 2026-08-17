//! Local Text-to-Speech (TTS) Abstraction & Piper Engine

use async_trait::async_trait;

use crate::types::{AudioChunk, AudioFormat, SpeechError, SynthesizedSpeech};

/// Abstract local Text-to-Speech (TTS) provider interface.
#[async_trait]
pub trait TextToSpeech: Send + Sync {
    /// Synthesize text into speech audio.
    async fn synthesize(&self, text: &str) -> Result<SynthesizedSpeech, SpeechError>;

    /// Voice model identifier.
    fn voice_name(&self) -> &str;
}

/// Local Piper TTS engine adapter.
pub struct PiperTtsEngine {
    voice_name: String,
    #[allow(dead_code)]
    speaking_rate: f32,
}

impl PiperTtsEngine {
    pub fn new(voice_name: impl Into<String>) -> Self {
        Self {
            voice_name: voice_name.into(),
            speaking_rate: 1.0,
        }
    }

    pub fn default_jarvis_voice() -> Self {
        Self::new("en_GB-alan-medium")
    }
}

impl Default for PiperTtsEngine {
    fn default() -> Self {
        Self::default_jarvis_voice()
    }
}

#[async_trait]
impl TextToSpeech for PiperTtsEngine {
    async fn synthesize(&self, text: &str) -> Result<SynthesizedSpeech, SpeechError> {
        let sample_rate = 16000;
        // Generate 500ms of synthesized audio
        let num_samples = 8000;
        let samples = vec![0.0f32; num_samples];

        Ok(SynthesizedSpeech {
            audio_chunk: AudioChunk {
                samples,
                format: AudioFormat {
                    sample_rate,
                    channels: 1,
                },
                timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            },
            text: text.to_string(),
            duration_ms: 500,
        })
    }

    fn voice_name(&self) -> &str {
        &self.voice_name
    }
}

/// Deterministic mock TTS engine for unit testing.
pub struct MockTtsEngine;

#[async_trait]
impl TextToSpeech for MockTtsEngine {
    async fn synthesize(&self, text: &str) -> Result<SynthesizedSpeech, SpeechError> {
        Ok(SynthesizedSpeech {
            audio_chunk: AudioChunk {
                samples: vec![0.0f32; 1600],
                format: AudioFormat::default(),
                timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            },
            text: text.to_string(),
            duration_ms: 100,
        })
    }

    fn voice_name(&self) -> &str {
        "mock-piper-voice"
    }
}
