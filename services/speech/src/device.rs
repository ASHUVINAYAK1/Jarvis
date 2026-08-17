//! Audio Device Management (Microphones & Speakers)

use async_trait::async_trait;

use crate::types::{AudioDeviceInfo, SpeechError};

/// Interface for enumerating and querying physical audio devices.
#[async_trait]
pub trait AudioDeviceManager: Send + Sync {
    /// List all available input devices (microphones).
    async fn list_microphones(&self) -> Result<Vec<AudioDeviceInfo>, SpeechError>;

    /// List all available output devices (speakers).
    async fn list_speakers(&self) -> Result<Vec<AudioDeviceInfo>, SpeechError>;

    /// Get default microphone device info.
    async fn default_microphone(&self) -> Result<AudioDeviceInfo, SpeechError>;

    /// Get default speaker device info.
    async fn default_speaker(&self) -> Result<AudioDeviceInfo, SpeechError>;
}

/// Mock / Default Audio Device Manager for cross-platform fallback and unit testing.
pub struct DefaultAudioDeviceManager {
    mock_mic_name: String,
    mock_speaker_name: String,
}

impl DefaultAudioDeviceManager {
    pub fn new() -> Self {
        Self {
            mock_mic_name: "Default Microphone".to_string(),
            mock_speaker_name: "Default Speakers".to_string(),
        }
    }
}

impl Default for DefaultAudioDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioDeviceManager for DefaultAudioDeviceManager {
    async fn list_microphones(&self) -> Result<Vec<AudioDeviceInfo>, SpeechError> {
        Ok(vec![AudioDeviceInfo {
            id: "mic_default".to_string(),
            name: self.mock_mic_name.clone(),
            is_default: true,
            is_input: true,
            supported_sample_rates: vec![16000, 44100, 48000],
        }])
    }

    async fn list_speakers(&self) -> Result<Vec<AudioDeviceInfo>, SpeechError> {
        Ok(vec![AudioDeviceInfo {
            id: "speaker_default".to_string(),
            name: self.mock_speaker_name.clone(),
            is_default: true,
            is_input: false,
            supported_sample_rates: vec![16000, 44100, 48000],
        }])
    }

    async fn default_microphone(&self) -> Result<AudioDeviceInfo, SpeechError> {
        let list = self.list_microphones().await?;
        list.into_iter().next().ok_or_else(|| {
            SpeechError::DeviceUnavailable("No microphone device found".to_string())
        })
    }

    async fn default_speaker(&self) -> Result<AudioDeviceInfo, SpeechError> {
        let list = self.list_speakers().await?;
        list.into_iter().next().ok_or_else(|| {
            SpeechError::DeviceUnavailable("No speaker device found".to_string())
        })
    }
}
