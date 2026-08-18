//! Audio Output & Speaker Playback Control via CPAL

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn, instrument};

use crate::types::{SpeechError, SynthesizedSpeech};

/// Resample input float samples to match device target sample rate and channel count.
pub fn resample_pcm(
    input: &[f32],
    in_rate: u32,
    in_channels: u16,
    out_rate: u32,
    out_channels: u16,
) -> Vec<f32> {
    if input.is_empty() || in_rate == 0 || out_rate == 0 || in_channels == 0 || out_channels == 0 {
        return Vec::new();
    }

    // Step 1: Convert input to mono frames (average channels per frame)
    let in_ch = in_channels as usize;
    let mono_input: Vec<f32> = if in_channels == 1 {
        input.to_vec()
    } else {
        input
            .chunks(in_ch)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    };

    // Step 2: Resample mono stream if input and output rates differ
    let resampled_mono: Vec<f32> = if in_rate == out_rate {
        mono_input
    } else {
        let ratio = in_rate as f64 / out_rate as f64;
        let target_frames = ((mono_input.len() as f64) / ratio).round() as usize;
        let mut res = Vec::with_capacity(target_frames);

        for i in 0..target_frames {
            let src_idx = (i as f64) * ratio;
            let idx0 = src_idx.floor() as usize;
            let idx1 = (idx0 + 1).min(mono_input.len().saturating_sub(1));
            let frac = (src_idx - idx0 as f64) as f32;

            let s0 = mono_input.get(idx0).copied().unwrap_or(0.0);
            let s1 = mono_input.get(idx1).copied().unwrap_or(0.0);

            let sample = (s0 + frac * (s1 - s0)).clamp(-1.0, 1.0);
            res.push(sample);
        }
        res
    };

    // Step 3: Expand mono stream to target channel count (e.g. 1, 2, 4, 6, 8 channels)
    let out_ch = out_channels as usize;
    if out_channels == 1 {
        resampled_mono
    } else {
        let mut interleaved = Vec::with_capacity(resampled_mono.len() * out_ch);
        for &s in &resampled_mono {
            for _ in 0..out_ch {
                interleaved.push(s);
            }
        }
        interleaved
    }
}

/// Abstract interface for physical or mock speaker playback using CPAL.
pub struct AudioOutput {
    is_playing: Arc<AtomicBool>,
    volume: Arc<Mutex<f32>>,
    active_cancel_flag: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(Mutex::new(1.0)),
            active_cancel_flag: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    /// Immediately interrupt and cancel active audio playback (Barge-In).
    pub fn stop(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.active_cancel_flag.lock() {
            if let Some(flag) = guard.take() {
                flag.store(true, Ordering::SeqCst);
            }
        }
        info!("Immediate audio playback cancellation (barge-in) triggered");
    }

    /// Play synthesized speech audio to physical default output speakers via CPAL.
    #[instrument(skip(self, speech))]
    pub async fn play(&self, speech: SynthesizedSpeech) -> Result<(), SpeechError> {
        if speech.audio_chunk.samples.is_empty() {
            info!("Skipping playback for empty audio chunk");
            return Ok(());
        }

        self.is_playing.store(true, Ordering::SeqCst);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut guard = self.active_cancel_flag.lock().unwrap();
            *guard = Some(cancel_flag.clone());
        }

        let is_playing_flag = self.is_playing.clone();
        let cancel_cb = cancel_flag.clone();
        let volume = *self.volume.lock().unwrap();
        let text_repr = speech.text.clone();

        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_output_device() {
                Some(dev) => dev,
                None => {
                    let _ = tx.send(Err(SpeechError::DeviceUnavailable(
                        "No default CPAL audio output device found".to_string(),
                    )));
                    return;
                }
            };

            let device_name = device.name().unwrap_or_else(|_| "Default Speaker".to_string());
            let default_config = match device.default_output_config() {
                Ok(cfg) => cfg,
                Err(e) => {
                    let _ = tx.send(Err(SpeechError::OutputFailure(format!(
                        "Failed to get output audio stream config for '{}': {}",
                        device_name, e
                    ))));
                    return;
                }
            };

            let sample_rate = default_config.sample_rate().0;
            let channels = default_config.channels();

            let resampled = resample_pcm(
                &speech.audio_chunk.samples,
                speech.audio_chunk.format.sample_rate,
                speech.audio_chunk.format.channels,
                sample_rate,
                channels,
            );

            if resampled.is_empty() {
                let _ = tx.send(Ok(()));
                return;
            }

            let total_samples = resampled.len();
            let cursor_state = Arc::new(Mutex::new((0usize, resampled)));
            let state_cb = cursor_state.clone();
            let cancel_stream_cb = cancel_cb.clone();

            let err_fn = |err| {
                warn!("CPAL audio stream output error: {}", err);
            };

            let stream_result = match default_config.sample_format() {
                cpal::SampleFormat::F32 => {
                    device.build_output_stream(
                        &default_config.into(),
                        move |data: &mut [f32], _| {
                            if cancel_stream_cb.load(Ordering::Relaxed) {
                                data.fill(0.0);
                                return;
                            }
                            let mut guard = match state_cb.lock() {
                                Ok(g) => g,
                                Err(_) => return,
                            };
                            let (ref mut pos, ref samples) = *guard;
                            for sample in data.iter_mut() {
                                if *pos < samples.len() {
                                    *sample = samples[*pos] * volume;
                                    *pos += 1;
                                } else {
                                    *sample = 0.0;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    device.build_output_stream(
                        &default_config.into(),
                        move |data: &mut [i16], _| {
                            if cancel_stream_cb.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            let mut guard = match state_cb.lock() {
                                Ok(g) => g,
                                Err(_) => return,
                            };
                            let (ref mut pos, ref samples) = *guard;
                            for sample in data.iter_mut() {
                                if *pos < samples.len() {
                                    let f = (samples[*pos] * volume).clamp(-1.0, 1.0);
                                    *sample = (f * 32767.0) as i16;
                                    *pos += 1;
                                } else {
                                    *sample = 0;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    device.build_output_stream(
                        &default_config.into(),
                        move |data: &mut [u16], _| {
                            if cancel_stream_cb.load(Ordering::Relaxed) {
                                data.fill(32768);
                                return;
                            }
                            let mut guard = match state_cb.lock() {
                                Ok(g) => g,
                                Err(_) => return,
                            };
                            let (ref mut pos, ref samples) = *guard;
                            for sample in data.iter_mut() {
                                if *pos < samples.len() {
                                    let f = (samples[*pos] * volume).clamp(-1.0, 1.0);
                                    *sample = ((f * 32767.0) + 32768.0) as u16;
                                    *pos += 1;
                                } else {
                                    *sample = 32768;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                }
                fmt => {
                    let _ = tx.send(Err(SpeechError::OutputFailure(format!(
                        "Unsupported CPAL output sample format: {:?}",
                        fmt
                    ))));
                    return;
                }
            };

            let stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(SpeechError::OutputFailure(format!(
                        "Failed to build CPAL audio output stream for device '{}': {}",
                        device_name, e
                    ))));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = tx.send(Err(SpeechError::OutputFailure(format!(
                    "Failed starting CPAL audio playback stream: {}",
                    e
                ))));
                return;
            }

            info!(
                device = %device_name,
                sample_rate = sample_rate,
                channels = channels,
                text = %text_repr,
                "Real audio playback streaming on default output device"
            );

            while is_playing_flag.load(Ordering::SeqCst) && !cancel_cb.load(Ordering::SeqCst) {
                let pos = cursor_state.lock().unwrap().0;
                if pos >= total_samples {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }

            drop(stream);

            if cancel_cb.load(Ordering::SeqCst) {
                info!("Audio playback canceled due to barge-in");
                let _ = tx.send(Err(SpeechError::Interrupted));
            } else {
                info!("Audio playback completed successfully");
                let _ = tx.send(Ok(()));
            }
        });

        let res = rx.await.unwrap_or(Err(SpeechError::OutputFailure(
            "Playback thread terminated".to_string(),
        )));

        self.is_playing.store(false, Ordering::SeqCst);
        res
    }
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_pcm_identity() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let resampled = resample_pcm(&input, 16000, 1, 16000, 1);
        assert_eq!(resampled.len(), 4);
        for (a, b) in input.iter().zip(resampled.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }

    #[test]
    fn test_resample_pcm_upsample_and_stereo() {
        let input = vec![0.0, 1.0]; // 2 samples mono at 16000Hz -> 4 samples mono at 32000Hz -> 8 samples stereo
        let resampled = resample_pcm(&input, 16000, 1, 32000, 2);
        assert_eq!(resampled.len(), 8);
    }

    #[test]
    fn test_mono_to_stereo_expansion() {
        let mono = vec![0.5f32, -0.5f32];
        let stereo = resample_pcm(&mono, 22050, 1, 22050, 2);
        assert_eq!(stereo.len(), 4);
        assert_eq!(stereo, vec![0.5, 0.5, -0.5, -0.5]);
    }

    #[test]
    fn test_resample_pcm_bounds_and_clamping() {
        let input = vec![1.5f32, -2.0f32, 0.8f32];
        let resampled = resample_pcm(&input, 22050, 1, 44100, 2);
        assert!(!resampled.is_empty());
        for &s in &resampled {
            assert!(s >= -1.0 && s <= 1.0, "Sample {} out of bounds [-1.0, 1.0]", s);
        }
    }

    #[test]
    fn test_sample_format_conversions() {
        let val_f32 = 0.5f32;
        let i16_val = (val_f32 * 32767.0) as i16;
        assert_eq!(i16_val, 16383);

        let u16_val = ((val_f32 * 32767.0) + 32768.0) as u16;
        assert_eq!(u16_val, 49151);

        let neg_f32 = -1.0f32;
        let neg_i16 = (neg_f32 * 32767.0) as i16;
        assert_eq!(neg_i16, -32767);

        let neg_u16 = ((neg_f32 * 32767.0) + 32768.0) as u16;
        assert_eq!(neg_u16, 1);
    }
}
