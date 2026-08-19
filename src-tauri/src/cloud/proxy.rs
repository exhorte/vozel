//! Proxy optionnel vers les fournisseurs ASR/LLM cloud tiers, pour les
//! utilisateurs qui préfèrent que Vozel gère la clé API plutôt que de la
//! saisir eux-mêmes (BYO clé restant possible, comme chez Freestyle).
//! TODO (Phase 2).

#[allow(dead_code)]
pub fn forward_request(_payload: &[u8]) -> Result<Vec<u8>, String> {
    Err("not implemented".into())
}
