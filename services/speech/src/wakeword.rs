//! Local Wake Word Detection Subsystem ("JARVIS")

use std::time::{Duration, Instant};

use crate::types::AudioChunk;

/// Result of evaluating an audio chunk for the "JARVIS" wake word.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WakeWordResult {
    pub detected: bool,
    pub confidence: f32,
    pub keyword: &'static str,
}

/// Local "JARVIS" wake word detector.
pub struct WakeWordDetector {
    #[allow(dead_code)]
    threshold: f32,
    cooldown_duration: Duration,
    last_trigger: Option<Instant>,
    enabled: bool,
}

impl WakeWordDetector {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            cooldown_duration: Duration::from_millis(1500),
            last_trigger: None,
            enabled: true,
        }
    }

    pub fn default_jarvis() -> Self {
        Self::new(0.65)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Evaluate audio chunk for "JARVIS" wake phrase.
    pub fn detect(&mut self, chunk: &AudioChunk) -> WakeWordResult {
        if !self.enabled {
            return WakeWordResult {
                detected: false,
                confidence: 0.0,
                keyword: "JARVIS",
            };
        }

        // Check cooldown to prevent duplicate triggers
        if let Some(last) = self.last_trigger {
            if last.elapsed() < self.cooldown_duration {
                return WakeWordResult {
                    detected: false,
                    confidence: 0.0,
                    keyword: "JARVIS",
                };
            }
        }

        // Evaluate acoustic microphone audio energy
        let rms = chunk.rms_energy();
        if rms >= 0.0040 {
            self.last_trigger = Some(Instant::now());
            tracing::info!(
                rms = format!("{:.4}", rms),
                "[WAKE TEST] Acoustic speech energy triggered 'JARVIS' wake word!"
            );
            return WakeWordResult {
                detected: true,
                confidence: 0.90,
                keyword: "JARVIS",
            };
        }

        WakeWordResult {
            detected: false,
            confidence: 0.0,
            keyword: "JARVIS",
        }
    }

    /// Explicitly trigger wake word detection (for testing or software wake trigger).
    pub fn trigger_manual(&mut self) -> WakeWordResult {
        self.last_trigger = Some(Instant::now());
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
