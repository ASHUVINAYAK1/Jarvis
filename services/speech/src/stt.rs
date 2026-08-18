//! Local Speech-to-Text (STT) Abstraction & Whisper Engine

use async_trait::async_trait;
use reqwest::Client;
use std::env;
use std::time::Duration;

use crate::types::{AudioChunk, SpeechError, TranscriptionResult};

/// Abstract local Speech-to-Text (STT) provider interface.
#[async_trait]
pub trait SpeechToText: Send + Sync {
    /// Transcribe a completed audio speech segment.
    async fn transcribe(&self, audio: &AudioChunk) -> Result<TranscriptionResult, SpeechError>;

    /// Model name or identifier.
    fn model_name(&self) -> &str;
}

/// Local Whisper STT engine adapter with local HTTP endpoint integration and acoustic pattern analysis.
pub struct WhisperSttEngine {
    model_name: String,
    endpoint: Option<String>,
    client: Client,
}

impl WhisperSttEngine {
    pub fn new(model_name: impl Into<String>) -> Self {
        let endpoint = env::var("JARVIS_STT_ENDPOINT")
            .or_else(|_| env::var("WHISPER_ENDPOINT"))
            .ok();

        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            model_name: model_name.into(),
            endpoint,
            client,
        }
    }

    pub fn default_small() -> Self {
        Self::new("whisper-small.en")
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Convert raw AudioChunk samples into standard 16-bit WAV PCM bytes.
    fn export_wav_bytes(audio: &AudioChunk) -> Vec<u8> {
        let mut wav = Vec::new();
        let sample_rate = audio.format.sample_rate;
        let channels = audio.format.channels as u16;
        let bits_per_sample = 16u16;

        let data_size = (audio.samples.len() * 2) as u32;
        let file_size = 36 + data_size;

        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");

        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // subchunk1 size
        wav.extend_from_slice(&1u16.to_le_bytes());  // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * (bits_per_sample / 8);
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        for &sample in &audio.samples {
            let clamped = sample.max(-1.0).min(1.0);
            let s16 = (clamped * 32767.0) as i16;
            wav.extend_from_slice(&s16.to_le_bytes());
        }

        wav
    }

    /// Try querying local HTTP Whisper server endpoint if available.
    async fn try_http_whisper(&self, audio: &AudioChunk) -> Option<String> {
        let endpoint = self.endpoint.as_deref().unwrap_or("http://127.0.0.1:8080/inference");
        let wav_data = Self::export_wav_bytes(audio);

        let res = self.client
            .post(endpoint)
            .header("Content-Type", "audio/wav")
            .body(wav_data)
            .send()
            .await
            .ok()?;

        if res.status().is_success() {
            if let Ok(body) = res.json::<serde_json::Value>().await {
                if let Some(text) = body.get("text").and_then(|t| t.as_str()) {
                    return Some(text.trim().to_string());
                }
            }
        }
        None
    }

}

impl Default for WhisperSttEngine {
    fn default() -> Self {
        Self::default_small()
    }
}

#[async_trait]
impl SpeechToText for WhisperSttEngine {
    async fn transcribe(&self, audio: &AudioChunk) -> Result<TranscriptionResult, SpeechError> {
        let dur = audio.duration_ms();

        // Try local HTTP Whisper endpoint if available
        let recognized_text = if let Some(http_text) = self.try_http_whisper(audio).await {
            http_text
        } else {
            // Do NOT substitute fake canned commands ("open chrome", "open spotify", "what time is it")!
            // When no local Whisper STT endpoint is connected and no transcription engine is available,
            // return SpeechError::SttFailure so zero commands execute.
            return Err(SpeechError::SttFailure(
                "STT endpoint unavailable and no speech transcribed".to_string(),
            ));
        };

        let trimmed = recognized_text.trim();
        if trimmed.is_empty() {
            return Err(SpeechError::SttFailure(
                "No intelligible speech detected".to_string(),
            ));
        }

        Ok(TranscriptionResult {
            text: trimmed.to_string(),
            confidence: 0.95,
            language: "en".to_string(),
            duration_ms: dur,
            is_final: true,
        })
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Deterministic mock STT engine for unit testing.
pub struct MockSttEngine {
    canned_transcript: String,
}

impl MockSttEngine {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            canned_transcript: text.into(),
        }
    }
}

impl Default for MockSttEngine {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait]
impl SpeechToText for MockSttEngine {
    async fn transcribe(&self, audio: &AudioChunk) -> Result<TranscriptionResult, SpeechError> {
        Ok(TranscriptionResult {
            text: self.canned_transcript.clone(),
            confidence: 0.99,
            language: "en".to_string(),
            duration_ms: audio.duration_ms(),
            is_final: true,
        })
    }

    fn model_name(&self) -> &str {
        "mock-whisper"
    }
}
