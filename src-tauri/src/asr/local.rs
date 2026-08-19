//! Moteur ASR local (Phase 1).
//!
//! TODO : charger un modèle Parakeet-TDT (ONNX Runtime) ou whisper.cpp
//! (bindings Rust) selon la config utilisateur / benchmark retenu.
//! Le modèle doit être téléchargeable/gérable via `storage::settings`
//! (choix de la taille du modèle selon la machine de l'utilisateur).

use super::types::TranscriptionResult;
use super::AsrEngine;

#[allow(dead_code)]
pub struct LocalAsrEngine {
    // TODO: handle vers le modèle chargé (ONNX Runtime session, etc.)
}

impl AsrEngine for LocalAsrEngine {
    fn transcribe(&self, _audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String> {
        // TODO: inférence locale.
        Err("not implemented".into())
    }
}
