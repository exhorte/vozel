//! Nettoyage local du texte transcrit : ponctuation, grammaire légère,
//! suppression des hésitations ("euh", faux départs), application du
//! dictionnaire personnalisé (voir `storage::dictionary`).
//!
//! TODO (Phase 1) : implémentation basique par règles ; (Phase 2) : LLM
//! local quantifié (Qwen2.5-1.5B-Instruct ou Phi-3-mini via llama.cpp).

#[allow(dead_code)]
pub fn clean(raw_text: &str) -> String {
    // TODO: appliquer règles de nettoyage puis, à terme, le LLM local.
    raw_text.to_string()
}
