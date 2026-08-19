//! Raccourci clavier global d'activation de la dictée (Phase 1).
//!
//! TODO : intégrer une crate cross-plateforme type `global-hotkey` (déjà
//! utilisée dans l'écosystème Tauri). Deux modes attendus par l'utilisateur,
//! configurables dans les réglages :
//! - push-to-talk : dictée active tant que la touche est maintenue
//! - toggle : un appui démarre, un second arrête

#[allow(dead_code)]
pub enum HotkeyMode {
    PushToTalk,
    Toggle,
}

#[allow(dead_code)]
pub struct HotkeyManager;

impl HotkeyManager {
    pub fn register(_mode: HotkeyMode) -> Result<Self, String> {
        // TODO: enregistrer le raccourci global et brancher les callbacks
        // start/stop vers `audio::capture`.
        Err("not implemented".into())
    }
}
