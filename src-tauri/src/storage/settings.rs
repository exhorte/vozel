//! Réglages utilisateur : choix du modèle ASR (taille/local vs cloud),
//! raccourci clavier, fournisseur cloud + clé API, préférences de
//! confidentialité.
//!
//! TODO (Phase 1) : structure de réglages sérialisable (serde) persistée
//! via `storage::db` ou fichier de config au démarrage.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub asr_provider: String,
    pub hotkey: String,
    pub cloud_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            asr_provider: "local".into(),
            hotkey: "Fn".into(),
            cloud_enabled: false,
        }
    }
}
