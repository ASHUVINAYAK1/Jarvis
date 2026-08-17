# ADR-0006: Local Voice Pipeline, Speech Subsystem, and Duplex Interruption

- **Status:** Accepted
- **Date:** 2026-08-17
- **Deciders:** Principal Software Architect / Implementation Agent
- **Technical Context:** Document 2, Document 18, Document 22 (Voice & Speech Architecture)

---

## Context and Problem Statement

JARVIS requires a real-time, privacy-preserving, local voice interaction pipeline. The system must continuously process microphone audio, detect speech energy via Voice Activity Detection (VAD), identify the local "JARVIS" wake-word, transcribe utterances locally using Whisper STT, reason via the ModelGateway (Phase 04), synthesize spoken responses locally using Piper TTS, play audio to speakers, and instantly support user barge-in interruption.

## Decision

1. **Service Architecture (`services/speech`)**:
   - `AudioDeviceManager`: Discovers and enumerates microphones and speakers with default fallback.
   - `AudioCapture`: Real-time microphone capture emitting 16kHz Mono 32-bit float PCM chunks over bounded channels.
   - `VoiceActivityDetector`: Energy-based VAD evaluating silence timeouts (700ms) and minimum speech duration (300ms).
   - `WakeWordDetector`: Local "JARVIS" wake-word engine with configurable confidence threshold (0.65), debounce guard, and cooldown timer (1.5s).
   - `SpeechToText`: Async trait with `WhisperSttEngine` and `MockSttEngine`.
   - `TextToSpeech`: Async trait with `PiperTtsEngine` (`en_GB-alan-medium`) and `MockTtsEngine`.
   - `AudioOutput`: Speaker playback driver supporting instant barge-in cancellation (`stop()`).
   - `VoiceSessionController`: Orchestrates the real-time voice session state machine (`Idle` → `WakeDetected` → `Listening` → `Transcribing` → `Thinking` → `Speaking` → `Idle`), publishing state updates to `EventBus` (`VoiceEvent`) and routing queries through `ModelGateway`.
2. **Duplex Interruption & Barge-In**:
   - Audio playback is driven in 50ms ticks. If user speech or a cancellation signal occurs during speech synthesis, `AudioOutput` halts instantly, emitting `SpeechError::Interrupted` and transitioning the session back to `Listening`.
3. **Local-Only Privacy**:
   - Microphone audio is processed entirely in-memory and discarded. Zero cloud dependencies or audio uploads.

## Consequences

- **Positive:** Full local voice interaction pipeline operating offline.
- **Positive:** Low-latency barge-in interruption allows natural user interaction.
- **Positive:** Seamless event bus and ModelGateway integration without coupling to platform audio hardware.
