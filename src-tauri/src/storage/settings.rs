//! Réglages utilisateur : choix du modèle ASR (taille/local vs cloud),
//! raccourci clavier, fournisseur cloud + clé API, préférences de
//! confidentialité.
//!
//! TODO (Phase 1) : structure de réglages sérialisable (serde) persistée
//! via `storage::db` ou fichier de config au démarrage.

use crate::hotkey::HotkeyMode;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub asr_provider: String,
    /// Syntaxe `global_hotkey::hotkey::HotKey` (ex. `"control+alt+Space"`).
    pub hotkey: String,
    pub hotkey_mode: HotkeyMode,
    pub cloud_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            asr_provider: "local".into(),
            // "Fn" seul n'est pas utilisable : sur la plupart des claviers
            // laptop, la touche Fn est interceptée par le contrôleur clavier
            // (firmware) et n'atteint jamais l'OS comme un événement clavier
            // normal — `RegisterHotKey` (Win32) ne peut donc pas s'y
            // enregistrer. `control+alt+Space` testé et rejeté aussi (déjà
            // pris par une autre app/le système sur la machine de dev —
            // Ctrl+Alt est par ailleurs l'équivalent d'AltGr sur beaucoup de
            // claviers non-US, donc à éviter pour un raccourci global).
            // Défaut retenu : `control+shift+Space`, modifiable dans les
            // réglages (`Spec_Frontend.md` Phase 1 §1.2).
            hotkey: "control+shift+Space".into(),
            hotkey_mode: HotkeyMode::Toggle,
            cloud_enabled: false,
        }
    }
}
