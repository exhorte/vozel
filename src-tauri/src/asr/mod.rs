//! Reconnaissance vocale (ASR) — moteurs locaux et cloud, interchangeables.
//!
//! Voir `01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` (section 4.3) :
//! - Local par défaut : Parakeet-TDT (ONNX INT8) ou whisper.cpp quantifié,
//!   à trancher après benchmark français (voir Roadmap Phase 0).
//! - Cloud optionnel (Phase 2) : multi-fournisseurs au choix utilisateur
//!   (Groq, OpenAI, Deepgram...), avec clé API fournie par l'utilisateur ou
//!   proxy via `cloud::proxy`.

pub mod cloud;
pub mod local;
pub mod types;

use cloud::CloudAsrEngine;
use local::LocalAsrEngine;
use sqlx::SqlitePool;
use types::TranscriptionResult;

use crate::storage::settings::Settings;

/// Interface commune à tous les moteurs ASR (local ou cloud), pour pouvoir
/// les échanger sans changer le reste du pipeline.
pub trait AsrEngine {
    fn transcribe(&self, audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String>;
}

/// Aiguilleur local ↔ cloud (Spec_Backend_Desktop.md §2.4). Relit
/// `Settings::cloud_enabled` (+ fournisseur/clé) à **chaque** transcription :
/// un changement dans les réglages prend effet dès la dictée suivante, sans
/// redémarrage. Expose au pipeline la **même** interface `AsrEngine` — c'est
/// tout ce que `commands::run_pipeline` connaît, il n'a aucune notion du
/// choix local/cloud (critère d'acceptation §2.4).
pub struct RoutingAsrEngine {
    local: Option<LocalAsrEngine>,
    cloud: CloudAsrEngine,
    pool: SqlitePool,
}

impl RoutingAsrEngine {
    pub fn new(local: Option<LocalAsrEngine>, cloud: CloudAsrEngine, pool: SqlitePool) -> Self {
        Self { local, cloud, pool }
    }
}

impl AsrEngine for RoutingAsrEngine {
    fn transcribe(&self, audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String> {
        let settings = tauri::async_runtime::block_on(Settings::load_db(&self.pool))
            .unwrap_or_else(|e| {
                eprintln!("[asr] lecture des réglages impossible ({e}), moteur local par défaut");
                Settings::default()
            });

        if settings.cloud_enabled {
            self.cloud.transcribe(
                audio_pcm16k,
                &settings.cloud_provider,
                &settings.cloud_api_key,
            )
        } else {
            match &self.local {
                Some(engine) => engine.transcribe(audio_pcm16k),
                None => Err(
                    "moteur ASR local indisponible (modèle non installé) et cloud désactivé"
                        .into(),
                ),
            }
        }
    }
}
