//! Audio Output & Speaker Playback Control

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, instrument};

use crate::types::{SpeechError, SynthesizedSpeech};

/// Abstract interface for physical or mock speaker playback.
pub struct AudioOutput {
    is_playing: Arc<AtomicBool>,
    #[allow(dead_code)]
    volume: Arc<Mutex<f32>>,
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(Mutex::new(1.0)),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    /// Play synthesized speech audio to speakers.
    #[instrument(skip(self, speech))]
    pub async fn play(&self, speech: SynthesizedSpeech) -> Result<(), SpeechError> {
        self.is_playing.store(true, Ordering::SeqCst);
        info!(text = %speech.text, duration_ms = speech.duration_ms, "Audio output playback started");

        let is_playing_flag = self.is_playing.clone();
        let dur = speech.duration_ms;

        // Simulate real-time audio playback chunk by chunk (supports instant barge-in cancellation)
        let elapsed_ticks = (dur / 50).max(1);
        for _ in 0..elapsed_ticks {
            if !is_playing_flag.load(Ordering::SeqCst) {
                info!("Playback interrupted by user barge-in");
                return Err(SpeechError::Interrupted);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        self.is_playing.store(false, Ordering::SeqCst);
        info!("Audio output playback finished");
        Ok(())
    }

    /// Immediately interrupt and cancel active audio playback (Barge-In).
    pub fn stop(&self) {
        if self.is_playing.swap(false, Ordering::SeqCst) {
            info!("Immediate audio playback cancellation triggered");
        }
    }
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}
