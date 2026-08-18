//! Real-time Microphone Audio Capture & Physical Audio Input Stream (cpal)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::mpsc;
use tracing::{error, info, instrument, warn};

use crate::types::{AudioChunk, AudioFormat, SpeechError};

/// Bounded real-time microphone stream producer powered by cpal.
pub struct AudioCapture {
    is_active: Arc<AtomicBool>,
    format: AudioFormat,
}

impl AudioCapture {
    pub fn new(format: AudioFormat) -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            format,
        }
    }

    pub fn default_16k_mono() -> Self {
        Self::new(AudioFormat::default())
    }

    pub fn is_capturing(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    /// Start real-time physical microphone capture returning a receiver stream for PCM chunks.
    #[instrument(skip(self))]
    pub fn start_capture(&self) -> Result<mpsc::Receiver<AudioChunk>, SpeechError> {
        if self.is_active.swap(true, Ordering::SeqCst) {
            return Err(SpeechError::CaptureFailure(
                "Audio capture is already active".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(128);
        let active_flag = self.is_active.clone();
        let target_format = self.format;

        // Spawn real-time audio capture driver thread
        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(dev) => dev,
                None => {
                    error!("NO MICROPHONE DEVICE FOUND ON SYSTEM");
                    active_flag.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let device_name = device.name().unwrap_or_else(|_| "Unknown Mic".to_string());
            info!(device = %device_name, "Connecting to physical microphone device");

            let config = match device.default_input_config() {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to get default input microphone config");
                    active_flag.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let sample_rate = config.sample_rate().0;
            let channels = config.channels();
            info!(
                device = %device_name,
                sample_rate = sample_rate,
                channels = channels,
                sample_format = ?config.sample_format(),
                "[MIC DIAGNOSTIC] Microphone initialized successfully"
            );

            let tx_clone = tx.clone();
            let active_flag_clone = active_flag.clone();

            let err_fn = move |err| {
                error!(error = %err, "[MIC DIAGNOSTIC] Audio capture stream error");
            };

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        if !active_flag_clone.load(Ordering::SeqCst) {
                            return;
                        }
                        let samples: Vec<f32> = data.iter().copied().collect();
                        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
                        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
                        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

                        if rms > 0.005 {
                            info!(
                                rms = format!("{:.4}", rms),
                                peak = format!("{:.4}", peak),
                                samples = samples.len(),
                                "[MIC TEST] Live voice audio input detected!"
                            );
                        }

                        let chunk = AudioChunk {
                            samples,
                            format: target_format,
                            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
                        };
                        let _ = tx_clone.try_send(chunk);
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        if !active_flag_clone.load(Ordering::SeqCst) {
                            return;
                        }
                        let samples: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
                        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
                        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

                        if rms > 0.005 {
                            info!(
                                rms = format!("{:.4}", rms),
                                peak = format!("{:.4}", peak),
                                samples = samples.len(),
                                "[MIC TEST] Live voice audio input detected!"
                            );
                        }

                        let chunk = AudioChunk {
                            samples,
                            format: target_format,
                            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
                        };
                        let _ = tx_clone.try_send(chunk);
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &_| {
                        if !active_flag_clone.load(Ordering::SeqCst) {
                            return;
                        }
                        let samples: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();
                        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
                        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
                        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

                        if rms > 0.005 {
                            info!(
                                rms = format!("{:.4}", rms),
                                peak = format!("{:.4}", peak),
                                samples = samples.len(),
                                "[MIC TEST] Live voice audio input detected!"
                            );
                        }

                        let chunk = AudioChunk {
                            samples,
                            format: target_format,
                            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
                        };
                        let _ = tx_clone.try_send(chunk);
                    },
                    err_fn,
                    None,
                ),
                sample_format => {
                    warn!(
                        ?sample_format,
                        "Unsupported cpal sample format, falling back"
                    );
                    return;
                }
            };

            if let Ok(stream) = stream {
                if stream.play().is_ok() {
                    info!("[MIC DIAGNOSTIC] Microphone audio stream recording active...");
                    while active_flag.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            }

            active_flag.store(false, Ordering::SeqCst);
            info!("Microphone audio capture stream stopped");
        });

        Ok(rx)
    }

    /// Stop current capture stream.
    pub fn stop_capture(&self) {
        self.is_active.store(false, Ordering::SeqCst);
    }
}
