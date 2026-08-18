//! Local Text-to-Speech (TTS) Abstraction & Piper Engine

use std::path::PathBuf;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::{AudioChunk, AudioFormat, SpeechError, SynthesizedSpeech};

/// Abstract local Text-to-Speech (TTS) provider interface.
#[async_trait]
pub trait TextToSpeech: Send + Sync {
    /// Synthesize text into speech audio.
    async fn synthesize(&self, text: &str) -> Result<SynthesizedSpeech, SpeechError>;

    /// Voice model identifier.
    fn voice_name(&self) -> &str;
}

/// Configuration parameters for external Piper TTS executable process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiperConfig {
    pub executable_path: PathBuf,
    pub model_path: PathBuf,
    pub speaker_id: Option<u32>,
    pub sample_rate: u32,
}

impl PiperConfig {
    pub fn new(executable_path: impl Into<PathBuf>, model_path: impl Into<PathBuf>) -> Self {
        Self {
            executable_path: executable_path.into(),
            model_path: model_path.into(),
            speaker_id: None,
            sample_rate: 22050,
        }
    }

    /// Discover Piper executable and ONNX model deterministically:
    /// 1. Explicit environment variables (JARVIS_PIPER_PATH, JARVIS_PIPER_MODEL)
    /// 2. User local app directory (%USERPROFILE%\.jarvis\piper\ or %LOCALAPPDATA%\JARVIS\piper\)
    /// 3. System PATH lookup
    pub fn discover() -> Result<Self, SpeechError> {
        let exe_env = std::env::var("JARVIS_PIPER_PATH").ok().map(PathBuf::from);
        let model_env = std::env::var("JARVIS_PIPER_MODEL").ok().map(PathBuf::from);

        let exe_path = if let Some(p) = exe_env {
            if p.exists() {
                p
            } else {
                return Err(SpeechError::TtsFailure(format!(
                    "JARVIS_PIPER_PATH points to non-existent executable: {:?}",
                    p
                )));
            }
        } else if let Some(p) = Self::find_local_executable() {
            p
        } else if let Some(p) = Self::find_in_path("piper") {
            p
        } else if let Some(p) = Self::find_in_path("piper.exe") {
            p
        } else {
            return Err(SpeechError::TtsFailure(
                "Piper executable ('piper.exe') not found. Set JARVIS_PIPER_PATH or place piper in PATH / %USERPROFILE%\\.jarvis\\piper\\".to_string()
            ));
        };

        let model_path = if let Some(m) = model_env {
            if m.exists() {
                m
            } else {
                return Err(SpeechError::TtsFailure(format!(
                    "JARVIS_PIPER_MODEL points to non-existent voice model: {:?}",
                    m
                )));
            }
        } else if let Some(m) = Self::find_local_model() {
            m
        } else {
            return Err(SpeechError::TtsFailure(
                "Piper ONNX voice model file not found. Set JARVIS_PIPER_MODEL or place model.onnx in %USERPROFILE%\\.jarvis\\models\\piper\\".to_string()
            ));
        };

        Ok(Self {
            executable_path: exe_path,
            model_path,
            speaker_id: None,
            sample_rate: 22050,
        })
    }

    fn find_local_executable() -> Option<PathBuf> {
        if let Some(home) = dirs_home() {
            let candidate1 = home.join(".jarvis").join("piper").join("piper.exe");
            if candidate1.exists() {
                return Some(candidate1);
            }
            let candidate2 = home.join(".jarvis").join("piper").join("piper");
            if candidate2.exists() {
                return Some(candidate2);
            }
        }
        None
    }

    fn find_local_model() -> Option<PathBuf> {
        if let Some(home) = dirs_home() {
            let candidates = [
                home.join(".jarvis").join("models").join("piper").join("en_GB-alan-medium.onnx"),
                home.join(".jarvis").join("models").join("piper").join("model.onnx"),
                home.join(".jarvis").join("piper").join("model.onnx"),
            ];
            for c in candidates {
                if c.exists() {
                    return Some(c);
                }
            }
        }
        None
    }

    fn find_in_path(cmd_name: &str) -> Option<PathBuf> {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(cmd_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Local Piper TTS engine adapter calling the external process safely.
pub struct PiperTtsEngine {
    voice_name: String,
    config: Option<PiperConfig>,
}

impl PiperTtsEngine {
    pub fn new(voice_name: impl Into<String>) -> Self {
        Self {
            voice_name: voice_name.into(),
            config: PiperConfig::discover().ok(),
        }
    }

    pub fn with_config(config: PiperConfig) -> Self {
        let voice_name = config
            .model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("en_GB-alan-medium")
            .to_string();
        Self {
            voice_name,
            config: Some(config),
        }
    }

    pub fn default_jarvis_voice() -> Self {
        if let Ok(config) = PiperConfig::discover() {
            Self::with_config(config)
        } else {
            Self {
                voice_name: "en_GB-alan-medium".to_string(),
                config: None,
            }
        }
    }
}

impl Default for PiperTtsEngine {
    fn default() -> Self {
        Self::default_jarvis_voice()
    }
}

/// Helper function to parse RIFF WAV audio bytes (or raw 16-bit LE PCM fallback).
pub fn parse_audio_bytes(bytes: &[u8], fallback_sample_rate: u32) -> Result<(Vec<f32>, AudioFormat), SpeechError> {
    if bytes.is_empty() {
        return Err(SpeechError::TtsFailure(
            "Synthesized audio buffer is empty".to_string(),
        ));
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        let mut pos = 12;
        let mut audio_format_tag: u16 = 1; // 1 = PCM
        let mut channels: u16 = 1;
        let mut sample_rate: u32 = fallback_sample_rate;
        let mut bits_per_sample: u16 = 16;
        let mut data_bytes: Option<&[u8]> = None;

        while pos + 8 <= bytes.len() {
            let chunk_id = &bytes[pos..pos + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]) as usize;

            let chunk_data_start = pos + 8;
            let chunk_data_end = (chunk_data_start + chunk_size).min(bytes.len());

            if chunk_id == b"fmt " && chunk_data_start + 14 <= bytes.len() {
                let fmt_data = &bytes[chunk_data_start..chunk_data_end];
                if fmt_data.len() >= 14 {
                    audio_format_tag = u16::from_le_bytes([fmt_data[0], fmt_data[1]]);
                    channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]);
                    sample_rate = u32::from_le_bytes([fmt_data[4], fmt_data[5], fmt_data[6], fmt_data[7]]);
                    if fmt_data.len() >= 16 {
                        bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]);
                    }
                }
            } else if chunk_id == b"data" {
                data_bytes = Some(&bytes[chunk_data_start..chunk_data_end]);
                break;
            }

            // Word alignment in RIFF format (chunks are padded to even byte length)
            let pad = chunk_size % 2;
            pos += 8 + chunk_size + pad;
        }

        let pcm_bytes = match data_bytes {
            Some(b) => b,
            None => {
                if bytes.len() > 44 {
                    &bytes[44..]
                } else {
                    return Err(SpeechError::TtsFailure(
                        "No data chunk found in WAV header".to_string(),
                    ));
                }
            }
        };

        let mut samples = Vec::with_capacity(pcm_bytes.len() / 2);

        if bits_per_sample == 16 {
            for chunk in pcm_bytes.chunks_exact(2) {
                let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                samples.push((sample_i16 as f32 / 32768.0).clamp(-1.0, 1.0));
            }
        } else if bits_per_sample == 32 {
            if audio_format_tag == 3 {
                // IEEE Float
                for chunk in pcm_bytes.chunks_exact(4) {
                    let sample_f32 = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    samples.push(sample_f32.clamp(-1.0, 1.0));
                }
            } else {
                // 32-bit integer PCM
                for chunk in pcm_bytes.chunks_exact(4) {
                    let sample_i32 = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    samples.push((sample_i32 as f32 / 2147483648.0).clamp(-1.0, 1.0));
                }
            }
        } else if bits_per_sample == 8 {
            for &b in pcm_bytes {
                samples.push(((b as f32 - 128.0) / 128.0).clamp(-1.0, 1.0));
            }
        } else {
            return Err(SpeechError::TtsFailure(format!(
                "Unsupported WAV bits per sample: {}",
                bits_per_sample
            )));
        }

        Ok((
            samples,
            AudioFormat {
                sample_rate,
                channels: if channels == 0 { 1 } else { channels },
            },
        ))
    } else {
        // Raw 16-bit LE PCM fallback
        let mut samples = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            samples.push((sample_i16 as f32 / 32768.0).clamp(-1.0, 1.0));
        }
        Ok((
            samples,
            AudioFormat {
                sample_rate: fallback_sample_rate,
                channels: 1,
            },
        ))
    }
}

#[async_trait]
impl TextToSpeech for PiperTtsEngine {
    async fn synthesize(&self, text: &str) -> Result<SynthesizedSpeech, SpeechError> {
        let config = match &self.config {
            Some(cfg) => cfg,
            None => {
                return Err(SpeechError::TtsFailure(
                    "Piper TTS is unconfigured: missing executable or voice model path".to_string(),
                ));
            }
        };

        if !config.executable_path.exists() {
            return Err(SpeechError::TtsFailure(format!(
                "Piper executable missing at {:?}",
                config.executable_path
            )));
        }
        if !config.model_path.exists() {
            return Err(SpeechError::TtsFailure(format!(
                "Piper voice model missing at {:?}",
                config.model_path
            )));
        }

        let temp_dir = std::env::temp_dir();
        let temp_filename = format!("jarvis_tts_{}.wav", uuid::Uuid::new_v4());
        let temp_wav_path = temp_dir.join(temp_filename);

        let mut cmd = tokio::process::Command::new(&config.executable_path);
        cmd.arg("--model").arg(&config.model_path);
        cmd.arg("--output_file").arg(&temp_wav_path);
        if let Some(spk) = config.speaker_id {
            cmd.arg("--speaker").arg(spk.to_string());
        }

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            SpeechError::TtsFailure(format!(
                "Failed to spawn Piper process ({:?}): {}",
                config.executable_path, e
            ))
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let text_bytes = text.as_bytes();
            let _ = stdin.write_all(text_bytes).await;
            let _ = stdin.write_all(b"\n").await;
            let _ = stdin.flush().await;
            drop(stdin);
        }

        let output = child.wait_with_output().await.map_err(|e| {
            SpeechError::TtsFailure(format!("Failed waiting for Piper process output: {}", e))
        })?;

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            let _ = tokio::fs::remove_file(&temp_wav_path).await;
            return Err(SpeechError::TtsFailure(format!(
                "Piper process exited with error status (code {:?}): {}",
                output.status.code(),
                stderr_msg.trim()
            )));
        }

        let wav_bytes = match tokio::fs::read(&temp_wav_path).await {
            Ok(b) => b,
            Err(e) => {
                let _ = tokio::fs::remove_file(&temp_wav_path).await;
                return Err(SpeechError::TtsFailure(format!(
                    "Failed reading generated Piper WAV file {:?}: {}",
                    temp_wav_path, e
                )));
            }
        };

        let _ = tokio::fs::remove_file(&temp_wav_path).await;

        let (samples, format) = parse_audio_bytes(&wav_bytes, config.sample_rate)?;
        let duration_ms = if format.sample_rate > 0 {
            ((samples.len() as u64) * 1000) / (format.sample_rate as u64 * format.channels as u64)
        } else {
            500
        };

        Ok(SynthesizedSpeech {
            audio_chunk: AudioChunk {
                samples,
                format,
                timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            },
            text: text.to_string(),
            duration_ms: duration_ms.max(50),
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
                samples: vec![0.1f32; 1600],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piper_config_new() {
        let cfg = PiperConfig::new("C:\\bin\\piper.exe", "C:\\models\\en.onnx");
        assert_eq!(cfg.executable_path, PathBuf::from("C:\\bin\\piper.exe"));
        assert_eq!(cfg.model_path, PathBuf::from("C:\\models\\en.onnx"));
        assert_eq!(cfg.sample_rate, 22050);
    }

    #[tokio::test]
    async fn test_piper_missing_executable_error() {
        let cfg = PiperConfig::new("nonexistent_piper_exe_12345.exe", "nonexistent_model.onnx");
        let engine = PiperTtsEngine::with_config(cfg);
        let res = engine.synthesize("hello").await;
        assert!(res.is_err());
        match res.unwrap_err() {
            SpeechError::TtsFailure(msg) => {
                assert!(msg.contains("Piper executable missing"));
            }
            other => panic!("Expected TtsFailure, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_piper_unconfigured_error() {
        let engine = PiperTtsEngine {
            voice_name: "unconfigured".to_string(),
            config: None,
        };
        let res = engine.synthesize("hello").await;
        assert!(res.is_err());
        match res.unwrap_err() {
            SpeechError::TtsFailure(msg) => {
                assert!(msg.contains("unconfigured"));
            }
            other => panic!("Expected TtsFailure, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_audio_bytes_raw_pcm() {
        // Create 4 bytes of i16 PCM (2 samples)
        let raw = vec![0x00, 0x40, 0x00, 0xC0]; // +16384 (+0.5f32), -16384 (-0.5f32)
        let (samples, format) = parse_audio_bytes(&raw, 16000).unwrap();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.5).abs() < 0.01);
        assert!((samples[1] - (-0.5)).abs() < 0.01);
        assert_eq!(format.sample_rate, 16000);
    }

    #[test]
    fn test_parse_audio_bytes_empty_err() {
        let res = parse_audio_bytes(&[], 16000);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_audio_bytes_wav_header() {
        // Construct a synthetic 44-byte RIFF WAV buffer (16-bit PCM mono 22050Hz)
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + 8u32).to_le_bytes()); // Chunk size
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk size (16 for PCM)
        wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
        wav.extend_from_slice(&1u16.to_le_bytes()); // Channels (1 = Mono)
        wav.extend_from_slice(&22050u32.to_le_bytes()); // Sample rate
        wav.extend_from_slice(&(22050u32 * 2).to_le_bytes()); // Byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // Block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&8u32.to_le_bytes()); // Data chunk size (8 bytes = 4 i16 samples)
        wav.extend_from_slice(&[0x00, 0x40, 0x00, 0xC0, 0x00, 0x20, 0x00, 0xE0]);

        let (samples, format) = parse_audio_bytes(&wav, 16000).unwrap();
        assert_eq!(format.sample_rate, 22050);
        assert_eq!(format.channels, 1);
        assert_eq!(samples.len(), 4);
        assert!((samples[0] - 0.5).abs() < 0.01);
        assert!((samples[1] - (-0.5)).abs() < 0.01);
        assert!((samples[2] - 0.25).abs() < 0.01);
        assert!((samples[3] - (-0.25)).abs() < 0.01);
    }

    #[test]
    fn test_parse_real_piper_test_wav_if_present() {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let p = std::path::Path::new(&user_profile).join(".jarvis\\piper\\test.wav");
            if p.exists() {
                let bytes = std::fs::read(&p).unwrap();
                let (samples, format) = parse_audio_bytes(&bytes, 16000).unwrap();
                assert_eq!(format.sample_rate, 22050);
                assert_eq!(format.channels, 1);
                assert!(!samples.is_empty());
                for &s in &samples[0..10.min(samples.len())] {
                    assert!(s >= -1.0 && s <= 1.0);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_mock_tts_engine_returns_valid_speech() {
        let engine = MockTtsEngine;
        let speech = engine.synthesize("Chrome is open, sir.").await.unwrap();
        assert_eq!(speech.text, "Chrome is open, sir.");
        assert_eq!(speech.audio_chunk.samples.len(), 1600);
        assert_eq!(speech.duration_ms, 100);
        assert_eq!(engine.voice_name(), "mock-piper-voice");
    }
}
