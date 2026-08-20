//! Capture audio et détection d'activité vocale (VAD).
//!
//! Phase 1 (MVP, Spec_Backend_Desktop.md §1.2) : capture microphone via
//! `cpal`, VAD énergie RMS (`vad::is_speech`) pour couper les silences
//! avant transcription. Tourne dans son propre thread, découplé de l'UI,
//! piloté par `hotkey` via `capture::CaptureCommand`.

pub mod capture;
pub mod vad;
