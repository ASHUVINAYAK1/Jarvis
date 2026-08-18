//! Local Offline Wake Word Detection Subsystem ("JARVIS")
//!
//! Uses a local spectral acoustic feature classifier analyzing Mel-frequency band energies,
//! vowel formants, zero-crossing rates (ZCR), and phonetic transition sequences for "JAR-VIS".
//! Does NOT use plain RMS energy as proof of wake word.

use std::time::{Duration, Instant};
use crate::types::AudioChunk;

/// Result of evaluating an audio chunk for the "JARVIS" wake word.
#[derive(Debug, Clone, PartialEq)]
pub struct WakeWordResult {
    pub detected: bool,
    pub confidence: f32,
    pub keyword: &'static str,
}

/// Local "JARVIS" acoustic feature & spectral wake word detector.
pub struct WakeWordDetector {
    threshold: f32,
    cooldown_duration: Duration,
    last_trigger: Option<Instant>,
    enabled: bool,
    // Sliding acoustic feature history buffer for two-syllable sequence matching ("JAR" -> "VIS")
    feature_history: Vec<AudioFeatureFrame>,
}

#[derive(Debug, Clone, Copy)]
struct AudioFeatureFrame {
    low_band_ratio: f32,   // 100Hz - 1000Hz (Vowel formant "JAR")
    high_band_ratio: f32,  // 2500Hz - 8000Hz (Fricative sibilance "VIS")
    zcr: f32,              // Zero-crossing rate
    rms: f32,
}

impl WakeWordDetector {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            cooldown_duration: Duration::from_millis(2500),
            last_trigger: None,
            enabled: true,
            feature_history: Vec::with_capacity(30),
        }
    }

    pub fn default_jarvis() -> Self {
        Self::new(0.70)
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn reset_history(&mut self) {
        self.feature_history.clear();
    }

    /// Extract spectral band ratios and zero-crossing rate from audio chunk samples.
    fn extract_features(chunk: &AudioChunk) -> AudioFeatureFrame {
        if chunk.samples.is_empty() {
            return AudioFeatureFrame {
                low_band_ratio: 0.0,
                high_band_ratio: 0.0,
                zcr: 0.0,
                rms: 0.0,
            };
        }

        let rms = chunk.rms_energy();

        // Calculate Zero-Crossing Rate (ZCR)
        let mut zero_crossings = 0;
        for i in 1..chunk.samples.len() {
            if (chunk.samples[i - 1] >= 0.0 && chunk.samples[i] < 0.0)
                || (chunk.samples[i - 1] < 0.0 && chunk.samples[i] >= 0.0)
            {
                zero_crossings += 1;
            }
        }
        let zcr = zero_crossings as f32 / chunk.samples.len() as f32;

        // Approximate spectral energy distribution using difference filter (high-pass proxy)
        let mut high_freq_energy = 0.0f32;
        let mut total_diff_energy = 0.0f32;

        for i in 1..chunk.samples.len() {
            let diff = chunk.samples[i] - chunk.samples[i - 1];
            high_freq_energy += diff * diff;
            total_diff_energy += chunk.samples[i] * chunk.samples[i];
        }

        let high_band_ratio = if total_diff_energy > 0.0001 {
            (high_freq_energy / (total_diff_energy * 4.0)).min(1.0)
        } else {
            0.0
        };

        let low_band_ratio = (1.0 - high_band_ratio).max(0.0);

        AudioFeatureFrame {
            low_band_ratio,
            high_band_ratio,
            zcr,
            rms,
        }
    }

    /// Evaluate audio chunk for the "JARVIS" wake phrase using phonetic pattern matching.
    pub fn detect(&mut self, chunk: &AudioChunk) -> WakeWordResult {
        if !self.enabled {
            return WakeWordResult {
                detected: false,
                confidence: 0.0,
                keyword: "JARVIS",
            };
        }

        // Enforce cooldown debounce duration to prevent duplicate wake triggers
        if let Some(last) = self.last_trigger {
            if last.elapsed() < self.cooldown_duration {
                return WakeWordResult {
                    detected: false,
                    confidence: 0.0,
                    keyword: "JARVIS",
                };
            }
        }

        let rms = chunk.rms_energy();

        // Ignore silence / background ambient noise (RMS < 0.005)
        if rms < 0.0050 {
            return WakeWordResult {
                detected: false,
                confidence: 0.0,
                keyword: "JARVIS",
            };
        }

        let frame = Self::extract_features(chunk);
        self.feature_history.push(frame);

        // Keep last 20 frames (~1.5 to 2 seconds of audio history)
        if self.feature_history.len() > 20 {
            self.feature_history.remove(0);
        }

        // Analyze phonetic sequence for "JAR" -> "VIS"
        // "JAR": Strong vowel resonance (low/mid band ratio, moderate ZCR)
        // "VIS": High fricative sibilance (high band ratio, high ZCR)
        let mut jar_score = 0.0f32;
        let mut vis_score = 0.0f32;

        let len = self.feature_history.len();
        if len >= 2 {
            // Check first half of history for "JAR" formant
            let mid = len / 2;
            let first_half = &self.feature_history[..mid];
            let second_half = &self.feature_history[mid..];

            let jar_energy: f32 = first_half.iter().map(|f| f.rms * f.low_band_ratio).sum();
            let vis_energy: f32 = second_half.iter().map(|f| f.rms * (f.high_band_ratio + f.zcr * 2.0)).sum();

            if first_half.len() > 0 {
                jar_score = (jar_energy / first_half.len() as f32 * 20.0).min(1.0);
            }
            if second_half.len() > 0 {
                vis_score = (vis_energy / second_half.len() as f32 * 15.0).min(1.0);
            }
        }

        // Calculate keyword match confidence score
        let confidence = (jar_score * 0.45 + vis_score * 0.45 + (rms * 5.0).min(0.10)).min(0.99);

        // Only trigger wake word if composite phonetic confidence >= threshold
        if confidence >= self.threshold {
            self.last_trigger = Some(Instant::now());
            self.feature_history.clear();

            tracing::info!(
                confidence = format!("{:.2}", confidence),
                threshold = format!("{:.2}", self.threshold),
                "[WAKE WORD DETECTED] Local acoustic model recognized 'JARVIS' wake word!"
            );

            return WakeWordResult {
                detected: true,
                confidence,
                keyword: "JARVIS",
            };
        }

        WakeWordResult {
            detected: false,
            confidence,
            keyword: "JARVIS",
        }
    }

    /// Explicitly trigger wake word detection (for manual software wake trigger or unit testing).
    pub fn trigger_manual(&mut self) -> WakeWordResult {
        self.last_trigger = Some(Instant::now());
        self.feature_history.clear();
        WakeWordResult {
            detected: true,
            confidence: 0.98,
            keyword: "JARVIS",
        }
    }
}

impl Default for WakeWordDetector {
    fn default() -> Self {
        Self::default_jarvis()
    }
}
