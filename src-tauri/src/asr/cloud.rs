//! Moteur ASR cloud multi-fournisseurs (Phase 2), à la manière de Freestyle :
//! l'utilisateur choisit son fournisseur (Groq, OpenAI, Deepgram, ...) et
//! fournit sa clé API, ou passe par `cloud::proxy` si Vozel gère la clé.
//!
//! TODO : définir un enum `Provider` + implémentation HTTP par fournisseur.

use super::types::TranscriptionResult;
use super::AsrEngine;

#[allow(dead_code)]
pub enum Provider {
    Groq,
    OpenAi,
    Deepgram,
}

#[allow(dead_code)]
pub struct CloudAsrEngine {
    pub provider: Provider,
}

impl AsrEngine for CloudAsrEngine {
    fn transcribe(&self, _audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String> {
        // TODO: appel HTTP streaming vers le fournisseur choisi.
        Err("not implemented".into())
    }
}
