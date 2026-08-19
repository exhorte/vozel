//! Capture audio et détection d'activité vocale (VAD).
//!
//! Phase 1 (MVP) : capture microphone via `cpal`, VAD (ex. Silero VAD) pour
//! détecter début/fin de parole et couper les silences avant transcription.
//! Tourne dans son propre thread, découplé de l'UI, pour ne jamais bloquer
//! le rendu de la fenêtre flottante.

pub mod capture;
pub mod vad;
