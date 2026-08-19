//! Reconnaissance vocale (ASR) — moteurs locaux et cloud, interchangeables.
//!
//! Voir `01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` (section 4.3) :
//! - Local par défaut : Parakeet-TDT (ONNX INT8) ou whisper.cpp quantifié,
//!   à trancher après benchmark français (voir Roadmap Phase 0).
//! - Cloud optionnel (Phase 2) : multi-fournisseurs au choix utilisateur
//!   (Groq, OpenAI, Deepgram...), avec clé API fournie par l'utilisateur ou
//!   proxy via `cloud::proxy`.

pub mod local;
pub mod cloud;
pub mod types;

use types::TranscriptionResult;

/// Interface commune à tous les moteurs ASR (local ou cloud), pour pouvoir
/// les échanger sans changer le reste du pipeline.
pub trait AsrEngine {
    fn transcribe(&self, audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String>;
}
