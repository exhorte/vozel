//! Nettoyage et reformulation IA du texte transcrit — couche découplée de
//! l'ASR (voir analyse section 4.4).
//!
//! Phase 1 : ponctuation/grammaire basique + suppression des hésitations.
//! Phase 2 §2.3 : nettoyage avancé par un petit LLM local — **ONNX/`ort`**,
//! pas GGUF/llama.cpp (voir `llm` + `01_Recherche/Approche_LLM_Local.md`).
//!
//! `commands::run_pipeline` ne voit qu'un `dyn TextCleaner` : peu importe
//! que le nettoyage vienne des règles ou du LLM (même principe que
//! `asr::RoutingAsrEngine`). Repli **obligatoire** sur les règles si le LLM
//! est absent ou échoue — ne jamais casser une dictée à cause du LLM.

pub mod cleanup;
pub mod command_mode;
pub mod llm;

use std::sync::Arc;

use llm::{GenParams, LlmEngine};

/// Interface commune de nettoyage du texte transcrit.
pub trait TextCleaner: Send + Sync {
    /// `raw` : texte brut de l'ASR. `dictionary` : remplacements exacts
    /// (§2.2) à appliquer. Retourne le texte nettoyé (jamais d'erreur —
    /// toute défaillance retombe sur les règles).
    fn clean(&self, raw: &str, dictionary: &[(String, String)]) -> String;
}

/// Nettoyage par règles seules (Phase 1 §1.4) — toujours disponible.
pub struct RuleCleaner;

impl TextCleaner for RuleCleaner {
    fn clean(&self, raw: &str, dictionary: &[(String, String)]) -> String {
        cleanup::clean(raw, dictionary)
    }
}

/// Nettoyage avancé par LLM local (§2.3), avec repli automatique sur les
/// règles. Chaîne : dictionnaire + règles de base (`cleanup::clean`) →
/// LLM → passe de règles finale (garantit capitalisation + ponctuation
/// finale même si le LLM les a manquées). Si le LLM lève une erreur ou
/// renvoie du vide, on garde le résultat « règles seules ».
pub struct LlmCleaner {
    /// Partagé (via `Arc`) avec `CommandModeState` quand le Command Mode est
    /// aussi actif — le modèle n'est chargé qu'une fois (voir `lib.rs`).
    engine: Arc<LlmEngine>,
    params: GenParams,
}

impl LlmCleaner {
    pub fn new(engine: Arc<LlmEngine>, params: GenParams) -> Self {
        Self { engine, params }
    }
}

impl TextCleaner for LlmCleaner {
    fn clean(&self, raw: &str, dictionary: &[(String, String)]) -> String {
        let rules_only = cleanup::clean(raw, dictionary);
        if rules_only.is_empty() {
            return rules_only;
        }
        match self.engine.clean(&rules_only, &self.params) {
            Ok(llm_out) if !llm_out.trim().is_empty() => cleanup::clean(&llm_out, &[]),
            Ok(_) => {
                eprintln!("[postprocess] LLM a renvoyé du vide, repli sur les règles");
                rules_only
            }
            Err(e) => {
                eprintln!("[postprocess] échec du nettoyage LLM ({e}), repli sur les règles");
                rules_only
            }
        }
    }
}
